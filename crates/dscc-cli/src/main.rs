use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

#[cfg(test)]
use std::hash::{Hash, Hasher};
#[cfg(target_os = "linux")]
use std::path::Path;

use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use dscc_device::{
    list_sanitized_hid, list_sanitized_hid_with_access_probe, DeviceTransportKind, HidApiTransport,
    MockTransport, RawHidDevice, SanitizedHidDevice,
};
use serde::{Deserialize, Serialize};

const DEFAULT_AGENT_URL: &str = "http://127.0.0.1:43473";

#[derive(Debug, Parser)]
#[command(name = "dscc")]
#[command(about = "DualSense Command Center diagnostics and agent helper")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local loopback agent.
    Serve {
        /// Loopback address for the agent API.
        #[arg(long, default_value = dscc_agent::DEFAULT_BIND_ADDR)]
        addr: SocketAddr,
    },
    /// Print agent status from the local API.
    Status {
        /// Base URL for the agent API.
        #[arg(long, alias = "base-url", env = "DSCC_AGENT_URL", default_value = DEFAULT_AGENT_URL)]
        url: String,
    },
    /// Print OS-specific app directories used by the agent.
    Paths {
        /// Output JSON instead of a readable report.
        #[arg(long)]
        json: bool,
    },
    /// Device discovery and diagnostics commands.
    Devices {
        #[command(subcommand)]
        command: DevicesCommand,
    },
    /// Print a local mock controller list for demos and diagnostics.
    MockDevices {
        /// Output JSON instead of a readable table.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DevicesCommand {
    /// Print supported controllers from the agent registry.
    List(DeviceListArgs),
    /// Poll the agent registry and stream attach/detach/status events.
    Watch(DeviceWatchArgs),
    /// Print agent and platform device diagnostics.
    Diagnose(DeviceDiagnoseArgs),
    /// Print sanitized HID candidates for clean-room research.
    ListHid(DeviceListHidArgs),
}

#[derive(Debug, Args)]
struct AgentArgs {
    /// Base URL for the agent API.
    #[arg(long, alias = "base-url", env = "DSCC_AGENT_URL", default_value = DEFAULT_AGENT_URL)]
    url: String,
}

#[derive(Debug, Args)]
struct DeviceListArgs {
    #[command(flatten)]
    agent: AgentArgs,
    /// Output JSON instead of a readable table.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DeviceWatchArgs {
    #[command(flatten)]
    agent: AgentArgs,
    /// Output newline-delimited JSON events.
    #[arg(long)]
    json: bool,
    /// Polling interval for the HTTP fallback watcher.
    #[arg(long, default_value_t = 1_000)]
    poll_interval_ms: u64,
}

