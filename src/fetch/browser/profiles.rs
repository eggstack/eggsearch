use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const PROFILE_SCHEMA_VERSION: u32 = 1;
pub const MAX_DISPLAY_NAME_LEN: usize = 64;
pub const MIN_DISPLAY_NAME_LEN: usize = 1;
pub const MAX_PROFILE_COUNT: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserProfileMetadata {
    pub id: String,
    pub display_name: String,
    pub allowed_origin: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub browser_family: String,
    pub browser_major_version: Option<u32>,
    pub schema_version: u32,
}

#[derive(Debug)]
pub enum ProfileError {
    NameInvalid(String),
    NameTooLong,
    NameTooShort,
    OriginRequired,
    OriginInvalid(String),
    ProfileNotFound(String),
    ProfileBusy(String),
    ProfileIncomplete(String),
    ProfileIncompatible {
        name: String,
        profile_version: u32,
        browser_version: u32,
    },
    ProfilesDisabled,
    ProfileLimitReached,
    IoError(String),
    SymlinkDetected(String),
    PathEscape(String),
    MetadataCorrupt(String),
    ConfigDirUnavailable,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NameInvalid(name) => write!(
                f,
                "profile name '{name}' contains invalid characters; \
                 use ASCII letters, digits, hyphen, or underscore"
            ),
            Self::NameTooLong => write!(
                f,
                "profile name exceeds maximum length of {MAX_DISPLAY_NAME_LEN}"
            ),
            Self::NameTooShort => write!(
                f,
                "profile name must be at least {MIN_DISPLAY_NAME_LEN} character"
            ),
            Self::OriginRequired => {
                write!(f, "an HTTP(S) origin is required to create a profile")
            }
            Self::OriginInvalid(origin) => {
                write!(f, "invalid origin '{origin}'; must be an HTTP(S) origin")
            }
            Self::ProfileNotFound(name) => {
                write!(f, "browser profile '{name}' not found")
            }
            Self::ProfileBusy(name) => {
                write!(
                    f,
                    "browser profile '{name}' is busy (locked by another process)"
                )
            }
            Self::ProfileIncomplete(name) => {
                write!(f, "browser profile '{name}' is incomplete or corrupted")
            }
            Self::ProfileIncompatible {
                name,
                profile_version,
                browser_version,
            } => {
                write!(
                    f,
                    "browser profile '{name}' was created with browser major version \
                     {profile_version} but current browser is version {browser_version}; \
                     remove and recreate the profile"
                )
            }
            Self::ProfilesDisabled => {
                write!(
                    f,
                    "persistent browser profiles are disabled; \
                     enable [fetch.browser].persistent_profiles_enabled"
                )
            }
            Self::ProfileLimitReached => {
                write!(
                    f,
                    "maximum number of browser profiles ({MAX_PROFILE_COUNT}) reached"
                )
            }
            Self::IoError(e) => write!(f, "profile I/O error: {e}"),
            Self::SymlinkDetected(path) => {
                write!(
                    f,
                    "symlink detected at '{path}'; profile directories must not be symlinks"
                )
            }
            Self::PathEscape(path) => {
                write!(
                    f,
                    "path '{path}' escapes the profile root; profile data must stay within its directory"
                )
            }
            Self::MetadataCorrupt(path) => {
                write!(f, "profile metadata at '{path}' is corrupt or unreadable")
            }
            Self::ConfigDirUnavailable => {
                write!(f, "cannot determine platform application-data directory")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

pub type ProfileResult<T> = Result<T, ProfileError>;

fn validate_display_name(name: &str) -> ProfileResult<()> {
    if name.len() < MIN_DISPLAY_NAME_LEN {
        return Err(ProfileError::NameTooShort);
    }
    if name.len() > MAX_DISPLAY_NAME_LEN {
        return Err(ProfileError::NameTooLong);
    }
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            return Err(ProfileError::NameInvalid(name.to_string()));
        }
    }
    if name.starts_with('.') || name.ends_with('.') {
        return Err(ProfileError::NameInvalid(name.to_string()));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(ProfileError::NameInvalid(name.to_string()));
    }
    Ok(())
}

