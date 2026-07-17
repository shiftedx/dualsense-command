//! Built-in telemetry adapter catalog and deterministic test adapters.
//!
//! Game-specific packet parsers live behind this crate boundary. Until a
//! source has public documentation and provenance notes, adapters can be
//! exposed as setup-ready catalog entries without parsing private data.

#![forbid(unsafe_code)]

use dscc_telemetry::{
    AdapterCapabilities, AdapterConfig, AdapterDetection, SignalName, SignalUpdate, SignalValue,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use dscc_telemetry::SignalSnapshot;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BuiltInAdapter {
    pub id: &'static str,
    pub display_name: &'static str,
    pub protocol: AdapterProtocol,
    pub default_port: Option<u16>,
    pub packet_formats: &'static [&'static str],
    pub setup_hint: &'static str,
    pub setup_url: Option<&'static str>,
    pub enabled_by_default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterProtocol {
    Synthetic,
    Udp,
    SharedMemory,
    Sdk,
    Custom,
}

pub fn built_in_adapters() -> &'static [BuiltInAdapter] {
    &[
        BuiltInAdapter {
            id: "forza-data-out",
            display_name: "Forza Data Out",
            protocol: AdapterProtocol::Udp,
            default_port: Some(5300),
            packet_formats: &["sled", "dash", "horizon", "extended_dash"],
            setup_hint: "Enable UDP Race Telemetry in-game, set the target IP to localhost or this PC's LAN IP, and match the configured port.",
            setup_url: Some("https://support.forzamotorsport.net/hc/en-us/articles/21742934024211-Forza-Motorsport-Data-Out-Documentation"),
            enabled_by_default: false,
        },
        BuiltInAdapter {
            id: "ea-f1-udp",
            display_name: "EA F1 UDP Telemetry",
            protocol: AdapterProtocol::Udp,
            default_port: Some(20777),
            packet_formats: &["f1-25", "f1-24"],
            setup_hint: "Enable UDP telemetry in the F1 game settings and use a supported packet format.",
            setup_url: Some("https://forums.ea.com/blog/f1-games-game-info-hub-en/f1%C2%AE-25-udp-specification/12187347"),
            enabled_by_default: false,
        },
        BuiltInAdapter {
            id: "assetto-shared-memory",
            display_name: "Assetto Shared Memory",
            protocol: AdapterProtocol::SharedMemory,
            default_port: None,
            packet_formats: &["acpmf"],
            setup_hint: "Launch Assetto Corsa Rally on Windows; DSCC reads the public Assetto shared-memory pages when a driving session is active.",
            setup_url: Some("https://www.assettocorsamods.net/threads/doc-shared-memory-reference.58/"),
            enabled_by_default: true,
        },
        BuiltInAdapter {
            id: "ea-wrc-udp",
            display_name: "EA SPORTS WRC UDP",
            protocol: AdapterProtocol::Udp,
            default_port: Some(20777),
            packet_formats: &["wrc-v1.3"],
            setup_hint: "Configure the game's UDP telemetry file, then bind DSCC to the same port.",
            setup_url: Some("https://forums.ea.com/t5/s/tghpe58374/attachments/tghpe58374/wrc-general-discussion-en/2667/1/EA%20SPORTS%20WRC%20-%20UDP%20Telemetry%20Guide%20%28v1.3%29.pdf"),
            enabled_by_default: false,
        },
        BuiltInAdapter {
            id: "beamng",
            display_name: "BeamNG.drive",
            protocol: AdapterProtocol::Udp,
            default_port: Some(4444),
            packet_formats: &["outgauge", "motionsim"],
            setup_hint: "Use BeamNG's documented protocols for OutGauge, MotionSim, or a custom Lua bridge.",
            setup_url: Some("https://documentation.beamng.com/modding/protocols/"),
            enabled_by_default: false,
        },
        BuiltInAdapter {
            id: "live-for-speed",
            display_name: "Live for Speed",
            protocol: AdapterProtocol::Udp,
            default_port: Some(29999),
            packet_formats: &["insim", "outgauge"],
            setup_hint: "Configure LFS InSim or OutGauge and match DSCC's bind port.",
            setup_url: Some("https://en.lfsmanual.net/wiki/InSim"),
            enabled_by_default: false,
        },
        BuiltInAdapter {
            id: "raceroom",
            display_name: "RaceRoom",
            protocol: AdapterProtocol::SharedMemory,
            default_port: None,
            packet_formats: &["shared-memory"],
            setup_hint: "Uses RaceRoom's shared-memory API when running on a supported host.",
            setup_url: Some("https://github.com/kwstudios-sweden/r3e-api"),
            enabled_by_default: false,
        },
    ]
}

