use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::core::config::FetchSnapshotSection;
use crate::core::fetch::{ExtractMode, FetchSnapshotRequest};
use crate::fetch::cache::CacheScope;

use super::diff::{
    build_change, normalize_text, sha256, Change, MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_LINES,
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Clone)]
pub(crate) struct SnapshotIdentity {
    pub scope: CacheScope,
    pub url: String,
    pub extract_mode: ExtractMode,
    pub scoping_fingerprint: Option<u64>,
    pub representation_class: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SnapshotOutcome {
    pub change: Option<Change>,
    pub warnings: Vec<String>,
}

impl SnapshotOutcome {
    fn change(change: Change) -> Self {
        Self {
            change: Some(change),
            warnings: Vec::new(),
        }
    }

    pub(crate) fn warning(warning: impl Into<String>) -> Self {
        Self {
            change: None,
            warnings: vec![warning.into()],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoreError {
    Unavailable,
    SchemaUnsupported,
    Corrupt,
    ContentLimit,
}

impl StoreError {
    fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "snapshot_store_unavailable",
            Self::SchemaUnsupported => "snapshot_store_schema_unsupported",
            Self::Corrupt => "snapshot_store_corrupt",
            Self::ContentLimit => "snapshot_content_limit_exceeded",
        }
    }

    fn warning(self) -> SnapshotOutcome {
        SnapshotOutcome::warning(format!("snapshot_store_unavailable: {}", self.code()))
    }
}

struct StoredSnapshot {
    content_sha256: String,
    text: String,
    fetched_at: i64,
}

pub(crate) fn validate_request(request: &FetchSnapshotRequest) -> Result<(), String> {
    if let Some(key) = request.key.as_deref() {
        if key.len() > 128 || key.trim().is_empty() || key.chars().any(char::is_control) {
            return Err("invalid_snapshot_key: snapshot.key must be non-empty, contain no control characters, and be at most 128 UTF-8 bytes".into());
        }
    }
    if request.max_hunks.is_some_and(|value| value > 100) {
        return Err("snapshot.max_hunks must be at most 100".into());
    }
    if request.max_diff_chars.is_some_and(|value| value > 32_768) {
        return Err("snapshot.max_diff_chars must be at most 32768".into());
    }
    Ok(())
}

pub(crate) async fn compare_text(
    config: FetchSnapshotSection,
    request: FetchSnapshotRequest,
    identity: SnapshotIdentity,
    text: String,
) -> SnapshotOutcome {
    if !config.enabled {
        return SnapshotOutcome::warning("snapshot_store_disabled");
    }
    tokio::task::spawn_blocking(move || compare_text_sync(&config, &request, &identity, &text))
        .await
        .unwrap_or_else(|_| StoreError::Unavailable.warning())
}

pub(crate) async fn observe_removed(
    config: FetchSnapshotSection,
    request: FetchSnapshotRequest,
    identity: SnapshotIdentity,
) -> SnapshotOutcome {
    if !config.enabled {
        return SnapshotOutcome::warning("snapshot_store_disabled");
    }
    tokio::task::spawn_blocking(move || observe_removed_sync(&config, &request, &identity))
        .await
        .unwrap_or_else(|_| StoreError::Unavailable.warning())
}

fn compare_text_sync(
    config: &FetchSnapshotSection,
    request: &FetchSnapshotRequest,
    identity: &SnapshotIdentity,
    text: &str,
) -> SnapshotOutcome {
    let normalized = normalize_text(text);
    if normalized.len() > MAX_SNAPSHOT_BYTES
        || normalized.lines().count() > MAX_SNAPSHOT_LINES
        || normalized.len() > config.max_bytes
    {
        return StoreError::ContentLimit.warning();
    }
    let key = snapshot_address(identity, request.key.as_deref());
    let now = Utc::now().timestamp();
    let current_fetched_at = format_timestamp(now);
    let mut store = match SnapshotStore::open(config) {
        Ok(store) => store,
        Err(error) => return error.warning(),
    };
    let previous = match store.get(&key) {
        Ok(previous) => previous,
        Err(error) => return error.warning(),
    };
    if previous
        .as_ref()
        .is_some_and(|previous| previous.content_sha256 != sha256(&previous.text))
    {
        return StoreError::Corrupt.warning();
    }
    let previous_tuple = previous.as_ref().map(|previous| {
        (
            previous.text.as_str(),
            format_timestamp(previous.fetched_at),
        )
    });
    let change = build_change(
        previous_tuple
            .as_ref()
            .map(|(text, fetched_at)| (*text, fetched_at.as_str())),
        Some(&normalized),
        current_fetched_at,
        request.max_hunks.unwrap_or(20),
        request.max_diff_chars.unwrap_or(8192),
    );
    let hash = sha256(&normalized);
    if let Err(error) = store.put(&key, &hash, &normalized, now) {
        return error.warning();
    }
    let mut outcome = SnapshotOutcome::change(change);
    if store.prune(now).is_err() {
        outcome.warnings.push("snapshot_prune_failed".into());
    }
    outcome
}

fn observe_removed_sync(
    config: &FetchSnapshotSection,
    request: &FetchSnapshotRequest,
    identity: &SnapshotIdentity,
) -> SnapshotOutcome {
    let now = Utc::now().timestamp();
    let mut store = match SnapshotStore::open(config) {
        Ok(store) => store,
        Err(error) => return error.warning(),
    };
    let mut found = None;
    for representation in ["html", "markdown", "other"] {
        let mut candidate = identity.clone();
        candidate.representation_class = representation.to_string();
        let key = snapshot_address(&candidate, request.key.as_deref());
        match store.get(&key) {
            Ok(Some(previous)) => {
                found = Some((key, previous));
                break;
            }
            Ok(None) => {}
            Err(error) => return error.warning(),
        }
    }
    let Some((key, previous)) = found else {
        return SnapshotOutcome::warning("snapshot_not_found_for_removal");
    };
    if previous.content_sha256 != sha256(&previous.text) {
        return StoreError::Corrupt.warning();
    }
    if store.mark_seen(&key, now).is_err() {
        return StoreError::Unavailable.warning();
    }
    SnapshotOutcome::change(build_change(
        Some((&previous.text, &format_timestamp(previous.fetched_at))),
        None,
        format_timestamp(now),
        request.max_hunks.unwrap_or(20),
        request.max_diff_chars.unwrap_or(8192),
    ))
}

fn snapshot_address(identity: &SnapshotIdentity, caller_key: Option<&str>) -> String {
    let mut hasher = crate::core::identity::FnvHasher::new();
    crate::core::identity::write_entity_prefix(&mut hasher, "fetch_snapshot");
    let scope = match &identity.scope {
        CacheScope::Anonymous => "anonymous".to_string(),
        CacheScope::Profile(profile_id) => format!("profile:{}", profile_id.as_ref()),
    };
    for value in [
        scope,
        crate::core::identity::canonicalize_url(&identity.url),
        match identity.extract_mode {
            ExtractMode::Text => "text".to_string(),
            ExtractMode::Markdown => "markdown".to_string(),
            ExtractMode::MetadataOnly => "metadata_only".to_string(),
        },
        identity
            .scoping_fingerprint
            .map(|value| format!("{value:016x}"))
            .unwrap_or_default(),
        identity.representation_class.clone(),
        caller_key.unwrap_or_default().to_string(),
    ] {
        crate::core::identity::write_str(&mut hasher, &value);
    }
    format!("snapshot_{:016x}", hasher.finish())
}

fn format_timestamp(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

struct SnapshotStore {
    connection: Connection,
    config: FetchSnapshotSection,
}

impl SnapshotStore {
    fn open(config: &FetchSnapshotSection) -> Result<Self, StoreError> {
        let path = resolve_path(config)?;
        prepare_path(&path, config)?;
        let connection = Connection::open(&path).map_err(map_sqlite)?;
        connection
            .busy_timeout(Duration::from_millis(5_000))
            .map_err(map_sqlite)?;
        connection
            .pragma_update(None, "secure_delete", true)
            .map_err(map_sqlite)?;
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(map_sqlite)?;
        let page_size: i64 = connection
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(map_sqlite)?;
        let max_pages = (config.max_bytes / page_size.max(1) as usize).max(1);
        let max_pages = i64::try_from(max_pages).unwrap_or(i64::MAX);
        connection
            .pragma_update(None, "max_page_count", max_pages)
            .map_err(map_sqlite)?;
        let page_count: i64 = connection
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .map_err(map_sqlite)?;
        if page_count > max_pages {
            return Err(StoreError::Unavailable);
        }
        ensure_schema(&connection)?;
        Ok(Self {
            connection,
            config: config.clone(),
        })
    }

    fn get(&self, key: &str) -> Result<Option<StoredSnapshot>, StoreError> {
        self.connection
            .query_row(
                "SELECT content_sha256, text, fetched_at FROM snapshots WHERE snapshot_key = ?1",
                [key],
                |row| {
                    Ok(StoredSnapshot {
                        content_sha256: row.get(0)?,
                        text: row.get(1)?,
                        fetched_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(map_sqlite)
    }

    fn put(&mut self, key: &str, hash: &str, text: &str, now: i64) -> Result<(), StoreError> {
        self.connection
            .execute(
                "INSERT INTO snapshots (snapshot_key, content_sha256, text, fetched_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?4) ON CONFLICT(snapshot_key) DO UPDATE SET content_sha256 = excluded.content_sha256, text = excluded.text, fetched_at = excluded.fetched_at, last_seen = excluded.last_seen",
                params![key, hash, text, now],
            )
            .map_err(map_sqlite)?;
        Ok(())
    }

    fn mark_seen(&mut self, key: &str, now: i64) -> Result<(), StoreError> {
        self.connection
            .execute(
                "UPDATE snapshots SET last_seen = ?2 WHERE snapshot_key = ?1",
                params![key, now],
            )
            .map_err(map_sqlite)?;
        Ok(())
    }

    fn prune(&mut self, now: i64) -> Result<(), StoreError> {
        let retention = self.config.retention_days.saturating_mul(86_400);
        let retention = i64::try_from(retention).unwrap_or(i64::MAX);
        let cutoff = now.saturating_sub(retention);
        self.connection
            .execute("DELETE FROM snapshots WHERE last_seen < ?1", [cutoff])
            .map_err(map_sqlite)?;
        loop {
            let (entries, bytes): (i64, i64) = self
                .connection
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(length(CAST(text AS BLOB))), 0) FROM snapshots",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(map_sqlite)?;
            let max_entries = i64::try_from(self.config.max_entries).unwrap_or(i64::MAX);
            let max_bytes = i64::try_from(self.config.max_bytes).unwrap_or(i64::MAX);
            if entries <= max_entries && bytes <= max_bytes {
                break;
            }
            let oldest: Option<(String, i64)> = self
                .connection
                .query_row(
                    "SELECT snapshot_key, length(CAST(text AS BLOB)) FROM snapshots ORDER BY last_seen, snapshot_key LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(map_sqlite)?;
            let Some((key, _)) = oldest else {
                break;
            };
            self.connection
                .execute("DELETE FROM snapshots WHERE snapshot_key = ?1", [key])
                .map_err(map_sqlite)?;
        }
        Ok(())
    }
}

fn resolve_path(config: &FetchSnapshotSection) -> Result<PathBuf, StoreError> {
    if let Some(path) = &config.path {
        return Ok(path.clone());
    }
    dirs::data_local_dir()
        .map(|path| path.join("eggsearch").join("snapshots.sqlite3"))
        .ok_or(StoreError::Unavailable)
}

fn prepare_path(path: &Path, config: &FetchSnapshotSection) -> Result<(), StoreError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    #[cfg(unix)]
    let parent_existed = std::fs::symlink_metadata(parent).is_ok();
    std::fs::create_dir_all(parent).map_err(|_| StoreError::Unavailable)?;
    #[cfg(unix)]
    protect_directory(parent, parent_existed, config.path.is_some())?;
    let mut file = OpenOptions::new();
    file.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        file.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    file.open(path).map_err(|_| StoreError::Unavailable)?;
    #[cfg(unix)]
    protect_file(path)?;
    Ok(())
}

#[cfg(unix)]
fn protect_directory(path: &Path, existed: bool, explicit_path: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| StoreError::Unavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StoreError::Unavailable);
    }
    let mode = metadata.permissions().mode() & 0o777;
    if explicit_path && existed && mode & 0o077 != 0 {
        return Err(StoreError::Unavailable);
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o700))
        .map_err(|_| StoreError::Unavailable)
}

#[cfg(unix)]
fn protect_file(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = std::fs::symlink_metadata(path).map_err(|_| StoreError::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || crate::process::real_user_id() != Some(metadata.uid())
    {
        return Err(StoreError::Unavailable);
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| StoreError::Unavailable)
}

fn ensure_schema(connection: &Connection) -> Result<(), StoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_sqlite)?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::SchemaUnsupported);
    }
    if version < 0 {
        return Err(StoreError::Corrupt);
    }
    if version == 0 {
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite)?;
        if tables != 0 {
            return Err(StoreError::Corrupt);
        }
        connection
            .execute_batch(
                "CREATE TABLE snapshots (snapshot_key TEXT PRIMARY KEY NOT NULL, content_sha256 TEXT NOT NULL, text TEXT NOT NULL, fetched_at INTEGER NOT NULL, last_seen INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX snapshots_last_seen ON snapshots(last_seen, snapshot_key); PRAGMA user_version = 1;",
            )
            .map_err(map_sqlite)?;
    } else {
        let table: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'snapshots'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        if table.is_none()
            || connection
                .prepare("SELECT snapshot_key, content_sha256, text, fetched_at, last_seen FROM snapshots LIMIT 0")
                .is_err()
        {
            return Err(StoreError::Corrupt);
        }
    }
    Ok(())
}