#[derive(Debug, Args)]
struct DeviceDiagnoseArgs {
    #[command(flatten)]
    agent: AgentArgs,
    /// Output JSON instead of a readable report.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DeviceListHidArgs {
    /// Acknowledge that raw HID listing is experimental and sanitized.
    #[arg(long)]
    experimental: bool,
    /// Output JSON instead of a readable table.
    #[arg(long)]
    json: bool,
    /// Use synthetic sanitized HID records instead of probing the host.
    #[arg(long)]
    mock: bool,
    /// Attempt to open supported Sony controller candidates and report sanitized access state.
    #[arg(long)]
    probe_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ControllerSummary {
    id: String,
    name: String,
    model: String,
    transport: String,
    connected: bool,
    battery_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DiagnosticsResponse {
    loopback_only: bool,
    hardware_required: bool,
    checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct HealthCheck {
    name: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SanitizedHidCandidate {
    source: String,
    path_hash: Option<String>,
    vendor_id: Option<String>,
    product_id: Option<String>,
    manufacturer: Option<String>,
    product: Option<String>,
    serial_present: bool,
    transport: String,
    usage_page: Option<String>,
    usage: Option<String>,
    interface_number: Option<String>,
    note: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct WatchEvent {
    event: WatchEventKind,
    controller: ControllerSummary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WatchEventKind {
    Attached,
    Detached,
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { addr } => dscc_agent::serve(addr).await,
        Command::Status { url } => print_status(&url).await,
        Command::Paths { json } => print_paths(json),
        Command::Devices { command } => match command {
            DevicesCommand::List(args) => devices_list(args).await,
            DevicesCommand::Watch(args) => devices_watch(args).await,
            DevicesCommand::Diagnose(args) => devices_diagnose(args).await,
            DevicesCommand::ListHid(args) => devices_list_hid(args),
        },
        Command::MockDevices { json } => print_mock_devices(json).await,
    }
}

fn print_paths(json: bool) -> anyhow::Result<()> {
    let paths = dscc_agent::app_paths().context("failed to resolve OS app directories")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&paths)?);
    } else {
        println!("config_dir: {}", paths.config_dir);
        println!("data_dir:   {}", paths.data_dir);
        println!("log_dir:    {}", paths.log_dir);
    }
    Ok(())
}

async fn print_status(url: &str) -> anyhow::Result<()> {
    let endpoint = api_endpoint(url, "/api/status");
    let status: serde_json::Value = http_client()?
        .get(&endpoint)
        .send()
        .await
        .with_context(|| format!("failed to reach {endpoint}"))?
        .error_for_status()
        .with_context(|| format!("agent returned an error for {endpoint}"))?
        .json()
        .await
        .context("agent status response was not valid JSON")?;

    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

async fn print_mock_devices(json: bool) -> anyhow::Result<()> {
    let mock = mock_controllers();

    if json {
        println!("{}", serde_json::to_string_pretty(&mock)?);
    } else {
        print_controller_table(&mock);
    }
    Ok(())
}

async fn devices_list(args: DeviceListArgs) -> anyhow::Result<()> {
    let client = http_client()?;
    let controllers = fetch_controllers(&client, &args.agent.url).await?;

    let controllers = supported_connected_controllers(&controllers);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&controllers)?);
    } else if controllers.is_empty() {
        println!("no supported controllers found.");
    } else {
        print_controller_table(&controllers);
    }

    Ok(())
}

async fn devices_watch(args: DeviceWatchArgs) -> anyhow::Result<()> {
    let client = http_client()?;
    let mut previous = BTreeMap::new();
    let interval = Duration::from_millis(args.poll_interval_ms.max(100));

    loop {
        let current = controller_map(fetch_controllers(&client, &args.agent.url).await?);
        for event in diff_watch_events(&previous, &current) {
            print_watch_event(&event, args.json)?;
        }
        previous = current;

        tokio::time::sleep(interval).await;
    }
}

async fn devices_diagnose(args: DeviceDiagnoseArgs) -> anyhow::Result<()> {
    let client = http_client()?;
    let diagnostics = fetch_diagnostics_or_mock(&client, &args.agent.url).await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    } else {
        print_diagnostics(&diagnostics);
    }

    Ok(())
}

fn devices_list_hid(args: DeviceListHidArgs) -> anyhow::Result<()> {
    if !args.experimental {
        bail!(
            "list-hid is experimental; rerun with --experimental to print sanitized HID candidates"
        );
    }

    let candidates = if args.mock {
        mock_hid_candidates()
    } else {
        enumerate_sanitized_hid_candidates(args.probe_open)
            .context("failed to enumerate sanitized HID candidates")?
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&candidates)?);
    } else if candidates.is_empty() {
        println!("no HID candidates found by the CLI fallback.");
    } else {
        print_hid_candidate_table(&candidates);
    }

    Ok(())
}

fn http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build HTTP client")
}

