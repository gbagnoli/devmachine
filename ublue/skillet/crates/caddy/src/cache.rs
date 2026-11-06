//! Opt-in cache policy for public, credential-free web application assets.
use serde::{Deserialize, Serialize};

pub const IMAGE: &str = "ghcr.io/gbagnoli/skillet-caddy:cache-v1";
pub const PATH: &str = "/var/cache/skillet/jellyfin";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    /// No media, API, authentication, artwork or token-bearing requests.
    JellyfinStatic,
}

pub(super) fn container_options() -> [String; 3] {
    [
        format!("Tmpfs={PATH}:rw,size=128m,mode=0700"),
        "PodmanArgs=--memory=512m --memory-swap=512m".into(),
        "Environment=GOMEMLIMIT=384MiB".into(),
    ]
}

pub(super) fn global() -> &'static str {
    "    log {\n        format filter {\n            wrap json\n            fields {\n                request delete\n            }\n        }\n    }\n    cache {\n        ttl 10m\n        stale 0s\n        max_cacheable_body_bytes 2097152\n        default_cache_control no-store\n        log_level error\n        key {\n            hash\n            hide\n            headers Accept Accept-Encoding\n        }\n        simplefs {\n            configuration {\n                path /var/cache/skillet/jellyfin\n                size 2048\n                directory_size 64MB\n            }\n        }\n    }\n"
}

pub(super) fn route(upstream: &str) -> String {
    format!(
        r#"        @static_cache {{
            method GET
            path_regexp static `^/web/.*\.(js|css|woff2?|ttf|svg|png|jpg|ico)$`
            expression `{{http.request.uri.query}} == "" && {{http.request.header.Authorization}} == "" && {{http.request.header.Cookie}} == "" && {{http.request.header.X-Emby-Authorization}} == "" && {{http.request.header.X-Emby-Token}} == "" && {{http.request.header.X-MediaBrowser-Token}} == "" && {{http.request.header.Range}} == ""`
        }}
        handle @static_cache {{
            route {{
                cache
                header {{
                    Cache-Control no-store
                    match header Set-Cookie *
                }}
                header {{
                    Cache-Control no-store
                    match status 203 204 206 300 301 404 405 410 414 501
                }}
                reverse_proxy {upstream}
            }}
        }}
"#
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runtime;
