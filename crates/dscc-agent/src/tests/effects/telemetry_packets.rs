use super::*;

#[tokio::test]
async fn valid_forza_packet_switches_runtime_to_connected() {
    let state = AgentState::mock();
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_bound("127.0.0.1:5300".parse().unwrap());
    }
    let mut packet = vec![0_u8; 324];
    write_i32(&mut packet, 0, 1);
    write_f32(&mut packet, 8, 8_000.0);
    write_f32(&mut packet, 16, 6_000.0);
    write_f32(&mut packet, 244 + 12, 30.0);
    packet[244 + 71] = 204;
    let parsed =
        parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 7).expect("packet parses");

    state
        .apply_adapter_packet(parsed.adapter_id, parsed.packet_len, 7, parsed.updates)
        .await;

    let inner = state.inner.read().await;
    assert_eq!(inner.active_adapter_id.as_deref(), Some("forza-data-out"));
    assert_eq!(
        inner
            .require_adapter_runtime(FORZA_DATA_OUT_ADAPTER_ID)
            .packet_count,
        1
    );
    let adapters = materialized_adapters(&inner.adapters, &inner.adapter_runtimes, None);
    let forza = adapters
        .iter()
        .find(|adapter| adapter.id == "forza-data-out")
        .expect("Forza adapter exists");
    assert_eq!(forza.state, "connected");

    let telemetry = materialized_telemetry_response(&inner, Some(&detection));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "source.id" && signal.value == serde_json::json!("forza-data-out")
    }));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "game.id" && signal.value == serde_json::json!("forza-horizon-6")
    }));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "vehicle.speed_kmh" && signal.value == serde_json::json!(108.0)
    }));
}

#[tokio::test]
async fn forza_packet_rate_is_materialized_from_runtime_packets() {
    let state = AgentState::mock();
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_bound("127.0.0.1:5300".parse().unwrap());
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .rate_window_started_at = Some(Instant::now() - Duration::from_secs(2));
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .rate_window_packet_count = 119;
    }
    let mut packet = vec![0_u8; 324];
    write_i32(&mut packet, 0, 1);
    write_f32(&mut packet, 8, 8_000.0);
    write_f32(&mut packet, 16, 6_000.0);
    write_f32(&mut packet, 244 + 12, 30.0);
    packet[244 + 71] = 204;
    let parsed =
        parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, 9).expect("packet parses");

    state
        .apply_adapter_packet(parsed.adapter_id, parsed.packet_len, 9, parsed.updates)
        .await;

    let inner = state.inner.read().await;
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
    let forza = adapters
        .iter()
        .find(|adapter| adapter.id == "forza-data-out")
        .expect("Forza adapter exists");
    let packet_rate_hz = forza.packet_rate_hz.expect("packet rate is materialized");
    assert!((59..=60).contains(&packet_rate_hz));

    let telemetry = materialized_telemetry_response(&inner, Some(&detection));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "source.packet_rate_hz"
            && signal.value.as_f64() == Some(f64::from(packet_rate_hz))
    }));
}

#[tokio::test]
async fn short_forza_horizon_packet_gear_change_latches_shift_thump() {
    let state = AgentState::mock();
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);

    for (sequence, gear) in [(11, 3_u8), (12, 4_u8)] {
        let mut packet = vec![0_u8; 323];
        write_i32(&mut packet, 0, 1);
        write_f32(&mut packet, 8, 8_000.0);
        write_f32(&mut packet, 16, 6_000.0);
        write_f32(&mut packet, 244 + 12, 30.0);
        packet[244 + 71] = 255;
        packet[244 + 75] = gear;

        let parsed = parse_udp_telemetry_packet(FORZA_DATA_OUT_ADAPTER_ID, &packet, sequence)
            .expect("packet parses");
        state
            .apply_adapter_packet(
                parsed.adapter_id,
                parsed.packet_len,
                sequence,
                parsed.updates,
            )
            .await;
    }

    let inner = state.inner.read().await;
    assert_eq!(inner.telemetry.number("drivetrain.gear"), Some(4.0));
    let response = current_effect_response(&inner, Some(&detection), false);

    assert!(response
        .parity_effects
        .iter()
        .any(|effect| effect.id == "gear_shift_thump" && effect.state == "active"));
    match response.output.r2 {
        TriggerOutput::PulseAb {
            strength,
            frequency_hz,
            wall_zones,
        } => {
            assert!((frequency_hz - FORZA_SHIFT_FREQUENCY_HZ).abs() < f64::EPSILON);
            assert_eq!(wall_zones, 4);
            assert!(
                strength > 0.95,
                "shift thump should use the full configured wall-form kick, got {strength}"
            );
        }
        other => {
            panic!(
                "expected short Horizon gear change to drive R2 wall-form shift thump, got {other:?}"
            )
        }
    }
}