fn normalize_origin(origin: &str) -> ProfileResult<String> {
    let origin = origin.trim().to_string();
    if origin.is_empty() {
        return Err(ProfileError::OriginRequired);
    }
    let parsed =
        url::Url::parse(&origin).map_err(|_| ProfileError::OriginInvalid(origin.clone()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ProfileError::OriginInvalid(origin.clone()));
    }
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err(ProfileError::OriginInvalid(format!(
            "{origin} (paths not allowed; use origin only)"
        )));
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err(ProfileError::OriginInvalid(format!(
            "{origin} (credentials not allowed in origin)"
        )));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| ProfileError::OriginInvalid(origin.clone()))?;
    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost" {
        return Err(ProfileError::OriginInvalid(format!(
            "{origin} (localhost not allowed)"
        )));
    }
    if host_lower == "127.0.0.1" || host_lower == "::1" {
        return Err(ProfileError::OriginInvalid(format!(
            "{origin} (loopback not allowed)"
        )));
    }
    let port = parsed.port_or_known_default().unwrap_or(80);
    let scheme = parsed.scheme();
    Ok(format!("{scheme}://{host_lower}:{port}"))
}

fn opaque_id(display_name: &str, origin: &str) -> String {
    use crate::core::identity::FnvHasher;
    let mut hasher = FnvHasher::new();
    hasher.write(display_name.as_bytes());
    hasher.write(b"\0");
    hasher.write(origin.as_bytes());
    format!("prof_{:016x}", hasher.finish())
}

fn profile_root_dir() -> ProfileResult<PathBuf> {
    if let Some(data_dir) = dirs::data_dir() {
        Ok(data_dir.join("eggsearch").join("browser-profiles"))
    } else {
        Err(ProfileError::ConfigDirUnavailable)
    }
}

fn validate_not_symlink(path: &Path) -> ProfileResult<()> {
    if path.exists() && path.is_symlink() {
        return Err(ProfileError::SymlinkDetected(path.display().to_string()));
    }
    Ok(())
}

fn validate_inside_root(root: &Path, target: &Path) -> ProfileResult<()> {
    let canonical_root = fs::canonicalize(root)
        .map_err(|e| ProfileError::IoError(format!("cannot canonicalize root: {e}")))?;
    let canonical_target = fs::canonicalize(target)
        .map_err(|e| ProfileError::IoError(format!("cannot canonicalize target: {e}")))?;
    if !canonical_target.starts_with(&canonical_root) {
        return Err(ProfileError::PathEscape(target.display().to_string()));
    }
    Ok(())
}

pub struct ProfileLock {
    lock_path: PathBuf,
    lock_file: Option<fs::File>,
}

impl ProfileLock {
    fn new(profile_dir: &Path) -> Self {
        Self {
            lock_path: profile_dir.join(".lock"),
            lock_file: None,
        }
    }

    pub fn try_acquire(&mut self) -> ProfileResult<bool> {
        validate_not_symlink(&self.lock_path)?;

        match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
        {
            Ok(file) => {
                #[cfg(unix)]
                {
                    use std::os::unix::io::AsRawFd;
                    let fd = file.as_raw_fd();
                    let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
                    if ret != 0 {
                        return Ok(false);
                    }
                }
                self.lock_file = Some(file);
                Ok(true)
            }
            Err(e) => Err(ProfileError::IoError(format!(
                "failed to open lock file: {e}"
            ))),
        }
    }
}