fn api_endpoint(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

async fn fetch_controllers(
    client: &reqwest::Client,
    base_url: &str,
) -> anyhow::Result<Vec<ControllerSummary>> {
    let endpoint = api_endpoint(base_url, "/api/controllers");
    client
        .get(&endpoint)
        .send()
        .await
        .with_context(|| format!("failed to reach {endpoint}"))?
        .error_for_status()
        .with_context(|| format!("agent returned an error for {endpoint}"))?
        .json()
        .await
        .context("agent controller response was not valid JSON")
}

async fn fetch_diagnostics_or_mock(
    client: &reqwest::Client,
    base_url: &str,
) -> DiagnosticsResponse {
    match fetch_diagnostics(client, base_url).await {
        Ok(mut diagnostics) => {
            diagnostics.checks.extend(platform_device_checks());
            diagnostics
        }
        Err(error) => fallback_diagnostics(error.to_string()),
    }
}

async fn fetch_diagnostics(
    client: &reqwest::Client,
    base_url: &str,
) -> anyhow::Result<DiagnosticsResponse> {
    let endpoint = api_endpoint(base_url, "/api/diagnostics");
    client
        .get(&endpoint)
        .send()
        .await
        .with_context(|| format!("failed to reach {endpoint}"))?
        .error_for_status()
        .with_context(|| format!("agent returned an error for {endpoint}"))?
        .json()
        .await
        .context("agent diagnostics response was not valid JSON")
}

fn fallback_diagnostics(reason: String) -> DiagnosticsResponse {
    let mut checks = vec![
        HealthCheck {
            name: "agent-api".to_string(),
            status: "unreachable".to_string(),
            detail: format!("Could not query the local agent; using CLI fallback. {reason}"),
        },
        HealthCheck {
            name: "device-backend".to_string(),
            status: "mock".to_string(),
            detail: "Agent registry is unavailable; use devices list-hid --experimental to query sanitized dscc-device HID enumeration directly.".to_string(),
        },
    ];
    checks.extend(platform_device_checks());

    DiagnosticsResponse {
        loopback_only: true,
        hardware_required: false,
        checks,
    }
}

fn platform_device_checks() -> Vec<HealthCheck> {
    let mut checks = Vec::new();

    #[cfg(target_os = "linux")]
    {
        checks.push(linux_hidraw_check());
        checks.push(HealthCheck {
            name: "linux-udev-access".to_string(),
            status: "manual_check".to_string(),
            detail: "If controllers enumerate but cannot be opened as a normal user, install provenance-approved udev rules for hidraw access.".to_string(),
        });
    }

    #[cfg(target_os = "windows")]
    {
        checks.push(HealthCheck {
            name: "windows-hid-backend".to_string(),
            status: "manual_check".to_string(),
            detail: "If discovery fails, verify the HID backend can load and no runtime packaging dependency is missing.".to_string(),
        });
        checks.push(HealthCheck {
            name: "windows-exclusive-access".to_string(),
            status: "manual_check".to_string(),
            detail: "If opening a controller fails, close software that may claim exclusive controller access, such as input remappers or game launchers.".to_string(),
        });
    }

    #[cfg(target_os = "macos")]
    {
        checks.push(HealthCheck {
            name: "macos-support".to_string(),
            status: "development_only".to_string(),
            detail: "macOS is useful for smoke checks, but the discovery plan targets Windows and Linux first.".to_string(),
        });
    }

    checks.push(HealthCheck {
        name: "bluetooth-limitations".to_string(),
        status: "manual_check".to_string(),
        detail: "Bluetooth discovery and status fields remain limited until dscc-device validation records platform behavior.".to_string(),
    });

    checks
}

#[cfg(target_os = "linux")]
fn linux_hidraw_check() -> HealthCheck {
    let hidraw = Path::new("/sys/class/hidraw");
    if hidraw.exists() {
        HealthCheck {
            name: "linux-hidraw".to_string(),
            status: "ok".to_string(),
            detail: "/sys/class/hidraw is present; use list-hid --experimental to inspect sanitized candidates.".to_string(),
        }
    } else {
        HealthCheck {
            name: "linux-hidraw".to_string(),
            status: "missing".to_string(),
            detail: "/sys/class/hidraw is not present; the HID kernel interface may be unavailable in this environment.".to_string(),
        }
    }
}

fn supported_connected_controllers(controllers: &[ControllerSummary]) -> Vec<ControllerSummary> {
    controllers
        .iter()
        .filter(|controller| controller.connected && is_supported_model(&controller.model))
        .cloned()
        .collect()
}

fn is_supported_model(model: &str) -> bool {
    matches!(model, "DualSense" | "DualSense Edge")
}

fn controller_map(controllers: Vec<ControllerSummary>) -> BTreeMap<String, ControllerSummary> {
    controllers
        .into_iter()
        .map(|controller| (controller.id.clone(), controller))
        .collect()
}

fn diff_watch_events(
    previous: &BTreeMap<String, ControllerSummary>,
    current: &BTreeMap<String, ControllerSummary>,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();

    for (id, controller) in current {
        let Some(prior) = previous.get(id) else {
            if controller.connected && is_supported_model(&controller.model) {
                events.push(WatchEvent {
                    event: WatchEventKind::Attached,
                    controller: controller.clone(),
                });
            }
            continue;
        };

        let was_supported = prior.connected && is_supported_model(&prior.model);
        let is_supported = controller.connected && is_supported_model(&controller.model);
        if !was_supported && is_supported {
            events.push(WatchEvent {
                event: WatchEventKind::Attached,
                controller: controller.clone(),
            });
        } else if was_supported && !is_supported {
            events.push(WatchEvent {
                event: WatchEventKind::Detached,
                controller: prior.clone(),
            });
        } else if is_supported && controller_status_changed(prior, controller) {
            events.push(WatchEvent {
                event: WatchEventKind::Status,
                controller: controller.clone(),
            });
        }
    }

    for (id, prior) in previous {
        if !current.contains_key(id) && prior.connected && is_supported_model(&prior.model) {
            events.push(WatchEvent {
                event: WatchEventKind::Detached,
                controller: prior.clone(),
            });
        }
    }

    events
}

fn controller_status_changed(prior: &ControllerSummary, current: &ControllerSummary) -> bool {
    prior.name != current.name
        || prior.model != current.model
        || prior.transport != current.transport
        || prior.battery_percent != current.battery_percent
}

fn print_controller_table(controllers: &[ControllerSummary]) {
    println!(
        "{:<24} {:<18} {:<10} {:<9} {:<7} NAME",
        "ID", "MODEL", "TRANSPORT", "CONNECTED", "BATTERY"
    );
    for controller in controllers {
        println!(
            "{:<24} {:<18} {:<10} {:<9} {:<7} {}",
            controller.id,
            controller.model,
            controller.transport,
            yes_no(controller.connected),
            format_battery(controller.battery_percent),
            controller.name
        );
    }
}

fn print_watch_event(event: &WatchEvent, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(event)?);
    } else {
        let kind = match event.event {
            WatchEventKind::Attached => "attached",
            WatchEventKind::Detached => "detached",
            WatchEventKind::Status => "status",
        };
        println!(
            "{:<8} id={} model={} transport={} connected={} battery={}",
            kind,
            event.controller.id,
            event.controller.model,
            event.controller.transport,
            yes_no(event.controller.connected),
            format_battery(event.controller.battery_percent)
        );
    }

    Ok(())
}

