//! robots.txt cache with TTL and per-host allow/deny rules.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
struct RobotsEntry {
    fetched_at: Instant,
    allow_all: bool,
    disallowed_paths: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RobotsCache {
    client: reqwest::Client,
    ttl: Duration,
    cache: Arc<Mutex<HashMap<String, Option<RobotsEntry>>>>,
}

impl RobotsCache {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            ttl: Duration::from_secs(60 * 60),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn allowed(&self, url: &url::Url) -> bool {
        let host = match url.host_str() {
            Some(h) => h.to_lowercase(),
            None => return true,
        };

        let cached_opt = {
            let cache = self.cache.lock().await;
            cache.get(&host).cloned()
        };
        let mut entry = match cached_opt {
            Some(Some(e)) => e,
            _ => {
                let fetched = self.fetch_for(&host).await;
                let mut cache = self.cache.lock().await;
                cache.insert(host.clone(), fetched.clone());
                match fetched {
                    Some(e) => e,
                    None => return true,
                }
            }
        };
        if entry.fetched_at.elapsed() > self.ttl {
            if let Some(fresh) = self.fetch_for(&host).await {
                let mut cache = self.cache.lock().await;
                cache.insert(host.clone(), Some(fresh.clone()));
                entry = fresh;
            }
        }
        check_allowed(&entry, url.path())
    }

    async fn fetch_for(&self, host: &str) -> Option<RobotsEntry> {
        let url = format!("https://{host}/robots.txt");
        let resp = self.client.get(&url).timeout(Duration::from_secs(4)).send().await.ok()?;
        if !resp.status().is_success() {
            return Some(RobotsEntry {
                fetched_at: Instant::now(),
                allow_all: true,
                disallowed_paths: vec![],
            });
        }
        let text = resp.text().await.ok()?;
        Some(parse_robots(&text))
    }
}

fn check_allowed(entry: &RobotsEntry, path: &str) -> bool {
    if entry.allow_all {
        return true;
    }
    for prefix in &entry.disallowed_paths {
        if path.starts_with(prefix) {
            return false;
        }
    }
    true
}

fn parse_robots(text: &str) -> RobotsEntry {
    let mut wildcard = false;
    let mut disallowed = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("User-agent") {
            let v = rest.trim_start_matches(':').trim();
            wildcard = wildcard || v == "*";
        } else if wildcard {
            if let Some(rest) = line.strip_prefix("Disallow") {
                let v = rest.trim_start_matches(':').trim();
                if !v.is_empty() {
                    disallowed.push(v.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("Allow") {
                let v = rest.trim_start_matches(':').trim();
                if v == "/" {
                    return RobotsEntry {
                        fetched_at: Instant::now(),
                        allow_all: true,
                        disallowed_paths: vec![],
                    };
                }
            }
        }
    }
    RobotsEntry {
        fetched_at: Instant::now(),
        allow_all: disallowed.is_empty(),
        disallowed_paths: disallowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_robots() {
        let txt = r#"
            User-agent: *
            Disallow: /private/
            Disallow: /admin
            Allow: /
        "#;
        let e = parse_robots(txt);
        assert!(e.allow_all);
    }

    #[test]
    fn parses_disallow_only() {
        let txt = "User-agent: *\nDisallow: /x\n";
        let e = parse_robots(txt);
        assert!(!e.allow_all);
        assert!(e.disallowed_paths.contains(&"/x".to_string()));
    }
}
