use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use dscc_adapters::{
    built_in_adapters, built_in_udp_adapters, AdapterProtocol, BuiltInAdapter, UdpTelemetryAdapter,
};
use dscc_telemetry::AdapterDetection;

use crate::game_modules::{ASSETTO_SHARED_MEMORY_ADAPTER_ID, FORZA_DATA_OUT_ADAPTER_ID};
use crate::{AdapterSummary, GameDetectionResponse, HealthCheck};

pub(crate) const TELEMETRY_PACKET_STALE_AFTER: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub(crate) struct AdapterRuntime {
    pub(crate) adapter_id: String,
    pub(crate) display_name: String,
    pub(crate) protocol: AdapterProtocol,
    pub(crate) default_port: Option<u16>,
    pub(crate) bind_addr: Option<SocketAddr>,
    pub(crate) listener_bound: bool,
    pub(crate) listener_started_at: Option<Instant>,
    pub(crate) last_error: Option<String>,
    pub(crate) packet_count: u64,
    pub(crate) packet_rate_hz: Option<u16>,
    pub(crate) rate_window_started_at: Option<Instant>,
    pub(crate) rate_window_packet_count: u64,
    pub(crate) first_packet_at: Option<Instant>,
    pub(crate) last_packet_at: Option<Instant>,
    pub(crate) last_packet_len: Option<usize>,
    pub(crate) last_packet_sequence: Option<u64>,
    pub(crate) parse_error_count: u64,
    pub(crate) last_parse_error_len: Option<usize>,
    pub(crate) last_parse_error: Option<String>,
    pub(crate) last_parse_error_at: Option<Instant>,
}

impl Default for AdapterRuntime {
    fn default() -> Self {
        Self {
            adapter_id: String::new(),
            display_name: String::new(),
            protocol: AdapterProtocol::Custom,
            default_port: None,
            bind_addr: None,
            listener_bound: false,
            listener_started_at: None,
            last_error: None,
            packet_count: 0,
            packet_rate_hz: None,
            rate_window_started_at: None,
            rate_window_packet_count: 0,
            first_packet_at: None,
            last_packet_at: None,
            last_packet_len: None,
            last_packet_sequence: None,
            parse_error_count: 0,
            last_parse_error_len: None,
            last_parse_error: None,
            last_parse_error_at: None,
        }
    }
}

impl AdapterRuntime {
    pub(crate) fn for_udp_adapter(adapter: UdpTelemetryAdapter) -> Self {
        Self {
            adapter_id: adapter.id.to_string(),
            display_name: adapter.display_name.to_string(),
            protocol: AdapterProtocol::Udp,
            default_port: Some(adapter.default_port),
            ..Self::default()
        }
    }

    pub(crate) fn for_built_in_adapter(adapter: &BuiltInAdapter) -> Self {
        Self {
            adapter_id: adapter.id.to_string(),
            display_name: adapter.display_name.to_string(),
            protocol: adapter.protocol,
            default_port: adapter.default_port,
            ..Self::default()
        }
    }

    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn mark_ready(&mut self) {
        self.listener_bound = true;
        self.listener_started_at.get_or_insert_with(Instant::now);
        self.last_error = None;
    }

    pub(crate) fn mark_bound(&mut self, bind_addr: SocketAddr) {
        self.bind_addr = Some(bind_addr);
        self.listener_bound = true;
        self.listener_started_at = Some(Instant::now());
        self.last_error = None;
    }

    pub(crate) fn mark_bind_error(&mut self, bind_addr: SocketAddr, error: impl Into<String>) {
        self.bind_addr = Some(bind_addr);
        self.listener_bound = false;
        self.last_error = Some(error.into());
    }

    pub(crate) fn mark_packet(&mut self, packet_len: usize, sequence: u64) -> u16 {
        let now = Instant::now();
        let window_start = *self.rate_window_started_at.get_or_insert(now);
        self.rate_window_packet_count = self.rate_window_packet_count.saturating_add(1);
        let window_seconds = now.duration_since(window_start).as_secs_f64();
        let rate = if window_seconds >= 1.0 {
            let rate = ((self.rate_window_packet_count.saturating_sub(1)) as f64 / window_seconds)
                .round()
                .clamp(1.0, 1000.0) as u16;
            self.rate_window_started_at = Some(now);
            self.rate_window_packet_count = 1;
            rate
        } else {
            self.packet_rate_hz.unwrap_or_default()
        };

        self.packet_count = self.packet_count.saturating_add(1);
        self.packet_rate_hz = Some(rate);
        self.first_packet_at.get_or_insert(now);
        self.last_packet_at = Some(now);
        self.last_packet_len = Some(packet_len);
        self.last_packet_sequence = Some(sequence);
        self.last_error = None;
        rate
    }