fn print_diagnostics(diagnostics: &DiagnosticsResponse) {
    println!("loopback_only: {}", yes_no(diagnostics.loopback_only));
    println!(
        "hardware_required: {}",
        yes_no(diagnostics.hardware_required)
    );
    println!("{:<24} {:<18} DETAIL", "CHECK", "STATUS");
    for check in &diagnostics.checks {
        println!("{:<24} {:<18} {}", check.name, check.status, check.detail);
    }
}

fn print_hid_candidate_table(candidates: &[SanitizedHidCandidate]) {
    println!(
        "{:<18} {:<18} {:<8} {:<8} {:<10} {:<6} PRODUCT",
        "SOURCE", "PATH_HASH", "VID", "PID", "TRANSPORT", "SERIAL"
    );
    for candidate in candidates {
        println!(
            "{:<18} {:<18} {:<8} {:<8} {:<10} {:<6} {}",
            candidate.source,
            candidate.path_hash.as_deref().unwrap_or("-"),
            candidate.vendor_id.as_deref().unwrap_or("-"),
            candidate.product_id.as_deref().unwrap_or("-"),
            candidate.transport,
            yes_no(candidate.serial_present),
            candidate.product.as_deref().unwrap_or("-")
        );
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn format_battery(value: Option<u8>) -> String {
    value
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "-".to_string())
}

fn mock_controllers() -> Vec<ControllerSummary> {
    vec![ControllerSummary {
        id: "mock-dualsense-1".to_string(),
        name: "Mock DualSense".to_string(),
        model: "DualSense".to_string(),
        transport: "mock".to_string(),
        connected: true,
        battery_percent: Some(88),
    }]
}

fn mock_hid_candidates() -> Vec<SanitizedHidCandidate> {
    let transport = MockTransport::with_devices(vec![RawHidDevice::mock("mock://hid-candidate-1")
        .with_manufacturer("Mock Manufacturer")
        .with_product("Mock HID candidate")
        .with_serial_number_present(true)]);
    hid_candidates_from_device_listing(
        "dscc-device-mock",
        list_sanitized_hid(&transport).unwrap_or_default(),
    )
}

fn enumerate_sanitized_hid_candidates(
    probe_open: bool,
) -> anyhow::Result<Vec<SanitizedHidCandidate>> {
    let transport = HidApiTransport::new().context("failed to initialize hidapi transport")?;
    let listing = if probe_open {
        list_sanitized_hid_with_access_probe(&transport)
            .context("failed to enumerate HID devices through hidapi with access probe")?
    } else {
        list_sanitized_hid(&transport).context("failed to enumerate HID devices through hidapi")?
    };
    Ok(hid_candidates_from_device_listing("hidapi", listing))
}

fn hid_candidates_from_device_listing(
    source: &str,
    listing: Vec<SanitizedHidDevice>,
) -> Vec<SanitizedHidCandidate> {
    listing
        .into_iter()
        .map(|device| SanitizedHidCandidate {
            source: source.to_string(),
            path_hash: Some(format!("h:{}", device.path_hint.backend_path_hash())),
            vendor_id: device.vendor_id.map(format_hex_u16),
            product_id: device.product_id.map(format_hex_u16),
            manufacturer: device.manufacturer,
            product: device.product,
            serial_present: device.serial_number_present,
            transport: format_device_transport(device.transport_hint).to_string(),
            usage_page: device.usage_page.map(format_hex_u16),
            usage: device.usage.map(format_hex_u16),
            interface_number: device.interface_number.map(|value| value.to_string()),
            note: format!(
                "Sanitized by dscc-device; family_hint={:?}; access={:?}.",
                device.family_hint, device.access
            ),
        })
        .collect()
}

fn format_device_transport(transport: DeviceTransportKind) -> &'static str {
    match transport {
        DeviceTransportKind::Usb => "usb",
        DeviceTransportKind::Bluetooth => "bluetooth",
        DeviceTransportKind::Unknown => "unknown",
    }
}

