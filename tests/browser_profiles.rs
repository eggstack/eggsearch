#![cfg(feature = "browser")]

use eggsearch::fetch::browser::{
    BrowserProfileMetadata, ProfileError, ProfileManager, PROFILE_SCHEMA_VERSION,
};
use tempfile::TempDir;

fn make_manager(dir: &std::path::Path) -> ProfileManager {
    ProfileManager::new(Some(&dir.display().to_string()), true, Vec::new()).unwrap()
}

#[test]
fn profile_name_validation_rejects_invalid_names() {
    let long_name = "x".repeat(65);
    let cases: Vec<&str> = vec![
        "",
        "a b",
        "a.b",
        "../etc",
        "a/b",
        "a\\b",
        ".hidden",
        "trailing.",
        &long_name,
    ];
    for name in cases {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        assert!(
            mgr.create_profile(name, "https://example.com").is_err(),
            "expected rejection for name: {name:?}"
        );
    }
}

#[test]
fn profile_name_validation_accepts_valid_names() {
    let cases = vec!["my-profile", "test_profile", "Profile1", "a", "x123"];
    for name in cases {
        let tmp = TempDir::new().unwrap();
        let mgr = make_manager(tmp.path());
        assert!(
            mgr.create_profile(name, "https://example.com").is_ok(),
            "expected acceptance for name: {name:?}"
        );
    }
}

#[test]
fn origin_normalization_and_rejection() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    assert!(mgr.create_profile("t1", "https://Example.COM").is_ok());
    assert!(mgr.create_profile("t2", "http://example.com:8080").is_ok());
    assert!(mgr.create_profile("t3", "https://example.com/").is_ok());

    assert!(mgr.create_profile("r1", "").is_err());
    assert!(mgr.create_profile("r2", "ftp://example.com").is_err());
    assert!(mgr.create_profile("r3", "https://localhost").is_err());
    assert!(mgr.create_profile("r4", "https://127.0.0.1").is_err());
    assert!(mgr
        .create_profile("r5", "https://example.com/path")
        .is_err());
    assert!(mgr
        .create_profile("r6", "https://user:pass@example.com")
        .is_err());
}

#[test]
fn create_and_load_profile_metadata() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let meta = mgr
        .create_profile("my-site", "https://example.com")
        .unwrap();
    assert_eq!(meta.display_name, "my-site");
    assert_eq!(meta.allowed_origin, "https://example.com:443");
    assert!(meta.id.starts_with("prof_"));
    assert_eq!(meta.schema_version, PROFILE_SCHEMA_VERSION);
    assert!(meta.last_used_at.is_none());
    assert!(meta.browser_family.is_empty());

    let loaded = mgr.load_metadata(&meta.id).unwrap();
    assert_eq!(loaded.display_name, "my-site");
    assert_eq!(loaded.allowed_origin, "https://example.com:443");
}

#[test]
fn duplicate_create_returns_existing() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let m1 = mgr.create_profile("site", "https://a.com").unwrap();
    let m2 = mgr.create_profile("site", "https://a.com").unwrap();
    assert_eq!(m1.id, m2.id);
    assert_eq!(mgr.list_profiles().unwrap().len(), 1);
}

#[test]
fn resolve_by_name_exact_match() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    mgr.create_profile("portal", "https://portal.com").unwrap();

    let found = mgr.resolve_by_name("portal").unwrap();
    assert_eq!(found.display_name, "portal");

    assert!(mgr.resolve_by_name("missing").is_err());
}

#[test]
fn resolve_for_origin_enforces_exact_origin() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    mgr.create_profile("site", "https://a.com").unwrap();

    assert!(mgr.resolve_for_origin("site", "https://a.com").is_ok());
    assert!(mgr.resolve_for_origin("site", "https://b.com").is_err());
    assert!(mgr.resolve_for_origin("site", "http://a.com").is_err());
}

#[test]
fn list_profiles_sorted_and_empty() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    assert!(mgr.list_profiles().unwrap().is_empty());

    mgr.create_profile("beta", "https://b.com").unwrap();
    mgr.create_profile("alpha", "https://a.com").unwrap();

    let list = mgr.list_profiles().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].display_name, "alpha");
    assert_eq!(list[1].display_name, "beta");
}

#[test]
fn remove_profile_cleans_up() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let meta = mgr.create_profile("doomed", "https://x.com").unwrap();
    let profile_dir = mgr.profile_dir_for(&meta.id);
    assert!(profile_dir.exists());

    mgr.remove_profile("doomed").unwrap();
    assert!(!profile_dir.exists());
    assert!(mgr.resolve_by_name("doomed").is_err());
    assert!(mgr.list_profiles().unwrap().is_empty());
}

#[test]
fn remove_nonexistent_profile_fails() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    assert!(mgr.remove_profile("nope").is_err());
}

