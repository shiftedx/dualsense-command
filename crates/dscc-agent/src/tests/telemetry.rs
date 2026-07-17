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
