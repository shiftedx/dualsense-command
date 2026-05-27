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
