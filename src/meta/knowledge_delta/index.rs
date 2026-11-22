use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use rusqlite::{params, Connection, OptionalExtension};

use super::discovery::{edge_id, extract_subjects, subject_id};
use crate::core::identity::canonicalize_url;
use crate::core::knowledge_delta::KnowledgeIndexConfig;

pub const SCHEMA_VERSION: i64 = 2;
const MAX_SUBJECTS_PER_SOURCE: usize = 16;
const MAX_EDGES_PER_SOURCE: usize = 8;
const MAX_EDGES_PER_SUBJECT: usize = 64;
const MAX_EXPANSION_SUBJECTS: usize = 16;
const MAX_EXPANSION_NEIGHBORS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexRecord {
    pub partition: String,
    pub canonical_url: String,
    pub source_id: String,
    pub title: String,
    pub excerpt: String,
    pub trust: String,
    pub basis: String,
    pub precision: String,
    pub normalized_date: Option<String>,
    pub kdelta_id: String,
    pub conflicted: bool,
    pub temporal_relationship: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexStatus {
    pub documents: u64,
    pub newest_indexed_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexFreshness {
    NeverSeeded,
    Fresh,
    Stale,
}

pub fn classify_index_freshness(
    status: &IndexStatus,
    retention_days: u32,
    now_unix: i64,
) -> IndexFreshness {
    if status.documents == 0 {
        return IndexFreshness::NeverSeeded;
    }
    if retention_days == 0 {
        return IndexFreshness::Fresh;
    }
    let Some(newest_indexed_at) = status.newest_indexed_at else {
        return IndexFreshness::Fresh;
    };
    let cutoff = now_unix.saturating_sub(i64::from(retention_days) * 86_400);
    if newest_indexed_at < cutoff {
        IndexFreshness::Stale
    } else {
        IndexFreshness::Fresh
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexHit {
    pub record: IndexRecord,
    pub edge_source_ids: Vec<String>,
    pub edge_spans: Vec<EdgeSourceSpan>,
    pub expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeSourceSpan {
    pub source_id: String,
    pub span: String,
}

#[derive(Debug)]
pub enum IndexError {
    Unavailable(String),
    NewerSchema(i64),
    Corrupt(String),
    ReadOnly(String),
    Full(String),
    InsecurePermissions,
    InsecureDirectoryPermissions { directory: PathBuf, mode: u32 },
    InvalidPartition,
}

impl IndexError {
    pub fn message(&self) -> String {
        match self {
            Self::Unavailable(detail) => format!("knowledge index unavailable: {detail}"),
            Self::NewerSchema(version) => {
                format!("knowledge index schema {version} is newer than {SCHEMA_VERSION}")
            }
            Self::Corrupt(detail) => format!("knowledge index corrupt: {detail}"),
            Self::ReadOnly(detail) => format!("knowledge index read-only: {detail}"),
            Self::Full(detail) => format!("knowledge index full: {detail}"),
            Self::InsecurePermissions => {
                "knowledge index path must be private to the current user".to_string()
            }
            Self::InsecureDirectoryPermissions { directory, mode } => format!(
                "knowledge index directory {} has mode {:04o}; it must be private to the current user with mode 0700",
                directory.display(),
                mode
            ),
            Self::InvalidPartition => {
                "knowledge index partition is not anonymous or prof_<16 hex>".to_string()
            }
        }
    }
}

pub fn public_index_trust(trust: &str) -> bool {
    trust == "external_untrusted"
}

pub fn valid_partition(partition: &str) -> bool {
    partition == "anonymous"
        || (partition.starts_with("prof_")
            && partition.len() == 21
            && partition.as_bytes()[5..]
                .iter()
                .all(|byte| byte.is_ascii_hexdigit()))
}

pub fn database_path(config: &KnowledgeIndexConfig, config_dir: &Path, partition: &str) -> PathBuf {
    match &config.path {
        Some(path) if path.ends_with(".sqlite") || path.ends_with(".db") => PathBuf::from(path),
        Some(path) => PathBuf::from(path).join(partition).join("index.sqlite"),
        None => config_dir
            .join("knowledge-delta")
            .join(partition)
            .join("index.sqlite"),
    }
}

pub struct EvidenceIndex {
    conn: Connection,
    max_documents: u64,
    max_excerpt_chars: usize,
    retention_days: u32,
}

impl EvidenceIndex {
    pub fn open(path: &Path, config: &KnowledgeIndexConfig) -> Result<Self, IndexError> {
        prepare_index_path(path, config)?;
        let conn = Connection::open(path).map_err(map_sqlite)?;
        #[cfg(unix)]
        protect_database_file(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(500))
            .map_err(map_sqlite)?;
        conn.pragma_update(None, "secure_delete", true)
            .map_err(map_sqlite)?;
        conn.pragma_update(None, "journal_mode", "DELETE")
            .map_err(map_sqlite)?;
        let page_size: i64 = conn
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(map_sqlite)?;
        let max_pages = (config.max_bytes / page_size.max(1) as u64).max(1) as i64;
        conn.pragma_update(None, "max_page_count", max_pages)
            .map_err(map_sqlite)?;
        let page_count: i64 = conn
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .map_err(map_sqlite)?;
        if page_count > max_pages {
            return Err(IndexError::Full(format!(
                "database already uses {} bytes, exceeding max_bytes {}",
                page_count.saturating_mul(page_size),
                config.max_bytes
            )));
        }
        ensure_schema(&conn)?;
        Ok(Self {
            conn,
            max_documents: config.max_documents,
            max_excerpt_chars: config.max_excerpt_chars,
            retention_days: config.retention_days,
        })
    }

    pub fn retain(&self, records: &[IndexRecord]) -> Result<(), IndexError> {
        let tx = self.conn.unchecked_transaction().map_err(map_sqlite)?;
        self.expire(&tx)?;
        for record in records {
            if !valid_partition(&record.partition) {
                return Err(IndexError::InvalidPartition);
            }
            if !public_index_trust(&record.trust) {
                return Err(IndexError::Unavailable(
                    "refusing to index a private, no-store, pass-cookie, or profile-scoped body"
                        .into(),
                ));
            }
            self.insert_record(&tx, record)?;
        }
        self.evict(&tx)?;
        tx.commit().map_err(map_sqlite)?;
        Ok(())
    }

    pub fn status(&self, partition: &str) -> Result<IndexStatus, IndexError> {
        if !valid_partition(partition) {
            return Err(IndexError::InvalidPartition);
        }
        let (documents, newest_indexed_at): (i64, Option<i64>) = self
            .conn
            .query_row(
                "SELECT COUNT(*), MAX(indexed_at) FROM candidate_events WHERE partition = ?1",
                [partition],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_sqlite)?;
        let documents = u64::try_from(documents)
            .map_err(|_| IndexError::Corrupt("document count is negative".into()))?;
        Ok(IndexStatus {
            documents,
            newest_indexed_at,
        })
    }

    pub fn lookup(
        &self,
        partition: &str,
        topic: &str,
        limit: usize,
    ) -> Result<Vec<IndexHit>, IndexError> {
        if !valid_partition(partition) {
            return Err(IndexError::InvalidPartition);
        }
        let Some(query) = build_fts_query(topic) else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare(
            "SELECT c.canonical_url, c.source_id, c.title, c.excerpt, c.trust, c.basis, c.precision, c.normalized_date, c.kdelta_id, c.conflicted, c.temporal_relationship
             FROM candidate_events_fts f
             JOIN candidate_events c ON c.rowid = f.rowid AND c.partition = f.partition
             WHERE candidate_events_fts MATCH ?1 AND f.partition = ?2
             ORDER BY bm25(candidate_events_fts), c.kdelta_id LIMIT ?3",
        ).map_err(map_sqlite)?;
        let rows = stmt
            .query_map(params![query, partition, limit as i64], |row| {
                Ok(IndexHit {
                    record: row_record(partition, row)?,
                    edge_source_ids: Vec::new(),
                    edge_spans: Vec::new(),
                    expanded: false,
                })
            })
            .map_err(map_sqlite)?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row.map_err(map_sqlite)?);
        }
        Ok(hits)
    }

    pub fn expand(
        &self,
        partition: &str,
        subjects: &[String],
        limit: usize,
    ) -> Result<Vec<IndexHit>, IndexError> {
        if !valid_partition(partition) {
            return Err(IndexError::InvalidPartition);
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut neighbors: BTreeMap<String, Vec<EdgeSourceSpan>> = BTreeMap::new();
        for source in subjects.iter().take(MAX_EXPANSION_SUBJECTS) {
            if neighbors.len() >= MAX_EXPANSION_NEIGHBORS {
                break;
            }
            let edges: Vec<(String, EdgeSourceSpan)> = {
                let mut stmt = self.conn.prepare(
                    "SELECT to_subject, source_id, span FROM subject_evidence_edges WHERE partition = ?1 AND from_subject = ?2 ORDER BY edge_id LIMIT ?3",
                ).map_err(map_sqlite)?;
                let rows = stmt
                    .query_map(
                        params![partition, source, MAX_EDGES_PER_SUBJECT as i64],
                        |row| {
                            Ok((
                                row.get(0)?,
                                EdgeSourceSpan {
                                    source_id: row.get(1)?,
                                    span: row.get(2)?,
                                },
                            ))
                        },
                    )
                    .map_err(map_sqlite)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(map_sqlite)?
            };
            for (neighbor, span) in edges {
                if !neighbors.contains_key(&neighbor) && neighbors.len() >= MAX_EXPANSION_NEIGHBORS
                {
                    break;
                }
                let spans = neighbors.entry(neighbor).or_default();
                if !spans.contains(&span) {
                    spans.push(span);
                }
            }
        }
        let mut hits: Vec<IndexHit> = Vec::new();
        for (neighbor, edge_spans) in neighbors {
            if hits.len() >= limit {
                break;
            }
            let Some(query) = build_fts_query(&neighbor) else {
                continue;
            };
            let mut stmt = self.conn.prepare(
                "SELECT c.canonical_url, c.source_id, c.title, c.excerpt, c.trust, c.basis, c.precision, c.normalized_date, c.kdelta_id, c.conflicted, c.temporal_relationship
                 FROM candidate_events_fts
                 JOIN candidate_events c ON c.rowid = candidate_events_fts.rowid
                 WHERE candidate_events_fts MATCH ?1 AND c.partition = ?2 AND c.primary_subject = ?3
                 ORDER BY bm25(candidate_events_fts), c.kdelta_id LIMIT ?4",
            ).map_err(map_sqlite)?;
            let rows = stmt
                .query_map(
                    params![
                        query,
                        partition,
                        neighbor,
                        limit.saturating_sub(hits.len()) as i64
                    ],
                    |row| {
                        let edge_source_ids = edge_spans
                            .iter()
                            .map(|span| span.source_id.clone())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect();
                        Ok(IndexHit {
                            record: row_record(partition, row)?,
                            edge_source_ids,
                            edge_spans: edge_spans.clone(),
                            expanded: true,
                        })
                    },
                )
                .map_err(map_sqlite)?;
            for row in rows {
                let hit = row.map_err(map_sqlite)?;
                if !hits
                    .iter()
                    .any(|existing| existing.record.kdelta_id == hit.record.kdelta_id)
                {
                    hits.push(hit);
                }
            }
        }
        hits.sort_by(|left, right| left.record.kdelta_id.cmp(&right.record.kdelta_id));
        hits.truncate(limit);
        Ok(hits)
    }

    fn insert_record(&self, tx: &Connection, record: &IndexRecord) -> Result<(), IndexError> {
        let canonical_url = canonicalize_url(&record.canonical_url);
        let excerpt = clip(&record.excerpt, self.max_excerpt_chars);
        let title = clip(&record.title, 512);
        let topic = "";
        let subjects: Vec<_> = extract_subjects(&title, &excerpt)
            .into_iter()
            .take(MAX_SUBJECTS_PER_SOURCE)
            .collect();
        let primary_subject = subjects
            .iter()
            .find(|subject| subject.verified)
            .map(|subject| subject.token.clone());
        let now = unix_now();
        tx.execute(
            "INSERT OR REPLACE INTO documents (partition, canonical_url, source_id, title, excerpt, trust, basis, precision, normalized_date, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![record.partition, canonical_url, record.source_id, title, excerpt, record.trust, record.basis, record.precision, record.normalized_date, now],
        ).map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM candidate_events_fts WHERE rowid IN (SELECT rowid FROM candidate_events WHERE partition = ?1 AND kdelta_id = ?2)",
            params![record.partition, record.kdelta_id],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM subject_evidence_edges WHERE partition = ?1 AND source_id = ?2",
            params![record.partition, record.source_id],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM subjects WHERE partition = ?1 AND source_id = ?2",
            params![record.partition, record.source_id],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "INSERT OR REPLACE INTO candidate_events (partition, kdelta_id, canonical_url, source_id, title, excerpt, trust, basis, precision, normalized_date, conflicted, temporal_relationship, topic, primary_subject, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![record.partition, record.kdelta_id, canonical_url, record.source_id, title, excerpt, record.trust, record.basis, record.precision, record.normalized_date, i64::from(record.conflicted), record.temporal_relationship, topic, primary_subject, now],
        ).map_err(map_sqlite)?;
        let rowid: i64 = tx
            .query_row(
                "SELECT rowid FROM candidate_events WHERE partition = ?1 AND kdelta_id = ?2",
                params![record.partition, record.kdelta_id],
                |row| row.get(0),
            )
            .map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM candidate_events_fts WHERE rowid = ?1",
            params![rowid],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "INSERT INTO candidate_events_fts (rowid, partition, kdelta_id, topic, title, excerpt, primary_subject) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![rowid, record.partition, record.kdelta_id, topic, title, excerpt, primary_subject],
        )
        .map_err(map_sqlite)?;
        let from = primary_subject.clone().unwrap_or_default();
        for subject in &subjects {
            let id = subject_id(subject.kind, &subject.token);
            tx.execute(
                "INSERT OR REPLACE INTO subjects (partition, subject_id, kind, token, verified, source_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![record.partition, id, subject.kind, subject.token, i64::from(subject.verified), record.source_id],
            ).map_err(map_sqlite)?;
        }
        for subject in subjects
            .into_iter()
            .filter(|item| item.verified)
            .take(MAX_EDGES_PER_SOURCE)
        {
            if from.is_empty() || subject.token.eq_ignore_ascii_case(&from) {
                continue;
            }
            let edge = edge_id(&record.source_id, &from, &subject.token, &subject.span);
            tx.execute(
                "INSERT OR REPLACE INTO subject_evidence_edges (partition, edge_id, from_subject, to_subject, source_id, span) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![record.partition, edge, from, subject.token, record.source_id, subject.span],
            ).map_err(map_sqlite)?;
            tx.execute(
                "DELETE FROM subject_evidence_edges WHERE partition = ?1 AND from_subject = ?2 AND edge_id NOT IN (SELECT edge_id FROM subject_evidence_edges WHERE partition = ?1 AND from_subject = ?2 ORDER BY edge_id LIMIT ?3)",
                params![record.partition, from, MAX_EDGES_PER_SUBJECT as i64],
            ).map_err(map_sqlite)?;
        }
        Ok(())
    }

    fn expire(&self, tx: &Connection) -> Result<(), IndexError> {
        if self.retention_days == 0 {
            return Ok(());
        }
        let cutoff = unix_now().saturating_sub(i64::from(self.retention_days) * 86_400);
        tx.execute(
            "DELETE FROM candidate_events_fts WHERE rowid IN (SELECT rowid FROM candidate_events WHERE indexed_at < ?1)",
            params![cutoff],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM candidate_events WHERE indexed_at < ?1",
            params![cutoff],
        )
        .map_err(map_sqlite)?;
        cleanup_orphans(tx)?;
        tx.execute(
            "DELETE FROM documents WHERE indexed_at < ?1",
            params![cutoff],
        )
        .map_err(map_sqlite)?;
        Ok(())
    }

    fn evict(&self, tx: &Connection) -> Result<(), IndexError> {
        let count: i64 = tx
            .query_row("SELECT COUNT(*) FROM candidate_events", [], |row| {
                row.get(0)
            })
            .map_err(map_sqlite)?;
        if count as u64 <= self.max_documents {
            return Ok(());
        }
        let extra = count - self.max_documents as i64;
        tx.execute(
            "DELETE FROM candidate_events_fts WHERE rowid IN (SELECT rowid FROM candidate_events ORDER BY indexed_at, kdelta_id LIMIT ?1)",
            params![extra],
        ).map_err(map_sqlite)?;
        tx.execute(
            "DELETE FROM candidate_events WHERE rowid IN (SELECT rowid FROM candidate_events ORDER BY indexed_at, kdelta_id LIMIT ?1)",
            params![extra],
        ).map_err(map_sqlite)?;
        cleanup_orphans(tx)?;
        Ok(())
    }
}

fn prepare_index_path(path: &Path, config: &KnowledgeIndexConfig) -> Result<(), IndexError> {
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        let parent_existed = std::fs::symlink_metadata(parent).is_ok();
        std::fs::create_dir_all(parent).map_err(map_io)?;
        #[cfg(unix)]
        protect_index_directory(parent, parent_existed, config)?;
    }
    #[cfg(unix)]
    protect_database_file(path)?;
    Ok(())
}

#[cfg(unix)]
fn protect_index_directory(
    path: &Path,
    existed: bool,
    config: &KnowledgeIndexConfig,
) -> Result<(), IndexError> {
    let metadata = std::fs::symlink_metadata(path).map_err(map_io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(IndexError::InsecurePermissions);
    }
    let mode = metadata.permissions().mode() & 0o777;
    let explicit_file = config
        .path
        .as_deref()
        .is_some_and(|configured| configured.ends_with(".sqlite") || configured.ends_with(".db"));
    if explicit_file && existed && mode & 0o077 != 0 {
        return Err(IndexError::InsecureDirectoryPermissions {
            directory: path.to_path_buf(),
            mode,
        });
    }
    if !explicit_file || !existed {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o700))
            .map_err(map_io)?;
    }
    Ok(())
}