#[test]
fn disabled_profiles_reject_all_operations() {
    let tmp = TempDir::new().unwrap();
    let mgr =
        ProfileManager::new(Some(&tmp.path().display().to_string()), false, Vec::new()).unwrap();

    assert!(matches!(
        mgr.create_profile("test", "https://x.com"),
        Err(ProfileError::ProfilesDisabled)
    ));
    assert!(matches!(
        mgr.resolve_by_name("test"),
        Err(ProfileError::ProfilesDisabled)
    ));
    assert!(mgr.list_profiles().unwrap().is_empty());
}

#[test]
fn allowed_profiles_restricts_creation() {
    let tmp = TempDir::new().unwrap();
    let mgr = ProfileManager::new(
        Some(&tmp.path().display().to_string()),
        true,
        vec!["allowed-one".to_string()],
    )
    .unwrap();

    assert!(mgr.create_profile("allowed-one", "https://x.com").is_ok());
    assert!(mgr.create_profile("not-allowed", "https://y.com").is_err());
}

#[test]
fn opaque_id_is_deterministic_and_stable() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let m1 = mgr.create_profile("test", "https://example.com").unwrap();
    let m2 = mgr.create_profile("test", "https://example.com").unwrap();
    assert_eq!(m1.id, m2.id);
    assert!(m1.id.starts_with("prof_"));
}

#[test]
fn opaque_id_differs_by_name_and_origin() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let m1 = mgr.create_profile("a", "https://x.com").unwrap();
    let m2 = mgr.create_profile("b", "https://x.com").unwrap();
    let m3 = mgr.create_profile("a", "https://y.com").unwrap();
    assert_ne!(m1.id, m2.id);
    assert_ne!(m1.id, m3.id);
}

#[test]
fn metadata_atomic_write_preserves_data() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    let meta = mgr.create_profile("atomic", "https://x.com").unwrap();

    let mut updated = meta.clone();
    updated.last_used_at = Some(chrono::Utc::now());
    updated.browser_family = "Chrome".to_string();
    updated.browser_major_version = Some(120);
    let profile_dir = mgr.profile_dir_for(&meta.id);
    mgr.write_metadata(&profile_dir, &updated).unwrap();

    let loaded = mgr.load_metadata(&meta.id).unwrap();
    assert!(loaded.last_used_at.is_some());
    assert_eq!(loaded.browser_family, "Chrome");
    assert_eq!(loaded.browser_major_version, Some(120));
}

#[test]
fn profile_count_limit_enforced() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    for i in 0..eggsearch::fetch::browser::profiles::MAX_PROFILE_COUNT {
        mgr.create_profile(&format!("p{i}"), "https://x.com")
            .unwrap();
    }
    assert!(matches!(
        mgr.create_profile("overflow", "https://y.com"),
        Err(ProfileError::ProfileLimitReached)
    ));
}

#[test]
fn symlink_on_profile_dir_rejected() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());

    let real_dir = tmp.path().join("real_profile");
    std::fs::create_dir_all(&real_dir).unwrap();
    let meta = BrowserProfileMetadata {
        id: "real_profile".to_string(),
        display_name: "real".to_string(),
        allowed_origin: "https://x.com:443".to_string(),
        created_at: chrono::Utc::now(),
        last_used_at: None,
        browser_family: String::new(),
        browser_major_version: None,
        schema_version: PROFILE_SCHEMA_VERSION,
    };
    mgr.write_metadata(&real_dir, &meta).unwrap();

    let link = tmp.path().join("fake_profile");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, &link).unwrap();

    #[cfg(unix)]
    assert!(matches!(
        mgr.load_metadata("fake_profile"),
        Err(ProfileError::SymlinkDetected(_))
    ));
}

#[test]
fn update_last_used_sets_timestamp() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    let mut meta = mgr.create_profile("ts", "https://x.com").unwrap();
    assert!(meta.last_used_at.is_none());

    mgr.update_last_used(&mut meta).unwrap();

    let loaded = mgr.load_metadata(&meta.id).unwrap();
    assert!(loaded.last_used_at.is_some());
}

#[test]
fn update_browser_info_sets_family() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    let mut meta = mgr.create_profile("browser", "https://x.com").unwrap();

    mgr.update_browser_info(&mut meta, "Chrome", Some(120))
        .unwrap();

    let loaded = mgr.load_metadata(&meta.id).unwrap();
    assert_eq!(loaded.browser_family, "Chrome");
    assert_eq!(loaded.browser_major_version, Some(120));
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
fn cache_scope_partitioning() {
    let s1 = eggsearch::fetch::cache::CacheScope::Profile("alice".into());
    let s2 = eggsearch::fetch::cache::CacheScope::Profile("bob".into());
    let anon = eggsearch::fetch::cache::CacheScope::Anonymous;
    assert_ne!(s1, s2);
    assert_ne!(s1, anon);
    assert_ne!(s2, anon);
}

#[test]
fn display_name_too_long_rejected() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    let name = "x".repeat(65);
    assert!(mgr.create_profile(&name, "https://x.com").is_err());
}

#[test]
fn display_name_with_dots_rejected() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(tmp.path());
    assert!(mgr.create_profile("a.b", "https://x.com").is_err());
    assert!(mgr.create_profile(".hidden", "https://x.com").is_err());
    assert!(mgr.create_profile("trailing.", "https://x.com").is_err());
}