pub fn adapter_by_id(id: &str) -> Option<&'static BuiltInAdapter> {
    built_in_adapters().iter().find(|adapter| adapter.id == id)
}

pub fn default_config_for(adapter: &BuiltInAdapter) -> AdapterConfig {
    AdapterConfig {
        enabled: adapter.enabled_by_default,
        auto_detect: true,
        bind_address: Some("127.0.0.1".to_string()),
        port: adapter.default_port,
        packet_format: adapter
            .packet_formats
            .first()
            .map(|format| (*format).to_string()),
        setup_url: adapter.setup_url.map(str::to_string),
        setup_text: Some(adapter.setup_hint.to_string()),
    }
}

pub fn capabilities_for(adapter: &BuiltInAdapter) -> AdapterCapabilities {
    AdapterCapabilities {
        udp_listener: adapter.protocol == AdapterProtocol::Udp,
        shared_memory: adapter.protocol == AdapterProtocol::SharedMemory,
        requires_setup: adapter.protocol != AdapterProtocol::Synthetic,
        supports_auto_detect: true,
        packet_formats: adapter
            .packet_formats
            .iter()
            .map(|format| (*format).to_string())
            .collect(),
    }
}

pub fn initial_detection(adapter: &BuiltInAdapter, enabled: bool) -> AdapterDetection {
    if !enabled {
        return AdapterDetection::Unavailable {
            reason: Some("adapter disabled".to_string()),
        };
    }

    match adapter.protocol {
        AdapterProtocol::Synthetic => AdapterDetection::Running,
        AdapterProtocol::Udp | AdapterProtocol::SharedMemory | AdapterProtocol::Sdk => {
            AdapterDetection::NeedsSetup {
                instructions: Some(adapter.setup_hint.to_string()),
            }
        }
        AdapterProtocol::Custom => AdapterDetection::Ready,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UdpTelemetryParseResult {
    pub adapter_id: &'static str,
    pub packet_format: &'static str,
    pub packet_len: usize,
    pub updates: Vec<SignalUpdate>,
}

#[derive(Clone, Copy)]
pub struct UdpTelemetryAdapter {
    pub id: &'static str,
    pub display_name: &'static str,
    pub default_port: u16,
    pub parse_packet: fn(&[u8], u64) -> Option<UdpTelemetryParseResult>,
}

pub fn built_in_udp_adapters() -> &'static [UdpTelemetryAdapter] {
    &[UdpTelemetryAdapter {
        id: "forza-data-out",
        display_name: "Forza Data Out",
        default_port: 5300,
        parse_packet: parse_forza_udp_packet,
    }]
}

pub fn udp_adapter_by_id(id: &str) -> Option<&'static UdpTelemetryAdapter> {
    built_in_udp_adapters()
        .iter()
        .find(|adapter| adapter.id == id)
}

