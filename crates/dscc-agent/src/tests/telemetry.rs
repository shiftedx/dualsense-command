use super::support::*;
use super::*;

#[tokio::test]
async fn telemetry_endpoint_returns_empty_list_in_mock_state() {
    let router = app(AgentState::mock());

    let signals: Vec<TelemetrySignalResponse> =
        get_json(router, "/api/telemetry", StatusCode::OK).await;

    // Mock state has no active telemetry adapter; real adapters (e.g. Forza
    // Data Out) populate this list once they receive packets.
    assert!(signals.is_empty());
}

#[tokio::test]
async fn adapters_include_first_wave_catalog() {
    let router = app(AgentState::mock());

    let adapters: Vec<AdapterSummary> = get_json(router, "/api/adapters", StatusCode::OK).await;
    let ids = adapters
        .iter()
        .map(|adapter| adapter.id.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"forza-data-out"));
    assert!(ids.contains(&"ea-f1-udp"));
    assert!(ids.contains(&"beamng"));
    assert!(adapters
        .iter()
        .find(|adapter| adapter.id == "forza-data-out")
        .is_some_and(|adapter| adapter.setup_url.is_some()));
}

#[tokio::test]
async fn disabled_adapter_stays_disabled_and_ignores_packets() {
    let state = AgentState::mock();
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_bound("127.0.0.1:5300".parse().unwrap());
    }

    let response = app(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/adapters/forza-data-out")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"enabled":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let updated: AdapterSummary = serde_json::from_slice(&body).unwrap();
    assert!(!updated.enabled);
    assert_eq!(updated.state, "disabled");

    let adapters: Vec<AdapterSummary> =
        get_json(app(state.clone()), "/api/adapters", StatusCode::OK).await;
    let forza = adapters
        .iter()
        .find(|adapter| adapter.id == FORZA_DATA_OUT_ADAPTER_ID)
        .expect("Forza adapter exists");
    assert!(!forza.enabled);
    assert_eq!(forza.state, "disabled");

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

    let signals: Vec<TelemetrySignalResponse> =
        get_json(app(state.clone()), "/api/telemetry", StatusCode::OK).await;
    assert!(signals.is_empty());

    let inner = state.inner.read().await;
    assert_eq!(
        inner
            .require_adapter_runtime(FORZA_DATA_OUT_ADAPTER_ID)
            .packet_count,
        0
    );
    let persisted = PersistedAgentState::from_inner(&inner);
    assert_eq!(
        persisted.adapters.get(FORZA_DATA_OUT_ADAPTER_ID),
        Some(&PersistedAdapterState { enabled: false })
    );
    let restored = adapters_with_persisted_state(&persisted.adapters);
    let restored_forza = restored
        .iter()
        .find(|adapter| adapter.id == FORZA_DATA_OUT_ADAPTER_ID)
        .expect("Forza adapter exists");
    assert!(!restored_forza.enabled);
    assert_eq!(restored_forza.state, "disabled");
}

#[tokio::test]
async fn current_controller_effect_test_returns_dry_run_output() {
    let router = app(AgentState::mock());

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/controllers/current/test-effect")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"target":"r2","mode":"wall","intensity":72,"durationMs":500}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let effect: EffectTestResponse = serde_json::from_slice(&body).unwrap();
    assert!(effect.accepted);
    assert!(effect.dry_run);
    assert!(matches!(effect.output.r2, TriggerOutput::Wall { .. }));
}

fn racing_updates(adapter: &str, gear: f64, clutch: f64) -> Vec<SignalUpdate> {
    vec![
        signal_update("source.id", adapter),
        signal_update("game.state", "driving"),
        signal_update("drivetrain.gear", gear),
        signal_update("input.clutch", clutch),
    ]
}

#[tokio::test]
async fn audit_source_change_resets_racing_history() {
    let state = AgentState::mock();
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            1,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 3.0, 1.0),
        )
        .await;
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            2,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 4.0, 1.0),
        )
        .await;
    state
        .apply_adapter_packet(
            ASSETTO_SHARED_MEMORY_ADAPTER_ID,
            120,
            1,
            racing_updates(ASSETTO_SHARED_MEMORY_ADAPTER_ID, 2.0, 0.0),
        )
        .await;
    let inner = state.inner.read().await;
    assert_eq!(inner.telemetry.text("drivetrain.shift_event"), Some("none"));
    assert_eq!(
        inner
            .forza_effect_runtime
            .latched_shift_pulse(Instant::now()),
        0.0
    );
}