impl Drop for ProfileLock {
    fn drop(&mut self) {
        if self.lock_file.is_some() {
            #[cfg(unix)]
            {
                if let Some(ref file) = self.lock_file {
                    use std::os::unix::io::AsRawFd;
                    let fd = file.as_raw_fd();
                    unsafe {
                        libc::flock(fd, libc::LOCK_UN);
                    }
                }
            }
            self.lock_file = None;
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

pub fn parse_browser_major_version(version_str: &str) -> Option<u32> {
    let digit_start = version_str.find(|c: char| c.is_ascii_digit())?;
    let rest = &version_str[digit_start..];
    let digit_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..digit_end].parse::<u32>().ok()
}

pub struct ProfileManager {
    root_dir: PathBuf,
    profiles_enabled: bool,
    allowed_profiles: Vec<String>,
}

impl ProfileManager {
    pub fn new(
        profiles_dir: Option<&str>,
        profiles_enabled: bool,
        allowed_profiles: Vec<String>,
    ) -> ProfileResult<Self> {
        let root_dir = if let Some(dir) = profiles_dir {
            if dir.is_empty() {
                profile_root_dir()?
            } else {
                PathBuf::from(dir)
            }
        } else {
            profile_root_dir()?
        };

        if profiles_enabled {
            fs::create_dir_all(&root_dir).map_err(|e| {
                ProfileError::IoError(format!("failed to create profiles dir: {e}"))
            })?;
            validate_not_symlink(&root_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o700);
                fs::set_permissions(&root_dir, perms).map_err(|e| {
                    ProfileError::IoError(format!("failed to set profiles dir permissions: {e}"))
                })?;
            }
        }

        Ok(Self {
            root_dir,
            profiles_enabled,
            allowed_profiles,
        })
    }

    pub fn profiles_enabled(&self) -> bool {
        self.profiles_enabled
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn create_profile(
        &self,
        display_name: &str,
        origin: &str,
    ) -> ProfileResult<BrowserProfileMetadata> {
        if !self.profiles_enabled {
            return Err(ProfileError::ProfilesDisabled);
        }

        validate_display_name(display_name)?;
        let normalized_origin = normalize_origin(origin)?;

        if !self.allowed_profiles.is_empty()
            && !self.allowed_profiles.iter().any(|p| p == display_name)
        {
            return Err(ProfileError::ProfileNotFound(format!(
                "{display_name} is not in the allowed_profiles list"
            )));
        }

        let id = opaque_id(display_name, &normalized_origin);
        let profile_dir = self.root_dir.join(&id);

        if profile_dir.exists() {
            return self.load_metadata(&id);
        }

        let existing = self.list_profiles()?;
        if existing.len() >= MAX_PROFILE_COUNT {
            return Err(ProfileError::ProfileLimitReached);
        }

        fs::create_dir_all(&profile_dir)
            .map_err(|e| ProfileError::IoError(format!("failed to create profile dir: {e}")))?;
        validate_not_symlink(&profile_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o700);
            fs::set_permissions(&profile_dir, perms).map_err(|e| {
                ProfileError::IoError(format!("failed to set profile dir permissions: {e}"))
            })?;
        }

        let metadata = BrowserProfileMetadata {
            id: id.clone(),
            display_name: display_name.to_string(),
            allowed_origin: normalized_origin,
            created_at: Utc::now(),
            last_used_at: None,
            browser_family: String::new(),
            browser_major_version: None,
            schema_version: PROFILE_SCHEMA_VERSION,
        };

        self.write_metadata(&profile_dir, &metadata)?;

        let chrome_data = profile_dir.join("chrome-data");
        fs::create_dir_all(&chrome_data)
            .map_err(|e| ProfileError::IoError(format!("failed to create chrome-data dir: {e}")))?;

        Ok(metadata)
    }

    pub fn resolve_by_name(&self, display_name: &str) -> ProfileResult<BrowserProfileMetadata> {
        if !self.profiles_enabled {
            return Err(ProfileError::ProfilesDisabled);
        }

        validate_display_name(display_name)?;

        if !self.allowed_profiles.is_empty()
            && !self.allowed_profiles.iter().any(|p| p == display_name)
        {
            return Err(ProfileError::ProfileNotFound(format!(
                "{display_name} is not in the allowed_profiles list"
            )));
        }

        let profiles = self.list_profiles()?;
        profiles
            .into_iter()
            .find(|p| p.display_name == display_name)
            .ok_or_else(|| ProfileError::ProfileNotFound(display_name.to_string()))
    }

    pub fn resolve_for_origin(
        &self,
        display_name: &str,
        request_origin: &str,
    ) -> ProfileResult<BrowserProfileMetadata> {
        let meta = self.resolve_by_name(display_name)?;
        let normalized_request = normalize_origin(request_origin)?;
        if meta.allowed_origin != normalized_request {
            return Err(ProfileError::ProfileNotFound(format!(
                "profile '{}' is not allowed for origin '{}'; it is restricted to '{}'",
                meta.display_name, request_origin, meta.allowed_origin
            )));
        }
        Ok(meta)
    }

    pub fn load_metadata(&self, id: &str) -> ProfileResult<BrowserProfileMetadata> {
        let profile_dir = self.root_dir.join(id);
        let meta_path = profile_dir.join("profile.toml");

        if !profile_dir.exists() {
            return Err(ProfileError::ProfileNotFound(id.to_string()));
        }

        validate_not_symlink(&profile_dir)?;

        if !meta_path.exists() {
            return Err(ProfileError::ProfileIncomplete(id.to_string()));
        }

        let content = fs::read_to_string(&meta_path)
            .map_err(|e| ProfileError::IoError(format!("failed to read metadata: {e}")))?;
        let metadata: BrowserProfileMetadata = toml::from_str(&content)
            .map_err(|e| ProfileError::MetadataCorrupt(format!("{}: {e}", meta_path.display())))?;

        Ok(metadata)
    }

    pub fn write_metadata(
        &self,
        profile_dir: &Path,
        metadata: &BrowserProfileMetadata,
    ) -> ProfileResult<()> {
        let meta_path = profile_dir.join("profile.toml");
        let tmp_path = profile_dir.join("profile.toml.tmp");

        validate_inside_root(&self.root_dir, profile_dir)?;

        let content = toml::to_string_pretty(metadata)
            .map_err(|e| ProfileError::IoError(format!("failed to serialize metadata: {e}")))?;

        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| {
                    ProfileError::IoError(format!("failed to create temp metadata: {e}"))
                })?;
            file.write_all(content.as_bytes()).map_err(|e| {
                ProfileError::IoError(format!("failed to write temp metadata: {e}"))
            })?;
            file.sync_all()
                .map_err(|e| ProfileError::IoError(format!("failed to sync temp metadata: {e}")))?;
        }