    pub(crate) fn mark_parse_error(&mut self, packet_len: usize, sequence: u64) {
        self.parse_error_count = self.parse_error_count.saturating_add(1);
        if self.last_parse_error_len != Some(packet_len) {
            self.last_parse_error = Some(format!(
                "unsupported {} packet length {packet_len}",
                self.display_name
            ));
            self.last_parse_error_len = Some(packet_len);
        }
        self.last_parse_error_at = Some(Instant::now());
        self.last_packet_sequence = Some(sequence);
    }

    pub(crate) fn has_recent_packet(&self, now: Instant) -> bool {
        self.last_packet_at.is_some_and(|last_packet_at| {
            now.duration_since(last_packet_at) <= TELEMETRY_PACKET_STALE_AFTER
        })
    }
}

pub(crate) fn default_adapter_runtimes() -> BTreeMap<String, AdapterRuntime> {
    let mut runtimes: BTreeMap<String, AdapterRuntime> = built_in_adapters()
        .iter()
        .map(|adapter| {
            (
                adapter.id.to_string(),
                AdapterRuntime::for_built_in_adapter(adapter),
            )
        })
        .collect();
    for adapter in built_in_udp_adapters() {
        runtimes.insert(
            adapter.id.to_string(),
            AdapterRuntime::for_udp_adapter(*adapter),
        );
    }
    runtimes
}

pub(crate) fn materialized_adapters(
    adapters: &[AdapterSummary],
    runtimes: &BTreeMap<String, AdapterRuntime>,
    game_detection: Option<&GameDetectionResponse>,
) -> Vec<AdapterSummary> {
    let now = Instant::now();
    let mut adapters = adapters.to_vec();
    for adapter in &mut adapters {
        if let Some(runtime) = runtimes.get(&adapter.id) {
            apply_adapter_runtime_summary(adapter, runtime, game_detection, now);
        }
    }
    adapters
}

pub(crate) fn apply_adapter_runtime_summary(
    adapter: &mut AdapterSummary,
    runtime: &AdapterRuntime,
    game_detection: Option<&GameDetectionResponse>,
    now: Instant,
) {
    if !adapter.enabled {
        adapter.state = "disabled".to_string();
        adapter.packet_rate_hz = None;
        return;
    }

    let bind_addr = runtime
        .bind_addr
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| default_adapter_bind_addr(runtime));
    let detected_game = detected_adapter_game_name(game_detection, &runtime.adapter_id);

    if !runtime.listener_bound {
        if let Some(error) = runtime.last_error.as_ref() {
            adapter.state = "faulted".to_string();
            adapter.packet_rate_hz = None;
            adapter.setup_hint = if runtime.protocol == AdapterProtocol::SharedMemory {
                format!(
                    "DSCC could not start the {} reader: {error}",
                    runtime.display_name
                )
            } else {
                format!(
                    "DSCC could not bind the {} UDP listener on {bind_addr}: {error}",
                    runtime.display_name
                )
            };
        }
        return;
    }

    if runtime.has_recent_packet(now) {
        adapter.state = "connected".to_string();
        adapter.packet_rate_hz = runtime.packet_rate_hz;
        let packet_len = runtime.last_packet_len.unwrap_or_default();
        adapter.setup_hint = format!(
            "Receiving {} via {}; last packet was {packet_len} bytes {}.",
            runtime.display_name,
            runtime_transport_label(runtime, &bind_addr),
            runtime
                .last_packet_at
                .map(|last| format_elapsed_brief(now.duration_since(last)))
                .unwrap_or_else(|| "just now".to_string())
        );
        return;
    }

    adapter.packet_rate_hz = Some(0);
    if runtime.packet_count > 0 {
        adapter.state = "needs_setup".to_string();
        adapter.setup_hint = format!(
            "{} is ready via {}, but the stream is stale; last packet arrived {}.",
            runtime.display_name,
            runtime_transport_label(runtime, &bind_addr),
            runtime
                .last_packet_at
                .map(|last| format_elapsed_brief(now.duration_since(last)))
                .unwrap_or_else(|| "earlier".to_string())
        );
    } else if runtime.adapter_id == FORZA_DATA_OUT_ADAPTER_ID {
        if let Some(game_name) = detected_game {
            adapter.state = "needs_setup".to_string();
            adapter.setup_hint = format!(
                "{game_name} is running and DSCC is listening on {bind_addr}, but no Data Out packets have arrived. Enable UDP Race Telemetry in-game, set target IP to 127.0.0.1, use port 5300, then enter a driving session."
            );
        } else {
            adapter.state = "ready".to_string();
            adapter.setup_hint = format!(
                "DSCC is listening on {bind_addr}; launch a supported Forza title and enable UDP Race Telemetry."
            );
        }
    } else if runtime.adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
        if let Some(game_name) = detected_game {
            adapter.state = "needs_setup".to_string();
            adapter.setup_hint = format!(
                "{game_name} is running and DSCC is watching Assetto shared memory, but no live physics page is available yet. Load into a driving session and make sure the game is not paused."
            );
        } else {
            adapter.state = "ready".to_string();
            adapter.setup_hint = "DSCC is watching Assetto shared memory; launch Assetto Corsa Rally and enter a driving session.".to_string();
        }
    } else {
        adapter.state = "needs_setup".to_string();
        adapter.setup_hint = format!(
            "DSCC is listening on {bind_addr}; configure {} to send UDP telemetry to this adapter.",
            runtime.display_name
        );
    }
}