pub fn parse_udp_telemetry_packet(
    adapter_id: &str,
    packet: &[u8],
    sequence: u64,
) -> Option<UdpTelemetryParseResult> {
    let adapter = udp_adapter_by_id(adapter_id)?;
    (adapter.parse_packet)(packet, sequence)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForzaPacketKind {
    Sled232,
    Dash311,
    Horizon324,
    ExtendedDash,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForzaParseResult {
    pub kind: ForzaPacketKind,
    pub packet_len: usize,
    pub updates: Vec<SignalUpdate>,
}

impl ForzaPacketKind {
    pub fn packet_format(self) -> &'static str {
        match self {
            ForzaPacketKind::Sled232 => "sled",
            ForzaPacketKind::Dash311 => "dash",
            ForzaPacketKind::Horizon324 => "horizon",
            ForzaPacketKind::ExtendedDash => "extended_dash",
        }
    }
}

fn parse_forza_udp_packet(packet: &[u8], sequence: u64) -> Option<UdpTelemetryParseResult> {
    let parsed = parse_forza_data_out_packet(packet, sequence)?;
    Some(UdpTelemetryParseResult {
        adapter_id: "forza-data-out",
        packet_format: parsed.kind.packet_format(),
        packet_len: parsed.packet_len,
        updates: parsed.updates,
    })
}

pub fn parse_forza_data_out_packet(packet: &[u8], sequence: u64) -> Option<ForzaParseResult> {
    if packet.len() < 232 {
        return None;
    }

    let kind = match packet.len() {
        232 => ForzaPacketKind::Sled232,
        311 => ForzaPacketKind::Dash311,
        323 => ForzaPacketKind::Horizon324,
        324 => ForzaPacketKind::Horizon324,
        len if len > 311 => ForzaPacketKind::ExtendedDash,
        _ => return None,
    };
    let dash_base = match kind {
        ForzaPacketKind::Horizon324 => 244,
        ForzaPacketKind::Dash311 | ForzaPacketKind::ExtendedDash => 232,
        ForzaPacketKind::Sled232 => usize::MAX,
    };

    let is_race_on = read_i32(packet, 0)?;
    let max_rpm = read_f32(packet, 8)? as f64;
    let current_rpm = read_f32(packet, 16)? as f64;
    let speed_ms = if dash_base != usize::MAX && packet.len() >= dash_base + 79 {
        read_f32(packet, dash_base + 12).map(f64::from)
    } else {
        velocity_speed_ms(packet)
    }
    .unwrap_or_default();
    let rpm_ratio = if max_rpm > 0.0 {
        (current_rpm / max_rpm).clamp(0.0, 1.25)
    } else {
        0.0
    };
    let acceleration_x = read_f32_f64(packet, 20)?;
    let acceleration_y = read_f32_f64(packet, 24)?;
    let acceleration_z = read_f32_f64(packet, 28)?;
    let acceleration_magnitude = acceleration_x
        .mul_add(
            acceleration_x,
            acceleration_y.mul_add(acceleration_y, acceleration_z * acceleration_z),
        )
        .sqrt();
    let rumble_strip = read_i32(packet, 116)?
        .abs()
        .max(read_i32(packet, 120)?.abs())
        .max(read_i32(packet, 124)?.abs())
        .max(read_i32(packet, 128)?.abs()) as f64;
    let puddle_depth = read_f32(packet, 132)?
        .abs()
        .max(read_f32(packet, 136)?.abs())
        .max(read_f32(packet, 140)?.abs())
        .max(read_f32(packet, 144)?.abs()) as f64;
    let tire_slip_ratio = read_f32(packet, 84)?
        .abs()
        .max(read_f32(packet, 88)?.abs())
        .max(read_f32(packet, 92)?.abs())
        .max(read_f32(packet, 96)?.abs()) as f64;
    let tire_slip_angle = read_f32(packet, 164)?
        .abs()
        .max(read_f32(packet, 168)?.abs())
        .max(read_f32(packet, 172)?.abs())
        .max(read_f32(packet, 176)?.abs()) as f64;
    let front_slip = read_f32(packet, 180)?
        .abs()
        .max(read_f32(packet, 184)?.abs()) as f64;
    let rear_slip = read_f32(packet, 188)?
        .abs()
        .max(read_f32(packet, 192)?.abs()) as f64;
    let suspension_travel = read_f32(packet, 196)?
        .abs()
        .max(read_f32(packet, 200)?.abs())
        .max(read_f32(packet, 204)?.abs())
        .max(read_f32(packet, 208)?.abs()) as f64;
    let rumble = read_f32(packet, 148)?
        .abs()
        .max(read_f32(packet, 152)?.abs())
        .max(read_f32(packet, 156)?.abs())
        .max(read_f32(packet, 160)?.abs()) as f64;

    let mut updates = Vec::with_capacity(40);
    updates.extend([
        update("source.id", "forza-data-out", sequence),
        update("source.connected", true, sequence),
        update("source.packet_size", packet.len() as f64, sequence),
        update(
            "game.state",
            if is_race_on == 1 { "driving" } else { "menu" },
            sequence,
        ),
        update("vehicle.max_rpm", finite(max_rpm), sequence),
        update("vehicle.rpm", finite(current_rpm), sequence),
        update("vehicle.rpm_ratio", finite(rpm_ratio), sequence),
        update("vehicle.speed_kmh", finite(speed_ms * 3.6), sequence),
        update("vehicle.acceleration.x", finite(acceleration_x), sequence),
        update("vehicle.acceleration.y", finite(acceleration_y), sequence),
        update("vehicle.acceleration.z", finite(acceleration_z), sequence),
        update(
            "vehicle.acceleration.magnitude",
            finite(acceleration_magnitude),
            sequence,
        ),
        update("tire.slip_ratio.max", finite(tire_slip_ratio), sequence),
        update("tire.slip_angle.max", finite(tire_slip_angle), sequence),
        update(
            "wheel.slip.front_left",
            read_f32_f64(packet, 180)?,
            sequence,
        ),
        update(
            "wheel.slip.front_right",
            read_f32_f64(packet, 184)?,
            sequence,
        ),
        update("wheel.slip.rear_left", read_f32_f64(packet, 188)?, sequence),
        update(
            "wheel.slip.rear_right",
            read_f32_f64(packet, 192)?,
            sequence,
        ),
        update("wheel.slip.front_max", finite(front_slip), sequence),
        update("wheel.slip.rear_max", finite(rear_slip), sequence),
        update(
            "wheel.slip.max",
            finite(front_slip.max(rear_slip)),
            sequence,
        ),
        update(
            "surface.rumble.max",
            finite(rumble.clamp(0.0, 1.0)),
            sequence,
        ),
        update(
            "surface.rumble_strip.max",
            finite(rumble_strip.clamp(0.0, 1.0)),
            sequence,
        ),
        update(
            "surface.puddle.max",
            finite(puddle_depth.clamp(0.0, 1.0)),
            sequence,
        ),
        update("suspension.travel.max", finite(suspension_travel), sequence),
    ]);

    if dash_base != usize::MAX && packet.len() >= dash_base + 79 {
        updates.extend([
            update(
                "input.throttle",
                input_u8(packet, dash_base + 71)?,
                sequence,
            ),
            update("input.brake", input_u8(packet, dash_base + 72)?, sequence),
            update("input.clutch", input_u8(packet, dash_base + 73)?, sequence),
            update(
                "input.handbrake",
                input_u8(packet, dash_base + 74)?,
                sequence,
            ),
            update(
                "drivetrain.gear",
                f64::from(read_u8(packet, dash_base + 75)?),
                sequence,
            ),
            update("input.steer", steer_i8(packet, dash_base + 76)?, sequence),
            update(
                "vehicle.power",
                read_f32_f64(packet, dash_base + 16)?,
                sequence,
            ),
            update(
                "vehicle.torque",
                read_f32_f64(packet, dash_base + 20)?,
                sequence,
            ),
            update(
                "vehicle.boost",
                read_f32_f64(packet, dash_base + 40)?,
                sequence,
            ),
        ]);
    } else {
        updates.extend([
            update("input.throttle", 0.0, sequence),
            update("input.brake", 0.0, sequence),
            update("input.clutch", 0.0, sequence),
            update("input.handbrake", 0.0, sequence),
            update("drivetrain.gear", 0.0, sequence),
            update("input.steer", 0.0, sequence),
        ]);
    }

    Some(ForzaParseResult {
        kind,
        packet_len: packet.len(),
        updates,
    })
}

fn signal(name: &str) -> SignalName {
    SignalName::new(name).expect("built-in signal name is valid")
}

fn update(name: &str, value: impl Into<SignalValue>, sequence: u64) -> SignalUpdate {
    SignalUpdate::new(signal(name), value).with_sequence(sequence)
}

fn read_bytes<const N: usize>(packet: &[u8], offset: usize) -> Option<[u8; N]> {
    packet.get(offset..offset + N)?.try_into().ok()
}

fn read_i32(packet: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(read_bytes(packet, offset)?))
}