#[cfg(unix)]
fn protect_database_file(path: &Path) -> Result<(), IndexError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true).mode(0o600);
    match options.open(path) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(map_io(error)),
    }
    let metadata = std::fs::symlink_metadata(path).map_err(map_io)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(IndexError::InsecurePermissions);
    }
    let mode = metadata.permissions().mode() & 0o600;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(map_io)
}

fn build_fts_query(input: &str) -> Option<String> {
    let mut terms = BTreeSet::new();
    for token in input.split(|ch: char| !ch.is_alphanumeric()) {
        if token.chars().count() >= 2 {
            terms.insert(token.to_lowercase());
        }
        if terms.len() >= MAX_EXPANSION_SUBJECTS {
            break;
        }
    }
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .into_iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

fn cleanup_orphans(tx: &Connection) -> Result<(), IndexError> {
    tx.execute(
        "DELETE FROM subject_evidence_edges WHERE NOT EXISTS (SELECT 1 FROM candidate_events c WHERE c.partition = subject_evidence_edges.partition AND c.source_id = subject_evidence_edges.source_id)",
        [],
    )
    .map_err(map_sqlite)?;
    tx.execute(
        "DELETE FROM documents WHERE NOT EXISTS (SELECT 1 FROM candidate_events c WHERE c.partition = documents.partition AND c.canonical_url = documents.canonical_url AND c.basis = documents.basis AND c.precision = documents.precision AND c.normalized_date IS documents.normalized_date)",
        [],
    )
    .map_err(map_sqlite)?;
    tx.execute(
        "DELETE FROM subjects WHERE NOT EXISTS (SELECT 1 FROM candidate_events c WHERE c.partition = subjects.partition AND c.source_id = subjects.source_id)",
        [],
    )
    .map_err(map_sqlite)?;
    Ok(())
}

fn enable_fts5_secure_delete(conn: &Connection) -> Result<(), IndexError> {
    let configured: Option<i64> = conn
        .query_row(
            "SELECT v FROM candidate_events_fts_config WHERE k = 'secure-delete'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if configured != Some(1) {
        conn.execute(
            "INSERT INTO candidate_events_fts(candidate_events_fts, rank) VALUES('secure-delete', 1)",
            [],
        )
        .map_err(map_sqlite)?;
    }
    Ok(())
}

fn migrate_schema_v1(conn: &Connection) -> Result<(), IndexError> {
    enable_fts5_secure_delete(conn)?;
    let tx = conn.unchecked_transaction().map_err(map_sqlite)?;
    tx.execute(
        "UPDATE candidate_events SET title = '' WHERE topic <> '' AND title = topic",
        [],
    )
    .map_err(map_sqlite)?;
    tx.execute(
        "UPDATE documents SET title = '' WHERE EXISTS (SELECT 1 FROM candidate_events c WHERE c.partition = documents.partition AND c.canonical_url = documents.canonical_url AND c.basis = documents.basis AND c.precision = documents.precision AND c.normalized_date IS documents.normalized_date AND c.topic <> '' AND documents.title = c.topic)",
        [],
    )
    .map_err(map_sqlite)?;
    tx.execute(
        "UPDATE candidate_events SET topic = '', primary_subject = NULL",
        [],
    )
    .map_err(map_sqlite)?;
    tx.execute("DELETE FROM candidate_events_fts", [])
        .map_err(map_sqlite)?;
    tx.execute("DELETE FROM subject_evidence_edges", [])
        .map_err(map_sqlite)?;
    tx.execute("DELETE FROM subjects", []).map_err(map_sqlite)?;
    let records = {
        let mut statement = tx
            .prepare("SELECT rowid, partition, source_id, title, excerpt FROM candidate_events")
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(map_sqlite)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(map_sqlite)?
    };
    for (rowid, partition, source_id, title, excerpt) in records {
        let subjects: Vec<_> = extract_subjects(&title, &excerpt)
            .into_iter()
            .take(MAX_SUBJECTS_PER_SOURCE)
            .collect();
        let primary = subjects
            .iter()
            .find(|subject| subject.verified)
            .map(|subject| subject.token.clone());
        tx.execute(
            "UPDATE candidate_events SET primary_subject = ?1 WHERE rowid = ?2",
            params![primary, rowid],
        )
        .map_err(map_sqlite)?;
        for subject in &subjects {
            let id = subject_id(subject.kind, &subject.token);
            tx.execute(
                "INSERT OR REPLACE INTO subjects (partition, subject_id, kind, token, verified, source_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![partition, id, subject.kind, subject.token, i64::from(subject.verified), source_id],
            )
            .map_err(map_sqlite)?;
        }
        if let Some(from) = primary {
            for subject in subjects
                .into_iter()
                .filter(|subject| subject.verified)
                .take(MAX_EDGES_PER_SOURCE)
            {
                if subject.token.eq_ignore_ascii_case(&from) {
                    continue;
                }
                let edge = edge_id(&source_id, &from, &subject.token, &subject.span);
                tx.execute(
                    "INSERT OR REPLACE INTO subject_evidence_edges (partition, edge_id, from_subject, to_subject, source_id, span) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![partition, edge, from, subject.token, source_id, subject.span],
                )
                .map_err(map_sqlite)?;
            }
        }
    }
    tx.execute_batch(
        "INSERT INTO candidate_events_fts (rowid, partition, kdelta_id, topic, title, excerpt, primary_subject) SELECT rowid, partition, kdelta_id, topic, title, excerpt, primary_subject FROM candidate_events;
         UPDATE schema_meta SET value = '2' WHERE key = 'version';",
    )
    .map_err(map_sqlite)?;
    tx.commit().map_err(map_sqlite)
}

fn ensure_schema(conn: &Connection) -> Result<(), IndexError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
    .map_err(map_sqlite)?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if let Some(raw) = existing {
        let version = raw
            .parse::<i64>()
            .map_err(|_| IndexError::Corrupt("schema version is not an integer".into()))?;
        if version > SCHEMA_VERSION {
            return Err(IndexError::NewerSchema(version));
        }
        if version == 1 {
            migrate_schema_v1(conn)?;
            return Ok(());
        }
        if version < SCHEMA_VERSION {
            return Err(IndexError::Corrupt(format!(
                "schema version {version} is older than {SCHEMA_VERSION}; refusing to delete rows"
            )));
        }
        enable_fts5_secure_delete(conn)?;
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE documents (
           partition TEXT NOT NULL,
           canonical_url TEXT NOT NULL,
           source_id TEXT NOT NULL,
           title TEXT NOT NULL,
           excerpt TEXT NOT NULL,
           trust TEXT NOT NULL,
           basis TEXT NOT NULL,
           precision TEXT NOT NULL,
           normalized_date TEXT,
           indexed_at INTEGER NOT NULL,
           PRIMARY KEY (partition, canonical_url, basis, precision, normalized_date)
         );
         CREATE TABLE candidate_events (
           partition TEXT NOT NULL,
           kdelta_id TEXT NOT NULL,
           canonical_url TEXT NOT NULL,
           source_id TEXT NOT NULL,
           title TEXT NOT NULL,
           excerpt TEXT NOT NULL,
           trust TEXT NOT NULL,
           basis TEXT NOT NULL,
           precision TEXT NOT NULL,
           normalized_date TEXT,
           conflicted INTEGER NOT NULL,
           temporal_relationship TEXT NOT NULL,
           topic TEXT NOT NULL,
           primary_subject TEXT,
           indexed_at INTEGER NOT NULL,
           PRIMARY KEY (partition, kdelta_id)
         );
         CREATE TABLE subjects (
           partition TEXT NOT NULL,
           subject_id TEXT NOT NULL,
           kind TEXT NOT NULL,
           token TEXT NOT NULL,
           verified INTEGER NOT NULL,
           source_id TEXT NOT NULL,
           PRIMARY KEY (partition, subject_id, source_id)
         );
         CREATE TABLE subject_evidence_edges (
           partition TEXT NOT NULL,
           edge_id TEXT NOT NULL,
           from_subject TEXT NOT NULL,
           to_subject TEXT NOT NULL,
           source_id TEXT NOT NULL,
           span TEXT NOT NULL,
           PRIMARY KEY (partition, edge_id)
         );
         CREATE VIRTUAL TABLE candidate_events_fts USING fts5(partition UNINDEXED, kdelta_id UNINDEXED, topic, title, excerpt, primary_subject, tokenize = 'unicode61 remove_diacritics 2');
         INSERT INTO schema_meta (key, value) VALUES ('version', '2');
         COMMIT;",
    )
    .map_err(map_sqlite)?;
    enable_fts5_secure_delete(conn)?;
    Ok(())
}

