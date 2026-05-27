use super::{http_get_body, TrayMenuAccent, SNAPSHOT_PATH, TRAY_HEALTH_REFRESH_INTERVAL};
use serde::Deserialize;
use std::{
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrayHealthSummary {
    pub(super) agent_running: bool,
    pub(super) agent_label: String,
    pub(super) agent_detail: String,
    pub(super) agent_accent: TrayMenuAccent,
    pub(super) profile_label: String,
    pub(super) profile_detail: String,
    pub(super) profile_accent: TrayMenuAccent,
    pub(super) controller_label: String,
    pub(super) controller_detail: String,
    pub(super) controller_accent: TrayMenuAccent,
    pub(super) diagnostics_label: String,
    pub(super) diagnostics_detail: String,
    pub(super) diagnostics_accent: TrayMenuAccent,
}

#[derive(Debug, Clone)]
pub(super) struct TrayHealthCache {
    pub(super) summary: TrayHealthSummary,
    pub(super) refreshed_at: Instant,
}

#[derive(Debug, Deserialize)]
pub(super) struct TraySnapshotDto {
    status: TraySnapshotStatusDto,
    #[serde(default)]
    profiles: Vec<TraySnapshotProfileDto>,
    #[serde(default)]
    controllers: Vec<TraySnapshotControllerDto>,
    #[serde(default, alias = "profileResolution")]
    profile_resolution: TraySnapshotProfileResolutionDto,
    diagnostics: TraySnapshotDiagnosticsDto,
}

#[derive(Debug, Deserialize)]
struct TraySnapshotStatusDto {
    #[serde(default)]
    version: String,
    #[serde(default)]
    active_profile_id: Option<String>,
    #[serde(default)]
    active_adapter_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TraySnapshotProfileDto {
    id: String,
    name: String,
    #[serde(default)]
    active: bool,
}

#[derive(Debug, Deserialize)]
struct TraySnapshotControllerDto {
    id: String,
    name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    transport: String,
    #[serde(default)]
    connected: bool,
}

#[derive(Debug, Default, Deserialize)]
struct TraySnapshotProfileResolutionDto {
    #[serde(default, alias = "controllerId")]
    controller_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TraySnapshotDiagnosticsDto {
    #[serde(default)]
    checks: Vec<TraySnapshotHealthCheckDto>,
}

#[derive(Debug, Deserialize)]
struct TraySnapshotHealthCheckDto {
    status: String,
}

pub(super) fn refreshing_health_summary() -> TrayHealthSummary {
    TrayHealthSummary {
        agent_running: false,
        agent_label: "Agent Status".to_string(),
        agent_detail: "Refreshing local runtime state".to_string(),
        agent_accent: TrayMenuAccent::Neutral,
        profile_label: "Profile Pending".to_string(),
        profile_detail: "Waiting for snapshot".to_string(),
        profile_accent: TrayMenuAccent::Neutral,
        controller_label: "Controller Pending".to_string(),
        controller_detail: "Waiting for snapshot".to_string(),
        controller_accent: TrayMenuAccent::Neutral,
        diagnostics_label: "Diagnostics Pending".to_string(),
        diagnostics_detail: "Waiting for health checks".to_string(),
        diagnostics_accent: TrayMenuAccent::Neutral,
    }
}

pub(super) fn offline_health_summary() -> TrayHealthSummary {
    TrayHealthSummary {
        agent_running: false,
        agent_label: "Agent Offline".to_string(),
        agent_detail: "Start the agent to enable controller control".to_string(),
        agent_accent: TrayMenuAccent::Danger,
        profile_label: "Profile Unavailable".to_string(),
        profile_detail: "Start the agent to read profile state".to_string(),
        profile_accent: TrayMenuAccent::Neutral,
        controller_label: "Controller Unavailable".to_string(),
        controller_detail: "Start the agent to read controller state".to_string(),
        controller_accent: TrayMenuAccent::Neutral,
        diagnostics_label: "Diagnostics Unavailable".to_string(),
        diagnostics_detail: "Waiting for the local runtime".to_string(),
        diagnostics_accent: TrayMenuAccent::Neutral,
    }
}

pub(super) fn spawn_tray_health_worker(
    cache: Arc<Mutex<TrayHealthCache>>,
    refresh_rx: Receiver<()>,
) {
    thread::spawn(move || {
        refresh_tray_health_cache(&cache);
        while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) =
            refresh_rx.recv_timeout(TRAY_HEALTH_REFRESH_INTERVAL)
        {
            while refresh_rx.try_recv().is_ok() {}
            refresh_tray_health_cache(&cache);
        }
    });
}

fn refresh_tray_health_cache(cache: &Arc<Mutex<TrayHealthCache>>) {
    let summary = fetch_tray_health_summary().unwrap_or_else(offline_health_summary);
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.summary = summary;
    cache.refreshed_at = Instant::now();
}

fn fetch_tray_health_summary() -> Option<TrayHealthSummary> {
    let body = http_get_body(SNAPSHOT_PATH, Duration::from_millis(900))?;
    let snapshot = serde_json::from_str::<TraySnapshotDto>(&body).ok()?;
    Some(tray_health_summary_from_snapshot(&snapshot))
}

pub(super) fn tray_health_summary_from_snapshot(snapshot: &TraySnapshotDto) -> TrayHealthSummary {
    let version = if snapshot.status.version.trim().is_empty() {
        "unknown"
    } else {
        snapshot.status.version.trim()
    };
    let active_profile_id = snapshot.status.active_profile_id.as_deref().or_else(|| {
        snapshot
            .profiles
            .iter()
            .find(|profile| profile.active)
            .map(|profile| profile.id.as_str())
    });
    let active_adapter = snapshot.status.active_adapter_id.as_deref();
    let (profile_label, profile_detail, profile_accent) =
        active_profile_summary(active_profile_id, &snapshot.profiles);
    let (controller_label, controller_detail, controller_accent) = active_controller_summary(
        snapshot.profile_resolution.controller_id.as_deref(),
        &snapshot.controllers,
    );
    let agent_detail = match (active_profile_id, active_adapter) {
        (_, Some(adapter)) => format!("v{version} - telemetry via {adapter}"),
        (Some(_), None) => format!("v{version} - profile ready"),
        _ => format!("v{version} - local runtime ready"),
    };

    let statuses = snapshot
        .diagnostics
        .checks
        .iter()
        .map(|check| check.status.as_str())
        .collect::<Vec<_>>();
    let (diagnostics_label, diagnostics_detail, diagnostics_accent) =
        diagnostics_summary_from_statuses(&statuses);

    TrayHealthSummary {
        agent_running: true,
        agent_label: "Agent Online".to_string(),
        agent_detail,
        agent_accent: TrayMenuAccent::Ready,
        profile_label,
        profile_detail,
        profile_accent,
        controller_label,
        controller_detail,
        controller_accent,
        diagnostics_label,
        diagnostics_detail,
        diagnostics_accent,
    }
}

fn diagnostics_summary_from_statuses(statuses: &[&str]) -> (String, String, TrayMenuAccent) {
    if statuses.is_empty() {
        return (
            "Diagnostics Warming Up".to_string(),
            "No checks reported yet".to_string(),
            TrayMenuAccent::Neutral,
        );
    }

    let pending = statuses
        .iter()
        .filter(|status| **status == "pending")
        .count();
    let attention = statuses
        .iter()
        .filter(|status| {
            !matches!(
                **status,
                "ok" | "hidapi" | "pending" | "ready" | "connected"
            )
        })
        .count();

    if attention > 0 {
        (
            "Diagnostics Need Attention".to_string(),
            format!("{attention} of {} checks need review", statuses.len()),
            TrayMenuAccent::Danger,
        )
    } else if pending > 0 {
        (
            "Diagnostics Warming Up".to_string(),
            format!(
                "{pending} check warming up, {} checks healthy",
                statuses.len() - pending
            ),
            TrayMenuAccent::Neutral,
        )
    } else {
        (
            "Diagnostics Clear".to_string(),
            format!("{} checks healthy", statuses.len()),
            TrayMenuAccent::Ready,
        )
    }
}

fn active_profile_summary(
    active_profile_id: Option<&str>,
    profiles: &[TraySnapshotProfileDto],
) -> (String, String, TrayMenuAccent) {
    let Some(profile_id) = active_profile_id else {
        return (
            "Profile: None".to_string(),
            "No active profile selected".to_string(),
            TrayMenuAccent::Neutral,
        );
    };
    let profile_name = profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| profile.name.clone())
        .unwrap_or_else(|| fallback_profile_name(profile_id));
    (
        format!("Profile: {profile_name}"),
        profile_id.to_string(),
        TrayMenuAccent::Ready,
    )
}

fn active_controller_summary(
    active_controller_id: Option<&str>,
    controllers: &[TraySnapshotControllerDto],
) -> (String, String, TrayMenuAccent) {
    let controller = active_controller_id
        .and_then(|id| controllers.iter().find(|controller| controller.id == id))
        .or_else(|| controllers.iter().find(|controller| controller.connected))
        .or_else(|| controllers.first());

    let Some(controller) = controller else {
        return (
            "Controller: None".to_string(),
            "No controller detected".to_string(),
            TrayMenuAccent::Neutral,
        );
    };

    let label = if controller.name.trim().is_empty() {
        fallback_controller_name(&controller.model)
    } else {
        controller.name.trim().to_string()
    };
    let detail = [
        fallback_controller_name(&controller.model),
        transport_label(&controller.transport),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" / ");

    (
        format!("Controller: {label}"),
        if detail.is_empty() {
            controller.id.clone()
        } else {
            detail
        },
        if controller.connected {
            TrayMenuAccent::Ready
        } else {
            TrayMenuAccent::Neutral
        },
    )
}

fn fallback_controller_name(model: &str) -> String {
    let normalized = model.trim();
    if normalized.is_empty() {
        "DualSense".to_string()
    } else if normalized.eq_ignore_ascii_case("dualsense_edge") {
        "DualSense Edge".to_string()
    } else if normalized.eq_ignore_ascii_case("dualsense") {
        "DualSense".to_string()
    } else {
        normalized.replace('_', " ")
    }
}

fn transport_label(transport: &str) -> String {
    match transport.trim().to_ascii_lowercase().as_str() {
        "usb" => "USB".to_string(),
        "bluetooth" => "Bluetooth".to_string(),
        "" | "unknown" => String::new(),
        value => value.to_string(),
    }
}

pub(super) fn fallback_profile_name(profile_id: &str) -> String {
    match profile_id {
        "global" => "Global".to_string(),
        "forza-horizon" => "Base".to_string(),
        "forza-horizon-immersive" => "Immersive".to_string(),
        "assetto-corsa-rally" => "Rally".to_string(),
        _ => profile_id.to_string(),
    }
}