fn read_u8(packet: &[u8], offset: usize) -> Option<u8> {
    packet.get(offset).copied()
}

fn read_i8(packet: &[u8], offset: usize) -> Option<i8> {
    Some(read_u8(packet, offset)? as i8)
}

fn read_f32(packet: &[u8], offset: usize) -> Option<f32> {
    Some(f32::from_le_bytes(read_bytes(packet, offset)?))
}

fn read_f32_f64(packet: &[u8], offset: usize) -> Option<f64> {
    Some(finite(f64::from(read_f32(packet, offset)?)))
}

fn input_u8(packet: &[u8], offset: usize) -> Option<f64> {
    Some(f64::from(read_u8(packet, offset)?) / 255.0)
}

fn steer_i8(packet: &[u8], offset: usize) -> Option<f64> {
    Some((f64::from(read_i8(packet, offset)?) / 127.0).clamp(-1.0, 1.0))
}

fn velocity_speed_ms(packet: &[u8]) -> Option<f64> {
    let x = f64::from(read_f32(packet, 32)?);
    let y = f64::from(read_f32(packet, 36)?);
    let z = f64::from(read_f32(packet, 40)?);
    Some((x.mul_add(x, y.mul_add(y, z * z))).sqrt())
}

fn finite(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_i32(packet: &mut [u8], offset: usize, value: i32) {
        packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f32(packet: &mut [u8], offset: usize, value: f32) {
        packet[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn catalog_contains_first_wave_adapters() {
        let ids = built_in_adapters()
            .iter()
            .map(|adapter| adapter.id)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"forza-data-out"));
        assert!(ids.contains(&"ea-f1-udp"));
        assert!(ids.contains(&"assetto-shared-memory"));
        assert!(ids.contains(&"ea-wrc-udp"));
        assert!(ids.contains(&"beamng"));
        assert!(ids.contains(&"raceroom"));
    }

    #[test]
    fn udp_adapter_registry_exposes_forza_parser() {
        let adapter = udp_adapter_by_id("forza-data-out").expect("Forza UDP adapter is registered");

        assert_eq!(adapter.display_name, "Forza Data Out");
        assert_eq!(adapter.default_port, 5300);
        assert!(udp_adapter_by_id("ea-f1-udp").is_none());
    }

    #[test]
    fn generic_udp_parser_wraps_forza_packets() {
        let mut packet = vec![0_u8; 324];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 204;
        packet[244 + 72] = 64;
        packet[244 + 75] = 4;

        let result = parse_udp_telemetry_packet("forza-data-out", &packet, 19)
            .expect("registered parser accepts Forza packets");
        let snapshot = SignalSnapshot::from_updates(result.updates);

        assert_eq!(result.adapter_id, "forza-data-out");
        assert_eq!(result.packet_format, "horizon");
        assert_eq!(result.packet_len, 324);
        assert_eq!(snapshot.text("source.id"), Some("forza-data-out"));
        assert_eq!(snapshot.number("vehicle.speed_kmh"), Some(108.0));
    }

    #[test]
    fn forza_horizon_packet_normalizes_core_signals() {
        let mut packet = vec![0_u8; 324];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 20, 3.0);
        write_f32(&mut packet, 24, 4.0);
        write_f32(&mut packet, 28, 12.0);
        write_i32(&mut packet, 116, 1);
        write_f32(&mut packet, 132, 0.33);
        write_f32(&mut packet, 84, 0.24);
        write_f32(&mut packet, 164, 0.31);
        write_f32(&mut packet, 180, 0.15);
        write_f32(&mut packet, 188, 0.42);
        write_f32(&mut packet, 196, 0.08);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 204;
        packet[244 + 72] = 64;
        packet[244 + 75] = 4;
        packet[244 + 76] = 12_i8 as u8;

        let result = parse_forza_data_out_packet(&packet, 9).expect("packet parses");
        let snapshot = SignalSnapshot::from_updates(result.updates);

        assert_eq!(result.kind, ForzaPacketKind::Horizon324);
        assert_eq!(snapshot.text("source.id"), Some("forza-data-out"));
        assert_eq!(snapshot.text("game.state"), Some("driving"));
        assert_eq!(snapshot.number("vehicle.speed_kmh"), Some(108.0));
        assert_eq!(snapshot.number("vehicle.rpm_ratio"), Some(0.75));
        assert_eq!(snapshot.number("surface.rumble_strip.max"), Some(1.0));
        assert_eq!(
            snapshot.number("surface.puddle.max"),
            Some(0.33000001311302185)
        );
        assert_eq!(
            snapshot.number("tire.slip_ratio.max"),
            Some(0.23999999463558197)
        );
        assert_eq!(
            snapshot.number("tire.slip_angle.max"),
            Some(0.3100000023841858)
        );
        assert!(snapshot
            .number("vehicle.acceleration.magnitude")
            .is_some_and(|value| (value - 13.0).abs() < 0.001));
        assert!(snapshot
            .number("input.throttle")
            .is_some_and(|value| (value - 0.8).abs() < 0.001));
        assert_eq!(snapshot.number("drivetrain.gear"), Some(4.0));
    }

    #[test]
    fn short_forza_horizon_packet_uses_horizon_dash_offsets() {
        let mut packet = vec![0_u8; 323];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 204;
        packet[244 + 72] = 64;
        packet[244 + 75] = 4;
        packet[244 + 76] = 12_i8 as u8;

        let result = parse_forza_data_out_packet(&packet, 10).expect("packet parses");
        let snapshot = SignalSnapshot::from_updates(result.updates);

        assert_eq!(result.kind, ForzaPacketKind::Horizon324);
        assert_eq!(snapshot.number("vehicle.speed_kmh"), Some(108.0));
        assert!(snapshot
            .number("input.throttle")
            .is_some_and(|value| (value - 0.8).abs() < 0.001));
        assert_eq!(snapshot.number("input.brake"), Some(64.0 / 255.0));
        assert_eq!(snapshot.number("drivetrain.gear"), Some(4.0));
        assert_eq!(snapshot.number("input.steer"), Some(12.0 / 127.0));
    }
}
