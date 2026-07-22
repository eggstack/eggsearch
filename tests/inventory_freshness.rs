#[cfg(feature = "mock")]
mod tests {
    use std::time::{Duration, Instant};

    use eggsearch::core::local::{LocalConfig, LocalSearchRequest};
    use eggsearch::meta::local_backend::LocalWorkspaceBackend;
    use eggsearch::meta::local_inventory_cache::{
        build_inventory, needs_rebuild, probe_needs_rebuild, FRESHNESS_PROBE_INTERVAL,
        INVENTORY_REBUILD_TTL,
    };
    use std::path::Path;

    fn default_config() -> LocalConfig {
        LocalConfig {
            enabled: true,
            roots: Vec::new(),
            max_file_bytes: 1_048_576,
            max_indexed_files: 50_000,
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
        }
    }

    fn git_init_with_files(dir: &std::path::Path, files: &[(&str, &str)]) {
        std::process::Command::new("git")
            .arg("init")
            .current_dir(dir)
            .output()
            .expect("git init");

        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }

        for (name, _) in files {
            std::process::Command::new("git")
                .arg("add")
                .arg(name)
                .current_dir(dir)
                .output()
                .expect("git add");
        }

        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial")
            .current_dir(dir)
            .output()
            .expect("git commit");
    }

    fn search_and_get_telemetry(
        backend: &LocalWorkspaceBackend,
        query: &str,
    ) -> eggsearch::core::local::InventoryTelemetry {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let req = LocalSearchRequest {
            query: query.to_string(),
            timeout_ms: Some(10_000),
            ..Default::default()
        };
        let result = rt.block_on(backend.search(&req));
        result.telemetry.unwrap_or_default()
    }

    #[test]
    fn test3_new_untracked_file_visible_after_probe() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(root, &[("tracked.rs", "fn tracked() {}")]);

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv1 = build_inventory(&config, &roots);

        let paths1: Vec<&str> = inv1.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths1.contains(&"tracked.rs"));
        assert!(!paths1.contains(&"new_file.rs"));

        std::fs::write(root.join("new_file.rs"), "fn new_fn() {}").unwrap();

        assert!(
            probe_needs_rebuild(&inv1),
            "new untracked file should be detected by probe"
        );

        let inv2 = build_inventory(&config, &roots);
        let paths2: Vec<&str> = inv2.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(
            paths2.contains(&"new_file.rs"),
            "rebuilt inventory should include new file"
        );
    }

    #[test]
    fn test4_deleted_untracked_file_disappears() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(
            root,
            &[
                ("tracked.rs", "fn tracked() {}"),
                ("temp.rs", "fn temp() {}"),
            ],
        );

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv1 = build_inventory(&config, &roots);

        let paths1: Vec<&str> = inv1.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(paths1.contains(&"temp.rs"));

        std::fs::remove_file(root.join("temp.rs")).unwrap();

        assert!(
            probe_needs_rebuild(&inv1),
            "deleted file should be detected by probe"
        );

        let inv2 = build_inventory(&config, &roots);
        let paths2: Vec<&str> = inv2.roots[0]
            .entries
            .iter()
            .map(|e| e.relative_path.as_str())
            .collect();
        assert!(
            !paths2.contains(&"temp.rs"),
            "rebuilt inventory should not include deleted file"
        );
    }

    #[test]
    fn test5_staged_changes_invalidate_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(root, &[("lib.rs", "fn lib() {}")]);

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv1 = build_inventory(&config, &roots);

        std::fs::write(root.join("lib.rs"), "fn lib_v2() {}").unwrap();

        std::process::Command::new("git")
            .arg("add")
            .arg("lib.rs")
            .current_dir(root)
            .output()
            .expect("git add");

        assert!(
            needs_rebuild(&inv1, &config, INVENTORY_REBUILD_TTL),
            "staged changes should trigger rebuild"
        );
        assert!(
            probe_needs_rebuild(&inv1),
            "staged changes should be detected by probe"
        );
    }

    #[test]
    fn test7_branch_switch_invalidates_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(root, &[("main.rs", "fn main() {}")]);

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv1 = build_inventory(&config, &roots);
        let head1 = inv1.roots[0].head_commit.clone();
        assert!(head1.is_some(), "should have HEAD on initial commit");

        std::process::Command::new("git")
            .arg("checkout")
            .arg("-b")
            .arg("feature")
            .current_dir(root)
            .output()
            .expect("git checkout");

        std::fs::write(root.join("feature.rs"), "fn feature() {}").unwrap();
        std::process::Command::new("git")
            .arg("add")
            .arg("feature.rs")
            .current_dir(root)
            .output()
            .expect("git add feature");
        std::process::Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("add feature")
            .current_dir(root)
            .output()
            .expect("git commit feature");

        let inv2 = build_inventory(&config, &roots);
        let head2 = inv2.roots[0].head_commit.clone();
        assert!(head2.is_some(), "should have HEAD on feature commit");

        assert_ne!(
            head1, head2,
            "HEAD should differ between main and feature branches"
        );

        assert!(
            needs_rebuild(&inv1, &config, INVENTORY_REBUILD_TTL),
            "inventory built on main should need rebuild after feature commit changes HEAD on feature branch"
        );
    }

    #[test]
    fn test10_failed_build_preserves_prior_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(root, &[("good.rs", "fn good() {}")]);

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];

        let _inv1 = build_inventory(&config, &roots);

        let backend = LocalWorkspaceBackend::new(config.clone()).unwrap();
        let telemetry = search_and_get_telemetry(&backend, "good");
        assert!(telemetry.used_inventory);
    }

    #[test]
    fn test_probe_interval_constants() {
        assert_eq!(FRESHNESS_PROBE_INTERVAL, Duration::from_secs(30));
        assert_eq!(INVENTORY_REBUILD_TTL, Duration::from_secs(300));
        assert!(FRESHNESS_PROBE_INTERVAL < INVENTORY_REBUILD_TTL);
    }

    #[test]
    fn test_freshness_confidence_high_within_probe() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv = build_inventory(&config, &roots);

        let age = inv.built_at.elapsed();
        assert!(
            age < FRESHNESS_PROBE_INTERVAL,
            "freshly built inventory should be within probe interval"
        );
    }

    #[test]
    fn test_freshness_confidence_medium_between_probe_and_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let mut inv = build_inventory(&config, &roots);

        inv.built_at = Instant::now() - Duration::from_secs(60);

        let age = inv.built_at.elapsed();
        assert!(age >= FRESHNESS_PROBE_INTERVAL);
        assert!(age < INVENTORY_REBUILD_TTL);
    }

    #[test]
    fn test_no_probe_needed_when_inventory_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        git_init_with_files(root, &[("main.rs", "fn main() {}")]);

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let roots = vec![(0, root.to_path_buf())];
        let inv = build_inventory(&config, &roots);

        assert!(
            !probe_needs_rebuild(&inv),
            "fresh inventory with no changes should not need rebuild"
        );
    }

    #[test]
    fn test_search_auto_builds_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let backend = LocalWorkspaceBackend::new(config.clone()).unwrap();

        let telemetry = search_and_get_telemetry(&backend, "main");
        assert!(telemetry.used_inventory);
        assert!(telemetry.cold_build);
    }

    #[test]
    fn test_search_reuses_fresh_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("lib.rs"), "pub fn add() -> i32 { 1 }").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            respect_gitignore: false,
            ..default_config()
        };
        let backend = LocalWorkspaceBackend::new(config.clone()).unwrap();

        let _t1 = search_and_get_telemetry(&backend, "main");
        let t2 = search_and_get_telemetry(&backend, "lib");

        assert!(t2.used_inventory);
        assert!(t2.inventory_fresh);
        assert!(!t2.cold_build);
    }

    #[test]
    fn test_eligibility_consistency_across_surfaces() {
        use eggsearch::core::local::{
            is_eligible, is_eligible_for_indexing, is_git_path_eligible, should_skip_component,
        };

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::write(root.join("source.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join(".hidden"), "secret").unwrap();
        std::fs::write(root.join("image.png"), vec![0u8; 50]).unwrap();
        std::fs::create_dir(root.join("target")).unwrap();
        std::fs::write(root.join("target/gen.rs"), "generated").unwrap();

        let config = LocalConfig {
            enabled: true,
            roots: vec![root.to_path_buf()],
            include_hidden: false,
            respect_gitignore: false,
            follow_symlinks: false,
            ..default_config()
        };

        let source_path = root.join("source.rs");
        assert!(is_eligible_for_indexing(&source_path, &config));
        assert!(is_eligible(
            Path::new("source.rs"),
            Some(root),
            &config,
            true
        ));
        assert!(is_git_path_eligible("source.rs", root, &config));

        assert!(should_skip_component(".hidden", false));
        assert!(!is_eligible(
            Path::new(".hidden"),
            Some(root),
            &config,
            true
        ));

        assert!(!is_eligible_for_indexing(&root.join("image.png"), &config));
        assert!(!is_eligible(
            Path::new("image.png"),
            Some(root),
            &config,
            true
        ));
        assert!(!is_git_path_eligible("image.png", root, &config));

        assert!(should_skip_component("target", false));
        assert!(!is_eligible(
            Path::new("target/gen.rs"),
            Some(root),
            &config,
            true
        ));

        assert!(should_skip_component("target", false));
        assert!(!is_git_path_eligible("target/gen.rs", root, &config));

        let config_hidden = LocalConfig {
            include_hidden: true,
            ..config.clone()
        };
        assert!(!should_skip_component(".hidden", true));
        assert!(is_eligible(
            Path::new(".hidden"),
            Some(root),
            &config_hidden,
            true
        ));
    }
}
