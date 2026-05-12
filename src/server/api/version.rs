//! Eigenwallet (asb) version info: reads the running image tag from the
//! `asb` Deployment in the `eigenwallet` namespace and compares it to the
//! latest release on GitHub. The GitHub response is cached in
//! `AppStateInner` for 1 hour to stay well under the 60/hr/IP
//! unauthenticated rate limit.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use k8s_openapi::api::apps::v1::Deployment;
use kube::Api;
use serde::Deserialize;

use crate::server::state::AppStateInner;
use crate::types::VersionInfoDto;

const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/eigenwallet/eigenwallet/releases/latest";
const USER_AGENT: &str = "eigenwallet-admin";
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default, Clone)]
pub struct GithubReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
}

#[derive(Debug, Default)]
pub struct VersionInfoCache {
    pub data: Option<GithubReleaseInfo>,
    pub fetched_at: Option<Instant>,
}

impl VersionInfoCache {
    fn is_fresh(&self) -> bool {
        self.fetched_at
            .map(|t| t.elapsed() < CACHE_TTL)
            .unwrap_or(false)
    }
}

pub async fn fetch(state: &AppStateInner) -> Result<VersionInfoDto> {
    let (current, mut fetch_error) = match read_current_version(state).await {
        Ok(v) => (Some(v), None),
        Err(e) => {
            tracing::warn!(error = %e, "failed to read running asb version from k8s");
            (
                None,
                Some(format!("failed to read running asb version: {e}")),
            )
        }
    };

    let latest_info = match cached_or_fetch_latest(state).await {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!(error = %e, "failed to fetch latest eigenwallet release");
            // Don't overwrite kube fetch_error if it was set; surface the more
            // load-bearing of the two. Prefer the kube error since the version
            // banner's primary purpose is showing the current version.
            if fetch_error.is_none() {
                fetch_error = Some(format!("failed to fetch latest release: {e}"));
            }
            None
        }
    };

    let latest = latest_info.as_ref().map(|i| normalize_tag(&i.tag_name));
    let releases_url = latest_info.as_ref().map(|i| i.html_url.clone());
    let has_update = match (current.as_deref(), latest.as_deref()) {
        (Some(c), Some(l)) => compare_versions(c, l).map(|o| o.is_lt()).unwrap_or(false),
        _ => false,
    };

    Ok(VersionInfoDto {
        current,
        latest,
        has_update,
        releases_url,
        fetch_error,
    })
}

async fn read_current_version(state: &AppStateInner) -> Result<String> {
    let kube = state
        .kube
        .as_ref()
        .ok_or_else(|| anyhow!("kube client not initialised"))?;
    let client = kube.client();
    let api: Api<Deployment> = Api::namespaced(client, &state.config.asb_namespace);
    let dep = api
        .get(&state.config.asb_deployment_name)
        .await
        .map_err(|e| {
            anyhow!(
                "get deployment {}/{}: {e}",
                state.config.asb_namespace,
                state.config.asb_deployment_name
            )
        })?;
    let image = dep
        .spec
        .and_then(|s| s.template.spec)
        .and_then(|p| p.containers.into_iter().next())
        .and_then(|c| c.image)
        .ok_or_else(|| anyhow!("asb deployment has no container image"))?;
    parse_image_tag(&image).ok_or_else(|| anyhow!("could not parse image tag from {image:?}"))
}

/// Parse `ghcr.io/foo/bar:4.5.0@sha256:...` -> `4.5.0`.
/// Also handles `ghcr.io/foo/bar:4.5.0` (no digest).
fn parse_image_tag(image: &str) -> Option<String> {
    // Strip optional `@sha256:...` digest suffix.
    let without_digest = image.split_once('@').map(|(l, _)| l).unwrap_or(image);
    // The image path may contain a registry port (`host:5000/repo:tag`), so
    // grab the tag from the LAST colon.
    let (_repo, tag) = without_digest.rsplit_once(':')?;
    // A port-only image (`host:5000/repo`) would have the segment after the
    // colon containing a `/`; reject those — there's no tag in that case.
    if tag.is_empty() || tag.contains('/') {
        return None;
    }
    Some(tag.to_string())
}

async fn cached_or_fetch_latest(state: &AppStateInner) -> Result<GithubReleaseInfo> {
    {
        let cache = state.version_info.read().await;
        if cache.is_fresh()
            && let Some(d) = cache.data.as_ref()
        {
            return Ok(d.clone());
        }
    }

    let info = fetch_latest_release().await?;
    {
        let mut cache = state.version_info.write().await;
        cache.data = Some(info.clone());
        cache.fetched_at = Some(Instant::now());
    }
    Ok(info)
}

async fn fetch_latest_release() -> Result<GithubReleaseInfo> {
    #[derive(Deserialize)]
    struct Resp {
        tag_name: String,
        html_url: String,
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;
    let resp: Resp = client
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(GithubReleaseInfo {
        tag_name: resp.tag_name,
        html_url: resp.html_url,
    })
}

/// Strip optional leading `v` from a tag. `v4.5.0` -> `4.5.0`.
fn normalize_tag(tag: &str) -> String {
    tag.strip_prefix('v').unwrap_or(tag).to_string()
}

/// Compare two `X.Y.Z` (or longer) numeric version strings. Returns `None` if
/// either side fails to parse into a non-empty sequence of integers. Stops
/// comparing at the first non-numeric segment (so prerelease suffixes like
/// `-rc1` are ignored — running `4.5.0` against `4.5.0-rc1` compares as
/// equal; not perfect but acceptable for the upgrade-available indicator).
fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let av = parse_numeric_version(a)?;
    let bv = parse_numeric_version(b)?;
    Some(av.cmp(&bv))
}

fn parse_numeric_version(s: &str) -> Option<Vec<u64>> {
    let core = s.strip_prefix('v').unwrap_or(s);
    // Stop at the first `-` or `+` to ignore pre-release / build metadata.
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let parts: Vec<u64> = core
        .split('.')
        .map(|p| p.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.is_empty() { None } else { Some(parts) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tagged_image_with_digest() {
        let img = "ghcr.io/tylerjw/eigenwallet-asb:4.5.0@sha256:abcdef";
        assert_eq!(parse_image_tag(img).as_deref(), Some("4.5.0"));
    }

    #[test]
    fn parses_tagged_image_without_digest() {
        assert_eq!(
            parse_image_tag("ghcr.io/tylerjw/eigenwallet-asb:4.5.0").as_deref(),
            Some("4.5.0")
        );
    }

    #[test]
    fn rejects_port_only_image() {
        assert_eq!(parse_image_tag("host:5000/repo"), None);
    }

    #[test]
    fn normalizes_v_prefix() {
        assert_eq!(normalize_tag("v4.5.1"), "4.5.1");
        assert_eq!(normalize_tag("4.5.1"), "4.5.1");
    }

    #[test]
    fn compares_versions() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("4.5.0", "4.5.1"), Some(Ordering::Less));
        assert_eq!(compare_versions("4.5.1", "4.5.0"), Some(Ordering::Greater));
        assert_eq!(compare_versions("4.5.0", "4.5.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("v4.5.0", "4.5.1"), Some(Ordering::Less));
        assert_eq!(compare_versions("4.6.0", "4.5.99"), Some(Ordering::Greater));
        // Different segment counts: 4.5 < 4.5.1
        assert_eq!(compare_versions("4.5", "4.5.1"), Some(Ordering::Less));
    }
}