#[tokio::test]
async fn audit_frozen_shared_memory_does_not_refresh_freshness() {
    let state = AgentState::mock();
    let mut physics = vec![0; ASSETTO_PHYSICS_MIN_LEN];
    write_i32(&mut physics, 0, 42);
    write_i32(&mut physics, 20, 3000);
    let pages = AssettoSharedMemoryPages {
        physics: &physics,
        graphics: None,
        static_page: None,
    };
    let (len, updates) = parse_assetto_shared_memory_pages(pages, 1).unwrap();
    state
        .apply_adapter_packet(ASSETTO_SHARED_MEMORY_ADAPTER_ID, len, 1, updates)
        .await;
    let stale = Instant::now() - Duration::from_secs(3);
    state
        .inner
        .write()
        .await
        .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        .last_packet_at = Some(stale);
    let (len, updates) = parse_assetto_shared_memory_pages(pages, 2).unwrap();
    state
        .apply_adapter_packet(ASSETTO_SHARED_MEMORY_ADAPTER_ID, len, 2, updates)
        .await;
    let inner = state.inner.read().await;
    assert!(!inner
        .require_adapter_runtime(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        .has_recent_packet(Instant::now()));
    assert!(!current_effect_snapshot(&inner, None).1);
    let output = current_effect_response(&inner, None, false).output;
    assert_eq!(output.l2, TriggerOutput::Off);
    assert_eq!(output.r2, TriggerOutput::Off);
    assert_eq!(output.rumble, ControllerOutputFrame::default().rumble);
    drop(inner);
    write_i32(&mut physics, 0, 43);
    let (len, updates) = parse_assetto_shared_memory_pages(
        AssettoSharedMemoryPages {
            physics: &physics,
            graphics: None,
            static_page: None,
        },
        3,
    )
    .unwrap();
    state
        .apply_adapter_packet(ASSETTO_SHARED_MEMORY_ADAPTER_ID, len, 3, updates)
        .await;
    assert!(current_effect_snapshot(&*state.inner.read().await, None).1);
}

#[tokio::test]
async fn audit_adapter_disable_invalidates_fresh_telemetry() {
    let state = AgentState::mock();
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            1,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 3.0, 0.0),
        )
        .await;
    for enabled in [false, true] {
        let _ = crate::api::update_adapter(
            Path(FORZA_DATA_OUT_ADAPTER_ID.to_string()),
            State(state.clone()),
            Json(UpdateAdapterRequest { enabled }),
        )
        .await
        .unwrap();
        let inner = state.inner.read().await;
        assert!(!current_effect_snapshot(&inner, None).1);
        assert!(!inner
            .require_adapter_runtime(FORZA_DATA_OUT_ADAPTER_ID)
            .has_recent_packet(Instant::now()));
    }
}

#[tokio::test]
async fn audit_new_racing_session_resets_clutch_and_gear() {
    let state = AgentState::mock();
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            1,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 3.0, 1.0),
        )
        .await;
    state
        .inner
        .write()
        .await
        .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
        .last_packet_at = Some(Instant::now() - Duration::from_secs(3));
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            2,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 1.0, 0.0),
        )
        .await;
    assert_eq!(
        state
            .inner
            .read()
            .await
            .telemetry
            .text("drivetrain.shift_event"),
        Some("none")
    );
    state
        .apply_adapter_packet(
            FORZA_DATA_OUT_ADAPTER_ID,
            324,
            3,
            racing_updates(FORZA_DATA_OUT_ADAPTER_ID, 2.0, 0.0),
        )
        .await;
    assert_eq!(
        state
            .inner
            .read()
            .await
            .telemetry
            .text("drivetrain.shift_event"),
        Some("shift")
    );
}

#[tokio::test]
async fn audit_shared_memory_wrap_and_restart_are_new_samples() {
    let state = AgentState::mock();
    for (sequence, packet_id) in [i32::MAX, i32::MIN, 0, 1].into_iter().enumerate() {
        let mut physics = vec![0; ASSETTO_PHYSICS_MIN_LEN];
        write_i32(&mut physics, 0, packet_id);
        write_i32(&mut physics, 20, 3000);
        let (len, updates) = parse_assetto_shared_memory_pages(
            AssettoSharedMemoryPages {
                physics: &physics,
                graphics: None,
                static_page: None,
            },
            sequence as u64,
        )
        .unwrap();
        state
            .apply_adapter_packet(
                ASSETTO_SHARED_MEMORY_ADAPTER_ID,
                len,
                sequence as u64,
                updates,
            )
            .await;
        let inner = state.inner.read().await;
        assert!(inner
            .require_adapter_runtime(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .has_recent_packet(Instant::now()));
        assert_eq!(
            inner.telemetry.number("source.sample_id"),
            Some(f64::from(packet_id))
        );
        assert_eq!(
            inner
                .require_adapter_runtime(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                .packet_count,
            sequence as u64 + 1
        );
    }
}

#[tokio::test]
async fn audit_absent_shared_memory_accepts_restarted_same_id() {
    let state = AgentState::mock();
    let mut physics = vec![0; ASSETTO_PHYSICS_MIN_LEN];
    write_i32(&mut physics, 0, 42);
    write_i32(&mut physics, 20, 3000);
    for sequence in 1..=2 {
        let (len, updates) = parse_assetto_shared_memory_pages(
            AssettoSharedMemoryPages {
                physics: &physics,
                graphics: None,
                static_page: None,
            },
            sequence,
        )
        .unwrap();
        state
            .apply_adapter_packet(ASSETTO_SHARED_MEMORY_ADAPTER_ID, len, sequence, updates)
            .await;
        let mut inner = state.inner.write().await;
        assert_eq!(
            inner
                .require_adapter_runtime(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
                .packet_count,
            sequence
        );
        assert!(current_effect_snapshot(&inner, None).1);
        inner
            .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .mark_mapping_absent();
        assert!(!current_effect_snapshot(&inner, None).1);
    }
}