        fs::rename(&tmp_path, &meta_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            ProfileError::IoError(format!("failed to rename metadata: {e}"))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&meta_path, perms);
        }

        Ok(())
    }

    pub fn update_last_used(&self, metadata: &mut BrowserProfileMetadata) -> ProfileResult<()> {
        metadata.last_used_at = Some(Utc::now());
        let profile_dir = self.root_dir.join(&metadata.id);
        self.write_metadata(&profile_dir, metadata)
    }

    pub fn update_browser_info(
        &self,
        metadata: &mut BrowserProfileMetadata,
        family: &str,
        major_version: Option<u32>,
    ) -> ProfileResult<()> {
        metadata.browser_family = family.to_string();
        metadata.browser_major_version = major_version;
        let profile_dir = self.root_dir.join(&metadata.id);
        self.write_metadata(&profile_dir, metadata)
    }

    pub fn check_compatibility(
        &self,
        metadata: &BrowserProfileMetadata,
        current_browser_major: Option<u32>,
    ) -> ProfileResult<()> {
        match (metadata.browser_major_version, current_browser_major) {
            (Some(profile_ver), Some(browser_ver)) if profile_ver > browser_ver => {
                Err(ProfileError::ProfileIncompatible {
                    name: metadata.display_name.clone(),
                    profile_version: profile_ver,
                    browser_version: browser_ver,
                })
            }
            _ => Ok(()),
        }
    }

    pub fn list_profiles(&self) -> ProfileResult<Vec<BrowserProfileMetadata>> {
        if !self.profiles_enabled {
            return Ok(Vec::new());
        }

        let mut profiles = Vec::new();

        let entries = match fs::read_dir(&self.root_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(ProfileError::IoError(format!(
                    "failed to read profiles dir: {e}"
                )))
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            validate_not_symlink(&path)?;

            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            match self.load_metadata(&dir_name) {
                Ok(meta) => profiles.push(meta),
                Err(_) => continue,
            }
        }

        profiles.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(profiles)
    }

    pub fn remove_profile(&self, display_name: &str) -> ProfileResult<String> {
        if !self.profiles_enabled {
            return Err(ProfileError::ProfilesDisabled);
        }

        validate_display_name(display_name)?;

        let meta = self.resolve_by_name(display_name)?;
        let profile_dir = self.root_dir.join(&meta.id);

        validate_inside_root(&self.root_dir, &profile_dir)?;

        let mut lock = ProfileLock::new(&profile_dir);
        if !lock
            .try_acquire()
            .map_err(|e| ProfileError::IoError(e.to_string()))?
        {
            return Err(ProfileError::ProfileBusy(display_name.to_string()));
        }

        for entry in fs::read_dir(&profile_dir).map_err(|e| {
            ProfileError::IoError(format!("failed to read profile dir for removal: {e}"))
        })? {
            let entry = entry
                .map_err(|e| ProfileError::IoError(format!("failed to read profile entry: {e}")))?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path).map_err(|e| {
                    ProfileError::IoError(format!("failed to remove profile subdir: {e}"))
                })?;
            } else {
                fs::remove_file(&path).map_err(|e| {
                    ProfileError::IoError(format!("failed to remove profile file: {e}"))
                })?;
            }
        }

        fs::remove_dir(&profile_dir).map_err(|e| {
            ProfileError::IoError(format!("failed to remove profile directory: {e}"))
        })?;

        Ok(meta.id)
    }

    pub fn profile_dir_for(&self, id: &str) -> PathBuf {
        self.root_dir.join(id)
    }

    pub fn chrome_data_dir_for(&self, id: &str) -> PathBuf {
        self.root_dir.join(id).join("chrome-data")
    }

    pub fn acquire_lock(&self, id: &str) -> ProfileResult<ProfileLock> {
        let profile_dir = self.root_dir.join(id);
        if !profile_dir.exists() {
            return Err(ProfileError::ProfileNotFound(id.to_string()));
        }
        let mut lock = ProfileLock::new(&profile_dir);
        if !lock
            .try_acquire()
            .map_err(|e| ProfileError::IoError(e.to_string()))?
        {
            return Err(ProfileError::ProfileBusy(id.to_string()));
        }
        Ok(lock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_manager(dir: &Path) -> ProfileManager {
        ProfileManager {
            root_dir: dir.to_path_buf(),
            profiles_enabled: true,
            allowed_profiles: Vec::new(),
        }
    }

    #[test]
    fn valid_display_names() {
        assert!(validate_display_name("my-profile").is_ok());
        assert!(validate_display_name("test_profile").is_ok());
        assert!(validate_display_name("Profile1").is_ok());
        assert!(validate_display_name("a").is_ok());
        assert!(validate_display_name("x123").is_ok());
    }

    #[test]
    fn invalid_display_names() {
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name("a b").is_err());
        assert!(validate_display_name("a.b").is_err());
        assert!(validate_display_name("../etc").is_err());
        assert!(validate_display_name("a/b").is_err());
        assert!(validate_display_name("a\\b").is_err());
        assert!(validate_display_name(&"x".repeat(65)).is_err());
        assert!(validate_display_name(".hidden").is_err());
        assert!(validate_display_name("trailing.").is_err());
    }

    #[test]
    fn origin_normalization() {
        let r = normalize_origin("https://Example.COM").unwrap();
        assert_eq!(r, "https://example.com:443");

        let r = normalize_origin("http://example.com:8080").unwrap();
        assert_eq!(r, "http://example.com:8080");

        let r = normalize_origin("https://example.com/").unwrap();
        assert_eq!(r, "https://example.com:443");
    }

    #[test]
    fn origin_rejection() {
        assert!(normalize_origin("").is_err());
        assert!(normalize_origin("ftp://example.com").is_err());
        assert!(normalize_origin("https://localhost").is_err());
        assert!(normalize_origin("https://127.0.0.1").is_err());
        assert!(normalize_origin("https://example.com/path").is_err());
        assert!(normalize_origin("https://user:pass@example.com").is_err());
    }

    #[test]
    fn opaque_id_deterministic() {
        let id1 = opaque_id("test", "https://example.com:443");
        let id2 = opaque_id("test", "https://example.com:443");
        assert_eq!(id1, id2);
        assert!(id1.starts_with("prof_"));
    }

    #[test]
    fn opaque_id_differs_by_name() {
        let id1 = opaque_id("test", "https://example.com:443");
        let id2 = opaque_id("other", "https://example.com:443");
        assert_ne!(id1, id2);
    }

    #[test]
    fn opaque_id_differs_by_origin() {
        let id1 = opaque_id("test", "https://example.com:443");
        let id2 = opaque_id("test", "https://other.com:443");
        assert_ne!(id1, id2);
    }

    #[test]
    fn create_and_load_profile() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());

        let meta = mgr
            .create_profile("my-site", "https://example.com")
            .unwrap();
        assert_eq!(meta.display_name, "my-site");
        assert_eq!(meta.allowed_origin, "https://example.com:443");
        assert!(meta.id.starts_with("prof_"));
        assert_eq!(meta.schema_version, PROFILE_SCHEMA_VERSION);

        let loaded = mgr.load_metadata(&meta.id).unwrap();
        assert_eq!(loaded.display_name, "my-site");
    }

    #[test]
    fn create_duplicate_returns_existing() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());

        let m1 = mgr.create_profile("site", "https://a.com").unwrap();
        let m2 = mgr.create_profile("site", "https://a.com").unwrap();
        assert_eq!(m1.id, m2.id);
    }

    #[test]
    fn resolve_by_name_works() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        mgr.create_profile("portal", "https://portal.com").unwrap();

        let found = mgr.resolve_by_name("portal").unwrap();
        assert_eq!(found.display_name, "portal");
        assert!(mgr.resolve_by_name("missing").is_err());
    }

    #[test]
    fn resolve_for_origin_enforces_origin() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        mgr.create_profile("site", "https://a.com").unwrap();

        assert!(mgr.resolve_for_origin("site", "https://a.com").is_ok());
        assert!(mgr.resolve_for_origin("site", "https://b.com").is_err());
    }

    #[test]
    fn list_profiles_empty() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        assert!(mgr.list_profiles().unwrap().is_empty());
    }

    #[test]
    fn list_profiles_returns_all() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        mgr.create_profile("alpha", "https://a.com").unwrap();
        mgr.create_profile("beta", "https://b.com").unwrap();

        let list = mgr.list_profiles().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].display_name, "alpha");
        assert_eq!(list[1].display_name, "beta");
    }

    #[test]
    fn remove_profile_works() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        mgr.create_profile("doomed", "https://x.com").unwrap();

        let removed_id = mgr.remove_profile("doomed").unwrap();
        assert!(!removed_id.is_empty());
        assert!(mgr.resolve_by_name("doomed").is_err());
        assert!(mgr.list_profiles().unwrap().is_empty());
    }

    #[test]
    fn remove_nonexistent_fails() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        assert!(mgr.remove_profile("nope").is_err());
    }

    #[test]
    fn disabled_profiles_reject_creation() {
        let tmp = TempDir::new().unwrap();
        let mgr = ProfileManager {
            root_dir: tmp.path().to_path_buf(),
            profiles_enabled: false,
            allowed_profiles: Vec::new(),
        };
        assert!(matches!(
            mgr.create_profile("test", "https://x.com"),
            Err(ProfileError::ProfilesDisabled)
        ));
    }

    #[test]
    fn allowed_profiles_restriction() {
        let tmp = TempDir::new().unwrap();
        let mgr = ProfileManager {
            root_dir: tmp.path().to_path_buf(),
            profiles_enabled: true,
            allowed_profiles: vec!["allowed-one".to_string()],
        };

        assert!(mgr.create_profile("allowed-one", "https://x.com").is_ok());
        assert!(mgr.create_profile("not-allowed", "https://y.com").is_err());
    }

    #[test]
    fn profile_dir_helpers() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        let dir = mgr.profile_dir_for("prof_abc123");
        assert_eq!(dir, tmp.path().join("prof_abc123"));

        let chrome = mgr.chrome_data_dir_for("prof_abc123");
        assert_eq!(chrome, tmp.path().join("prof_abc123").join("chrome-data"));
    }

    #[test]
    fn profile_count_limit() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        for i in 0..MAX_PROFILE_COUNT {
            mgr.create_profile(&format!("p{i}"), "https://x.com")
                .unwrap();
        }
        assert!(matches!(
            mgr.create_profile("overflow", "https://y.com"),
            Err(ProfileError::ProfileLimitReached)
        ));
    }

    #[test]
    fn metadata_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        let meta = mgr.create_profile("atomic", "https://x.com").unwrap();

        let mut updated = meta.clone();
        updated.last_used_at = Some(Utc::now());
        let profile_dir = mgr.profile_dir_for(&meta.id);
        mgr.write_metadata(&profile_dir, &updated).unwrap();

        let loaded = mgr.load_metadata(&meta.id).unwrap();
        assert!(loaded.last_used_at.is_some());
    }

    #[test]
    fn symlink_rejected_on_profile_dir() {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());

        let real_dir = tmp.path().join("real_profile");
        fs::create_dir_all(&real_dir).unwrap();
        let meta = BrowserProfileMetadata {
            id: "real_profile".to_string(),
            display_name: "real".to_string(),
            allowed_origin: "https://x.com:443".to_string(),
            created_at: Utc::now(),
            last_used_at: None,
            browser_family: String::new(),
            browser_major_version: None,
            schema_version: PROFILE_SCHEMA_VERSION,
        };
        mgr.write_metadata(&real_dir, &meta).unwrap();

        let link = tmp.path().join("fake_profile");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        assert!(matches!(
            mgr.load_metadata("fake_profile"),
            Err(ProfileError::SymlinkDetected(_))
        ));
    }
}
