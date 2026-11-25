use crate::core::config::{FetchProxyMode, FetchProxySection};
use crate::core::fetch::{FetchRouteInfo, FetchRouteKind};
use crate::fetch::types::FetchError;
use serde_json::json;
use url::Url;

#[derive(Clone)]
pub(crate) struct FetchProxyPlan {
    mode: FetchProxyMode,
    endpoint: Option<ProxyEndpoint>,
    username_env: Option<String>,
    password_env: Option<String>,
    no_proxy: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ProxyEndpoint {
    pub(crate) url: String,
    pub(crate) scheme: String,
    pub(crate) host: String,
    pub(crate) port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RouteDecision {
    pub(crate) kind: FetchRouteKind,
    pub(crate) reason: Option<&'static str>,
    pub(crate) scheme: Option<String>,
    pub(crate) fingerprint: Option<u64>,
}

impl RouteDecision {
    pub(crate) fn direct() -> Self {
        Self {
            kind: FetchRouteKind::Direct,
            reason: None,
            scheme: None,
            fingerprint: None,
        }
    }

    pub(crate) fn is_proxy(&self) -> bool {
        self.kind == FetchRouteKind::Proxy
    }
}

impl FetchProxyPlan {
    pub(crate) fn new(config: &FetchProxySection) -> Result<Self, FetchError> {
        if config.route == FetchProxyMode::Direct {
            config
                .validate()
                .map_err(|_| FetchError::ProxyRouteUnconfigured)?;
            return Ok(Self {
                mode: config.route,
                endpoint: None,
                username_env: None,
                password_env: None,
                no_proxy: Vec::new(),
            });
        }
        let configured_url = config
            .url
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or(FetchError::ProxyRouteUnconfigured)?;
        let parsed = Url::parse(configured_url).map_err(|_| FetchError::ProxyRouteUnconfigured)?;
        if !matches!(parsed.scheme(), "http" | "socks5") {
            return Err(FetchError::ProxySchemeUnsupported);
        }
        config
            .validate()
            .map_err(|_| FetchError::ProxyRouteUnconfigured)?;
        let host = parsed
            .host_str()
            .map(str::to_ascii_lowercase)
            .ok_or(FetchError::ProxyRouteUnconfigured)?;
        let port = parsed.port().unwrap_or(match parsed.scheme() {
            "http" => 80,
            _ => 1080,
        });
        Ok(Self {
            mode: config.route,
            endpoint: Some(ProxyEndpoint {
                url: parsed.to_string(),
                scheme: parsed.scheme().to_string(),
                host,
                port,
            }),
            username_env: config.username_env.clone(),
            password_env: config.password_env.clone(),
            no_proxy: config
                .no_proxy
                .iter()
                .map(|host| host.trim_start_matches('.').to_ascii_lowercase())
                .collect(),
        })
    }

    pub(crate) fn endpoint(&self) -> Option<&ProxyEndpoint> {
        self.endpoint.as_ref()
    }

    pub(crate) fn credential_envs(&self) -> (Option<&str>, Option<&str>) {
        (self.username_env.as_deref(), self.password_env.as_deref())
    }

    pub(crate) fn select(&self, url: &Url) -> RouteDecision {
        if self.mode == FetchProxyMode::Direct {
            return RouteDecision::direct();
        }
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        if self.mode == FetchProxyMode::Proxy
            && self
                .no_proxy
                .iter()
                .any(|pattern| host == *pattern || host.ends_with(&format!(".{pattern}")))
        {
            return RouteDecision {
                kind: FetchRouteKind::Direct,
                reason: Some("no_proxy"),
                scheme: None,
                fingerprint: None,
            };
        }
        let Some(endpoint) = self.endpoint.as_ref() else {
            return RouteDecision::direct();
        };
        RouteDecision {
            kind: FetchRouteKind::Proxy,
            reason: None,
            scheme: Some(endpoint.scheme.clone()),
            fingerprint: Some(route_fingerprint(self.mode, endpoint)),
        }
    }

    pub(crate) fn response_route(&self, url: &Url) -> Option<FetchRouteInfo> {
        if self.mode == FetchProxyMode::Direct {
            return None;
        }
        let selected = self.select(url);
        Some(FetchRouteInfo {
            kind: selected.kind,
            reason: selected.reason.map(str::to_string),
            scheme: selected.scheme,
        })
    }
}

fn route_fingerprint(mode: FetchProxyMode, endpoint: &ProxyEndpoint) -> u64 {
    let identity = format!(
        "{}\0{}\0{}\0{}",
        mode.as_str(),
        endpoint.scheme,
        endpoint.host,
        endpoint.port
    );
    identity
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn safe_endpoint_summary(url: Option<&str>) -> (Option<String>, Option<String>, Option<u16>) {
    let Some(url) = url.and_then(|value| Url::parse(value).ok()) else {
        return (None, None, None);
    };
    if !url.username().is_empty() || url.password().is_some() {
        return (None, None, None);
    }
    let scheme = matches!(url.scheme(), "http" | "socks5").then(|| url.scheme().to_string());
    let host = url.host_str().map(str::to_ascii_lowercase);
    let port = url.port().or_else(|| match url.scheme() {
        "http" => Some(80),
        "socks5" => Some(1080),
        _ => None,
    });
    (scheme, host, port)
}

pub(crate) fn diagnostics(config: &FetchProxySection) -> serde_json::Value {
    let credentials_configured = config.username_env.is_some() && config.password_env.is_some();
    let credentials_present = match (
        config.username_env.as_deref(),
        config.password_env.as_deref(),
    ) {
        (Some(username), Some(password)) => {
            std::env::var(username).is_ok_and(|value| !value.is_empty())
                && std::env::var(password).is_ok_and(|value| !value.is_empty())
        }
        _ => false,
    };
    let validation_state = if config.validate().is_err() {
        "invalid"
    } else if credentials_configured && !credentials_present {
        "credentials_missing"
    } else {
        "valid"
    };
    let (scheme, host, port) = safe_endpoint_summary(config.url.as_deref());
    json!({
        "route_class": config.route.as_str(),
        "scheme": scheme,
        "host": host,
        "port": port,
        "credentials_present": credentials_present,
        "validation_state": validation_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy_config(mode: FetchProxyMode) -> FetchProxySection {
        FetchProxySection {
            route: mode,
            url: Some("http://proxy.example:3128".into()),
            ..FetchProxySection::default()
        }
    }

    #[test]
    fn direct_route_has_no_response_metadata_or_fingerprint() {
        let plan = FetchProxyPlan::new(&FetchProxySection::default()).unwrap();
        let target = Url::parse("https://example.com/").unwrap();
        assert_eq!(plan.select(&target), RouteDecision::direct());
        assert!(plan.response_route(&target).is_none());
    }

    #[test]
    fn proxy_response_metadata_contains_only_route_kind_and_scheme() {
        let plan = FetchProxyPlan::new(&proxy_config(FetchProxyMode::Proxy)).unwrap();
        let target = Url::parse("https://example.com/").unwrap();
        let route = plan.response_route(&target).unwrap();
        let serialized = serde_json::to_value(route).unwrap();
        assert_eq!(serialized["kind"], "proxy");
        assert_eq!(serialized["scheme"], "http");
        assert!(serialized.get("host").is_none());
        assert!(serialized.get("port").is_none());
        assert!(serialized.get("credentials").is_none());
    }

    #[test]
    fn no_proxy_matches_exact_hosts_and_subdomains_only() {
        let mut config = proxy_config(FetchProxyMode::Proxy);
        config.no_proxy = vec![".internal.example".into()];
        let plan = FetchProxyPlan::new(&config).unwrap();
        let subdomain = plan.select(&Url::parse("https://docs.internal.example/").unwrap());
        assert_eq!(subdomain.kind, FetchRouteKind::Direct);
        assert_eq!(subdomain.reason, Some("no_proxy"));
        assert_eq!(subdomain.fingerprint, None);
        assert!(plan
            .select(&Url::parse("https://notinternal.example/").unwrap())
            .is_proxy());
    }

    #[test]
    fn route_fingerprint_excludes_credential_names_and_separates_route_classes() {
        let mut first = proxy_config(FetchProxyMode::Proxy);
        first.username_env = Some("FIRST_USER".into());
        first.password_env = Some("FIRST_PASSWORD".into());
        let mut second = first.clone();
        second.username_env = Some("SECOND_USER".into());
        second.password_env = Some("SECOND_PASSWORD".into());
        let first_plan = FetchProxyPlan::new(&first).unwrap();
        let second_plan = FetchProxyPlan::new(&second).unwrap();
        let target = Url::parse("https://example.com/").unwrap();
        assert_eq!(
            first_plan.select(&target).fingerprint,
            second_plan.select(&target).fingerprint
        );

        let required_plan =
            FetchProxyPlan::new(&proxy_config(FetchProxyMode::ProxyRequired)).unwrap();
        assert_ne!(
            first_plan.select(&target).fingerprint,
            required_plan.select(&target).fingerprint
        );
    }

    #[test]
    fn unsupported_proxy_scheme_is_typed() {
        let mut config = proxy_config(FetchProxyMode::Proxy);
        config.url = Some("socks5h://proxy.example:1080".into());
        assert!(matches!(
            FetchProxyPlan::new(&config),
            Err(FetchError::ProxySchemeUnsupported)
        ));
    }

    #[test]
    fn direct_and_proxy_routes_have_distinct_raw_and_derived_cache_keys() {
        let scope = crate::fetch::cache::CacheScope::Anonymous;
        let direct_raw = crate::fetch::cache::build_raw_cache_key_with_route(
            "https://example.com/page",
            &scope,
            crate::fetch::accept::AcceptPolicy::Html,
            None,
        );
        let proxy_raw = crate::fetch::cache::build_raw_cache_key_with_route(
            "https://example.com/page",
            &scope,
            crate::fetch::accept::AcceptPolicy::Html,
            Some(42),
        );
        assert_ne!(direct_raw, proxy_raw);

        let budget = crate::fetch::cache::ExtractionBudgetClass::document_scoped(500);
        let build_derived = |route_fingerprint| {
            crate::fetch::cache::build_derived_key_scoped_with_route(
                &scope,
                7,
                crate::core::fetch::ExtractMode::Text,
                budget,
                false,
                None,
                None,
                false,
                true,
                None,
                route_fingerprint,
            )
        };
        assert_ne!(build_derived(None), build_derived(Some(42)));
    }
}