fn format_hex_u16(value: u16) -> String {
    format!("0x{value:04x}")
}

#[cfg(test)]
fn redacted_hash(value: &str) -> String {
    let mut hasher = Fnv1a64::default();
    value.hash(&mut hasher);
    format!("h:{:016x}", hasher.finish())
}

#[cfg(test)]
#[derive(Default)]
struct Fnv1a64(u64);

#[cfg(test)]
impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }

        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(id: &str, connected: bool, battery_percent: Option<u8>) -> ControllerSummary {
        ControllerSummary {
            id: id.to_string(),
            name: format!("Controller {id}"),
            model: "DualSense".to_string(),
            transport: "usb".to_string(),
            connected,
            battery_percent,
        }
    }

    #[test]
    fn supported_connected_controllers_excludes_unknown_and_disconnected() {
        let mut unknown = controller("unknown", true, None);
        unknown.model = "Unknown Sony Controller".to_string();
        let controllers = vec![
            controller("ok", true, Some(50)),
            controller("off", false, None),
            unknown,
        ];

        let supported = supported_connected_controllers(&controllers);

        assert_eq!(supported, vec![controller("ok", true, Some(50))]);
    }

    #[test]
    fn diff_watch_events_reports_attach_status_and_detach() {
        let previous = controller_map(vec![
            controller("stable", true, Some(80)),
            controller("removed", true, Some(70)),
            controller("newly-connected", false, None),
        ]);
        let current = controller_map(vec![
            controller("stable", true, Some(79)),
            controller("newly-connected", true, Some(100)),
        ]);

        let events = diff_watch_events(&previous, &current);
        let kinds = events.iter().map(|event| event.event).collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                WatchEventKind::Attached,
                WatchEventKind::Status,
                WatchEventKind::Detached
            ]
        );
    }

    #[test]
    fn api_endpoint_normalizes_slashes() {
        assert_eq!(
            api_endpoint("http://127.0.0.1:43473/", "/api/controllers"),
            "http://127.0.0.1:43473/api/controllers"
        );
    }

    #[test]
    fn redacted_hash_does_not_expose_input() {
        let value = "/dev/hidraw0/with/private/serial";
        let hash = redacted_hash(value);

        assert!(hash.starts_with("h:"));
        assert!(!hash.contains("hidraw0"));
        assert_ne!(hash, redacted_hash("/dev/hidraw1/with/private/serial"));
    }

    #[test]
    fn mock_hid_candidate_is_sanitized() {
        let candidates = mock_hid_candidates();

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0]
            .path_hash
            .as_deref()
            .unwrap()
            .starts_with("h:"));
        assert!(candidates[0].vendor_id.is_none());
        assert!(candidates[0].product_id.is_none());
    }
}