fn map_sqlite(error: rusqlite::Error) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
            ) =>
        {
            StoreError::Corrupt
        }
        _ => StoreError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(path: &Path) -> FetchSnapshotSection {
        FetchSnapshotSection {
            enabled: true,
            path: Some(path.to_path_buf()),
            max_entries: 10,
            max_bytes: 1_048_576,
            retention_days: 30,
        }
    }

    fn private_directory(path: &Path) -> std::path::PathBuf {
        let directory = path.join("private");
        std::fs::create_dir(&directory).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        directory
    }

    fn identity(scope: CacheScope) -> SnapshotIdentity {
        SnapshotIdentity {
            scope,
            url: "https://example.com/a".into(),
            extract_mode: ExtractMode::Text,
            scoping_fingerprint: None,
            representation_class: "html".into(),
        }
    }

    fn request() -> FetchSnapshotRequest {
        FetchSnapshotRequest {
            compare: crate::core::fetch::SnapshotCompare::Line,
            key: None,
            max_hunks: None,
            max_diff_chars: None,
        }
    }

    #[test]
    fn stores_only_normalized_text_and_reopens_durably() {
        let dir = tempfile::tempdir().unwrap();
        let path = private_directory(dir.path()).join("snapshots.sqlite3");
        let cfg = config(&path);
        let first = compare_text_sync(
            &cfg,
            &request(),
            &identity(CacheScope::Anonymous),
            "Title\nBody\n",
        );
        assert_eq!(first.change.as_ref().unwrap().status, "new");
        let second = compare_text_sync(
            &cfg,
            &request(),
            &identity(CacheScope::Anonymous),
            "Title\nBody",
        );
        assert_eq!(second.change.as_ref().unwrap().status, "same");
        let conn = Connection::open(path).unwrap();
        let stored: String = conn
            .query_row("SELECT text FROM snapshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stored, "Title\nBody");
        assert!(!stored.contains("<html>"));
    }

    #[test]
    fn scope_isolation_uses_opaque_profile_ids() {
        let anon = snapshot_address(&identity(CacheScope::Anonymous), None);
        let one = snapshot_address(&identity(CacheScope::Profile("profile-1".into())), None);
        let two = snapshot_address(&identity(CacheScope::Profile("profile-2".into())), None);
        assert_ne!(anon, one);
        assert_ne!(one, two);
    }

    #[test]
    fn rejects_newer_schema_and_corrupt_database_without_resetting() {
        let dir = tempfile::tempdir().unwrap();
        let private = private_directory(dir.path());
        let path = private.join("newer.sqlite3");
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(conn);
        assert_eq!(
            SnapshotStore::open(&config(&path)).err(),
            Some(StoreError::SchemaUnsupported)
        );
        let corrupt = private.join("corrupt.sqlite3");
        std::fs::write(&corrupt, b"not sqlite").unwrap();
        assert_eq!(
            SnapshotStore::open(&config(&corrupt)).err(),
            Some(StoreError::Corrupt)
        );
        assert_eq!(std::fs::read(corrupt).unwrap(), b"not sqlite");
    }

    #[test]
    fn pruning_removes_oldest_then_lexicographic_key_under_caps() {
        let dir = tempfile::tempdir().unwrap();
        let path = private_directory(dir.path()).join("prune.sqlite3");
        let mut cfg = config(&path);
        cfg.max_entries = 2;
        let mut store = SnapshotStore::open(&cfg).unwrap();
        store.put("b", "b", "b", 1).unwrap();
        store.put("a", "a", "a", 1).unwrap();
        store.put("c", "c", "c", 2).unwrap();
        store.prune(2).unwrap();
        let keys: Vec<String> = store
            .connection
            .prepare("SELECT snapshot_key FROM snapshots ORDER BY snapshot_key")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(keys, ["b", "c"]);
    }
}
