use std::time::Duration;

use serde::Deserialize;

use crate::{current_timestamp, UpdateCheckResponse};

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const UPDATE_CHECK_URL: &str =
    "https://api.github.com/repos/shiftedx/dualsense-command/releases/latest";

#[derive(Debug, Deserialize)]
pub(crate) struct GithubReleaseResponse {
    pub(crate) tag_name: String,
    pub(crate) html_url: String,
    pub(crate) name: Option<String>,
    pub(crate) published_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VersionOrdering {
    Older,
    SameOrNewer,
    Unknown,
}

pub(crate) async fn fetch_latest_release_update_check() -> anyhow::Result<UpdateCheckResponse> {
    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .user_agent(format!(
            "DualSenseCommandCenter/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;
    let response = client
        .get(UPDATE_CHECK_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("GitHub Releases request failed with HTTP {status}");
    }
    let release = response.json::<GithubReleaseResponse>().await?;
    Ok(update_check_from_release(
        env!("CARGO_PKG_VERSION"),
        release,
        current_timestamp(),
    ))
}

pub(crate) fn update_check_from_release(
    current_version: &str,
    release: GithubReleaseResponse,
    checked_at: String,
) -> UpdateCheckResponse {
    let latest_version = normalize_release_version(&release.tag_name);
    let state = match compare_release_versions(current_version, &latest_version) {
        VersionOrdering::Older => "update_available",
        VersionOrdering::SameOrNewer => "up_to_date",
        VersionOrdering::Unknown => "unknown",
    };
    UpdateCheckResponse {
        current_version: current_version.to_string(),
        latest_version: Some(latest_version),
        release_url: Some(release.html_url),
        release_name: release.name,
        published_at: release.published_at,
        state: state.to_string(),
        checked_at: Some(checked_at),
        error: None,
        cached: false,
    }
}

pub(crate) fn unavailable_update_check(error: String) -> UpdateCheckResponse {
    UpdateCheckResponse {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: None,
        release_url: None,
        release_name: None,
        published_at: None,
        state: "unavailable".to_string(),
        checked_at: Some(current_timestamp()),
        error: Some(error),
        cached: false,
    }
}

fn normalize_release_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

pub(crate) fn compare_release_versions(current: &str, latest: &str) -> VersionOrdering {
    let Some(current) = parse_version_core(current) else {
        return VersionOrdering::Unknown;
    };
    let Some(latest) = parse_version_core(latest) else {
        return VersionOrdering::Unknown;
    };
    if current < latest {
        VersionOrdering::Older
    } else {
        VersionOrdering::SameOrNewer
    }
}

fn parse_version_core(version: &str) -> Option<Vec<u64>> {
    let normalized = normalize_release_version(version);
    let core = normalized
        .split_once(['-', '+'])
        .map(|(core, _)| core)
        .unwrap_or(normalized.as_str());
    let parts: Option<Vec<u64>> = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect();
    let mut parts = parts?;
    if parts.is_empty() {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}
