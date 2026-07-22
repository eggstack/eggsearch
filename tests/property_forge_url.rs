use eggsearch::core::repo_fetch::{
    codeberg_browser_url, codeberg_raw_url, gitea_browser_url, gitea_raw_url, github_browser_url,
    github_permalink_url, github_raw_permalink_url, github_raw_url, gitlab_browser_url,
    gitlab_raw_url,
};
use proptest::prelude::*;

fn owner_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,30}"
}

fn repo_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,30}"
}

fn ref_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9._-]{1,30}",
        "[a-zA-Z0-9._-]{1,20}/[a-zA-Z0-9._-]{1,20}",
        "[a-f0-9]{40}",
        "[a-f0-9]{7,12}",
    ]
}

fn file_path_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,10}/[a-zA-Z0-9._-]{1,20}\\.[a-z]{1,5}"
}

proptest! {
    #[test]
    fn github_browser_url_preserves_owner_repo_ref_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = github_browser_url(&owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{r}/")),
            "URL missing ref segment: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
        prop_assert!(url.starts_with("https://github.com/"),
            "URL wrong base: {url}");
    }

    #[test]
    fn github_raw_url_preserves_owner_repo_ref_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = github_raw_url(&owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.ends_with(&format!("/{r}/{path}")),
            "URL does not end with ref/path: {url}");
        prop_assert!(url.starts_with("https://raw.githubusercontent.com/"),
            "URL wrong base: {url}");
    }

    #[test]
    fn github_permalink_url_preserves_owner_repo_sha_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        sha in "[a-f0-9]{40}",
        path in file_path_strategy(),
    ) {
        let url = github_permalink_url(&owner, &repo, &sha, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{sha}/")),
            "URL missing commit sha: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
    }

    #[test]
    fn github_raw_permalink_url_preserves_owner_repo_sha_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        sha in "[a-f0-9]{40}",
        path in file_path_strategy(),
    ) {
        let url = github_raw_permalink_url(&owner, &repo, &sha, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{sha}/")),
            "URL missing commit sha: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
    }

    #[test]
    fn gitlab_browser_url_preserves_owner_repo_ref_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = gitlab_browser_url(&owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
        prop_assert!(url.starts_with("https://gitlab.com/"),
            "URL wrong base: {url}");
    }

    #[test]
    fn codeberg_browser_url_preserves_owner_repo_ref_path(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = codeberg_browser_url(&owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{r}/")),
            "URL missing ref segment: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
        prop_assert!(url.starts_with("https://codeberg.org/"),
            "URL wrong base: {url}");
    }

    #[test]
    fn gitea_browser_url_preserves_base_owner_repo_ref_path(
        base in "https://[a-z0-9.-]{3,20}",
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = gitea_browser_url(&base, &owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{r}/")),
            "URL missing ref segment: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
    }

    #[test]
    fn gitea_raw_url_preserves_base_owner_repo_ref_path(
        base in "https://[a-z0-9.-]{3,20}",
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url = gitea_raw_url(&base, &owner, &repo, &r, &path);
        prop_assert!(url.contains(&format!("{owner}/{repo}")),
            "URL missing owner/repo: {url}");
        prop_assert!(url.contains(&format!("/{r}/")),
            "URL missing ref segment: {url}");
        prop_assert!(url.ends_with(&format!("/{path}")),
            "URL does not end with path: {url}");
    }

    #[test]
    fn commit_sha_never_substituted_for_ref(
        owner in owner_strategy(),
        repo in repo_strategy(),
        sha in "[a-f0-9]{40}",
        path in file_path_strategy(),
    ) {
        let ref_url = github_browser_url(&owner, &repo, "HEAD", &path);
        let sha_url = github_permalink_url(&owner, &repo, &sha, &path);

        prop_assert!(ref_url.contains("/HEAD/"),
            "mutable ref URL should contain HEAD, not sha");
        prop_assert!(sha_url.contains(&format!("/{sha}/")),
            "permalink should contain commit sha");
        prop_assert_ne!(ref_url, sha_url,
            "HEAD URL and permalink URL must be different");
    }

    #[test]
    fn github_url_generation_deterministic(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let a1 = github_browser_url(&owner, &repo, &r, &path);
        let a2 = github_browser_url(&owner, &repo, &r, &path);
        prop_assert_eq!(a1, a2);

        let b1 = github_raw_url(&owner, &repo, &r, &path);
        let b2 = github_raw_url(&owner, &repo, &r, &path);
        prop_assert_eq!(b1, b2);

        let c1 = github_permalink_url(&owner, &repo, &r, &path);
        let c2 = github_permalink_url(&owner, &repo, &r, &path);
        prop_assert_eq!(c1, c2);

        let d1 = github_raw_permalink_url(&owner, &repo, &r, &path);
        let d2 = github_raw_permalink_url(&owner, &repo, &r, &path);
        prop_assert_eq!(d1, d2);
    }

    #[test]
    fn gitlab_url_generation_deterministic(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let a1 = gitlab_browser_url(&owner, &repo, &r, &path);
        let a2 = gitlab_browser_url(&owner, &repo, &r, &path);
        prop_assert_eq!(a1, a2);

        let b1 = gitlab_raw_url(&owner, &repo, &r, &path);
        let b2 = gitlab_raw_url(&owner, &repo, &r, &path);
        prop_assert_eq!(b1, b2);
    }

    #[test]
    fn codeberg_url_generation_deterministic(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let a1 = codeberg_browser_url(&owner, &repo, &r, &path);
        let a2 = codeberg_browser_url(&owner, &repo, &r, &path);
        prop_assert_eq!(a1, a2);

        let b1 = codeberg_raw_url(&owner, &repo, &r, &path);
        let b2 = codeberg_raw_url(&owner, &repo, &r, &path);
        prop_assert_eq!(b1, b2);
    }

    #[test]
    fn gitea_url_generation_deterministic(
        base in "https://[a-z0-9.-]{3,20}",
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let a1 = gitea_browser_url(&base, &owner, &repo, &r, &path);
        let a2 = gitea_browser_url(&base, &owner, &repo, &r, &path);
        prop_assert_eq!(a1, a2);

        let b1 = gitea_raw_url(&base, &owner, &repo, &r, &path);
        let b2 = gitea_raw_url(&base, &owner, &repo, &r, &path);
        prop_assert_eq!(b1, b2);
    }

    #[test]
    fn different_refs_produce_different_urls(
        owner in owner_strategy(),
        repo in repo_strategy(),
        path in file_path_strategy(),
    ) {
        let url_a = github_browser_url(&owner, &repo, "main", &path);
        let url_b = github_browser_url(&owner, &repo, "develop", &path);
        prop_assert_ne!(url_a, url_b,
            "different refs must produce different URLs");
    }

    #[test]
    fn different_owners_produce_different_urls(
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url_a = github_browser_url("owner-a", &repo, &r, &path);
        let url_b = github_browser_url("owner-b", &repo, &r, &path);
        prop_assert_ne!(url_a, url_b,
            "different owners must produce different URLs");
    }

    #[test]
    fn different_repos_produce_different_urls(
        owner in owner_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url_a = github_browser_url(&owner, "repo-a", &r, &path);
        let url_b = github_browser_url(&owner, "repo-b", &r, &path);
        prop_assert_ne!(url_a, url_b,
            "different repos must produce different URLs");
    }

    #[test]
    fn gitea_base_url_trailing_slash_normalised(
        owner in owner_strategy(),
        repo in repo_strategy(),
        r in ref_strategy(),
        path in file_path_strategy(),
    ) {
        let url1 = gitea_browser_url("https://gitea.example.com", &owner, &repo, &r, &path);
        let url2 = gitea_browser_url("https://gitea.example.com/", &owner, &repo, &r, &path);
        prop_assert_eq!(url1, url2,
            "trailing slash on base URL should be normalised");
    }
}