fn default_adapter_bind_addr(runtime: &AdapterRuntime) -> String {
    match runtime.default_port {
        Some(port) => format!("127.0.0.1:{port}"),
        None if runtime.protocol == AdapterProtocol::SharedMemory => "shared-memory".to_string(),
        None => "127.0.0.1".to_string(),
    }
}

fn runtime_transport_label<'a>(runtime: &AdapterRuntime, bind_addr: &'a str) -> &'a str {
    match runtime.protocol {
        AdapterProtocol::SharedMemory => "shared memory",
        _ => bind_addr,
    }
}

fn detected_adapter_game_name<'a>(
    game_detection: Option<&'a GameDetectionResponse>,
    adapter_id: &str,
) -> Option<&'a str> {
    game_detection.and_then(|detection| {
        (detection.adapter_id.as_deref() == Some(adapter_id))
            .then_some(detection.active_game_name.as_deref())
            .flatten()
    })
}

fn format_elapsed_brief(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds == 0 {
        "just now".to_string()
    } else if seconds < 60 {
        format!("{seconds}s ago")
    } else {
        format!("{}m {}s ago", seconds / 60, seconds % 60)
    }
}

pub(crate) fn adapter_runtime_health_check(
    runtime: &AdapterRuntime,
    game_detection: Option<&GameDetectionResponse>,
) -> HealthCheck {
    let now = Instant::now();
    let bind_addr = runtime
        .bind_addr
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| default_adapter_bind_addr(runtime));

    if !runtime.listener_bound {
        return HealthCheck {
            name: runtime.adapter_id.clone(),
            status: if runtime.last_error.is_some() {
                "blocked".to_string()
            } else {
                "pending".to_string()
            },
            detail: runtime.last_error.clone().unwrap_or_else(|| {
                format!(
                    "{} listener has not reported ready on {bind_addr}",
                    runtime.display_name
                )
            }),
        };
    }

    if runtime.has_recent_packet(now) {
        return HealthCheck {
            name: runtime.adapter_id.clone(),
            status: "ok".to_string(),
            detail: format!(
                "Receiving {} byte packets on {bind_addr} at {} Hz",
                runtime.last_packet_len.unwrap_or_default(),
                runtime.packet_rate_hz.unwrap_or_default()
            ),
        };
    }

    let detected_game = detected_adapter_game_name(game_detection, &runtime.adapter_id);
    let status = if detected_game.is_some() {
        "warning"
    } else {
        "ok"
    };
    let mut detail = if let Some(game_name) = detected_game {
        if runtime.protocol == AdapterProtocol::SharedMemory {
            format!(
                "{game_name} is running; shared-memory reader is ready, but no live physics page is available"
            )
        } else {
            format!(
                "{game_name} is running; listener is ready on {bind_addr}, but no live Data Out packets are arriving"
            )
        }
    } else if runtime.adapter_id == FORZA_DATA_OUT_ADAPTER_ID {
        format!(
            "Listener is ready on {bind_addr}; telemetry will activate when a supported Forza title is running"
        )
    } else if runtime.adapter_id == ASSETTO_SHARED_MEMORY_ADAPTER_ID {
        "Shared-memory reader is ready; telemetry will activate when Assetto Corsa Rally is running in a driving session".to_string()
    } else {
        format!(
            "Listener is ready on {bind_addr}; telemetry will activate when a supported source sends packets"
        )
    };
    if let Some(last_packet_at) = runtime.last_packet_at {
        detail = format!(
            "{detail}; last valid packet arrived {}",
            format_elapsed_brief(now.duration_since(last_packet_at))
        );
    }
    if let Some(last_parse_error) = runtime.last_parse_error.as_ref() {
        detail = format!("{detail}; {last_parse_error}");
    }

    HealthCheck {
        name: runtime.adapter_id.clone(),
        status: status.to_string(),
        detail,
    }
}

pub(crate) fn adapter_state_label(detection: &AdapterDetection) -> &'static str {
    match detection {
        AdapterDetection::Unavailable { .. } => "disabled",
        AdapterDetection::NeedsSetup { .. } => "needs_setup",
        AdapterDetection::Ready => "ready",
        AdapterDetection::Running => "connected",
        AdapterDetection::Faulted { .. } => "faulted",
    }
}