fn row_record(partition: &str, row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexRecord> {
    Ok(IndexRecord {
        partition: partition.to_string(),
        canonical_url: row.get(0)?,
        source_id: row.get(1)?,
        title: row.get(2)?,
        excerpt: row.get(3)?,
        trust: row.get(4)?,
        basis: row.get(5)?,
        precision: row.get(6)?,
        normalized_date: row.get(7)?,
        kdelta_id: row.get(8)?,
        conflicted: row.get::<_, i64>(9)? != 0,
        temporal_relationship: row.get(10)?,
    })
}

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn map_io(err: std::io::Error) -> IndexError {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
        IndexError::ReadOnly(err.to_string())
    } else {
        IndexError::Unavailable(err.to_string())
    }
}

fn map_sqlite(err: rusqlite::Error) -> IndexError {
    match &err {
        rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::ReadOnly => {
            IndexError::ReadOnly(err.to_string())
        }
        rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::DiskFull => {
            IndexError::Full(err.to_string())
        }
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::DatabaseCorrupt =>
        {
            IndexError::Corrupt(err.to_string())
        }
        _ => IndexError::Unavailable(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> KnowledgeIndexConfig {
        KnowledgeIndexConfig {
            enabled: true,
            path: None,
            max_bytes: 1024 * 1024,
            max_documents: 2,
            max_excerpt_chars: 40,
            retention_days: 365,
        }
    }

    fn record(id: &str) -> IndexRecord {
        IndexRecord {
            partition: "anonymous".into(),
            canonical_url: format!("https://example.com/{id}"),
            source_id: format!("src_{id}"),
            title: "widget-runtime notes".into(),
            excerpt: "see left-pad only as prose".into(),
            trust: "external_untrusted".into(),
            basis: "publication".into(),
            precision: "day".into(),
            normalized_date: Some("2024-07-02".into()),
            kdelta_id: format!("kdelta_{id}"),
            conflicted: false,
            temporal_relationship: "after_cutoff".into(),
        }
    }

    #[test]
    fn retain_is_idempotent_and_evicts_deterministically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let index = EvidenceIndex::open(&path, &config()).unwrap();
        index.retain(&[record("a")]).unwrap();
        index
            .retain(&[record("a"), record("b"), record("c")])
            .unwrap();
        let hits = index.lookup("anonymous", "widget-runtime", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits
            .iter()
            .all(|hit| hit.record.kdelta_id != "kdelta_a" || hits.len() == 2));
        let again = EvidenceIndex::open(&path, &config()).unwrap();
        assert_eq!(
            again
                .lookup("anonymous", "widget-runtime", 10)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn newer_schema_is_refused_without_deleting_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let index = EvidenceIndex::open(&path, &config()).unwrap();
        index.retain(&[record("a")]).unwrap();
        drop(index);
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE schema_meta SET value = '9' WHERE key = 'version'",
            [],
        )
        .unwrap();
        drop(conn);
        match EvidenceIndex::open(&path, &config()) {
            Err(IndexError::NewerSchema(9)) => {}
            Err(other) => panic!("newer schema must be refused: {}", other.message()),
            Ok(_) => panic!("newer schema must be refused"),
        }
        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM candidate_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn freshness_classifies_never_seeded_index() {
        assert_eq!(
            classify_index_freshness(
                &IndexStatus {
                    documents: 0,
                    newest_indexed_at: None,
                },
                30,
                1_000_000,
            ),
            IndexFreshness::NeverSeeded
        );
    }

    #[test]
    fn freshness_with_zero_retention_is_always_fresh() {
        assert_eq!(
            classify_index_freshness(
                &IndexStatus {
                    documents: 1,
                    newest_indexed_at: Some(1),
                },
                0,
                1_000_000,
            ),
            IndexFreshness::Fresh
        );
    }

    #[test]
    fn freshness_at_retention_boundary_is_fresh() {
        let now = 1_000_000;
        let boundary = now - 2 * 86_400;
        assert_eq!(
            classify_index_freshness(
                &IndexStatus {
                    documents: 1,
                    newest_indexed_at: Some(boundary),
                },
                2,
                now,
            ),
            IndexFreshness::Fresh
        );
    }

    #[test]
    fn freshness_just_past_retention_boundary_is_stale() {
        let now = 1_000_000;
        let boundary = now - 2 * 86_400;
        assert_eq!(
            classify_index_freshness(
                &IndexStatus {
                    documents: 1,
                    newest_indexed_at: Some(boundary - 1),
                },
                2,
                now,
            ),
            IndexFreshness::Stale
        );
    }

    #[test]
    fn status_counts_empty_and_retained_partitions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        let index = EvidenceIndex::open(&path, &config()).unwrap();
        assert_eq!(
            index.status("anonymous").unwrap(),
            IndexStatus {
                documents: 0,
                newest_indexed_at: None,
            }
        );
        index.retain(&[record("status")]).unwrap();
        let retained = index.status("anonymous").unwrap();
        assert_eq!(retained.documents, 1);
        assert!(retained.newest_indexed_at.is_some());
        assert_eq!(index.status("prof_0123456789abcdef").unwrap().documents, 0);
    }
}
