use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use dscc_agent::{
    app, AdapterSummary, AgentSnapshotResponse, AgentState, ControllerSummary, EffectTestResponse,
    TelemetrySignalResponse,
};
use tower::ServiceExt;

#[tokio::test]
async fn agent_api_contract_serves_pre_hardware_runtime_state() {
    let router = app(AgentState::mock());

    let controllers: Vec<ControllerSummary> =
        get_json(router.clone(), "/api/controllers", StatusCode::OK).await;
    assert_eq!(controllers.len(), 2);
    assert!(controllers
        .iter()
        .any(|controller| controller.model == "DualSense"));
    assert!(controllers
        .iter()
        .any(|controller| controller.model == "DualSense Edge"));

    let adapters: Vec<AdapterSummary> =
        get_json(router.clone(), "/api/adapters", StatusCode::OK).await;
    assert!(adapters
        .iter()
        .any(|adapter| adapter.id == "forza-data-out"));
    assert!(adapters
        .iter()
        .any(|adapter| adapter.id == "assetto-shared-memory"));
    assert!(adapters
        .iter()
        .any(|adapter| adapter.id == "forza-data-out" && adapter.setup_url.is_some()));
    assert_status(router.clone(), "/api/integrations", StatusCode::NOT_FOUND).await;

    let telemetry: Vec<TelemetrySignalResponse> =
        get_json(router.clone(), "/api/telemetry", StatusCode::OK).await;
    // Mock state has no active telemetry adapter; real adapters populate this
    // list once they receive packets.
    assert!(telemetry.is_empty());

    let snapshot: AgentSnapshotResponse =
        get_json(router.clone(), "/api/snapshot", StatusCode::OK).await;
    assert_eq!(snapshot.controllers.len(), 2);
    let profile_ids: Vec<&str> = snapshot
        .profiles
        .iter()
        .map(|profile| profile.id.as_str())
        .collect();
    assert_eq!(
        profile_ids,
        vec![
            "global",
            "forza-horizon",
            "forza-horizon-immersive",
            "assetto-corsa-rally"
        ]
    );
    assert!(snapshot
        .adapters
        .iter()
        .any(|adapter| adapter.id == "forza-data-out"));
    assert!(snapshot
        .diagnostics
        .checks
        .iter()
        .any(|check| check.name == "api" && check.status == "ok"));
    assert!(snapshot.effect_state.dry_run);
    assert!(snapshot.partial_errors.is_empty());

    let snapshot_json: serde_json::Value =
        get_json(router.clone(), "/api/snapshot", StatusCode::OK).await;
    assert!(snapshot_json.get("appSettings").is_some());
    assert!(snapshot_json.get("gameDetection").is_some());
    assert!(snapshot_json.get("profileResolution").is_some());
    assert!(snapshot_json.get("adapters").is_some());
    assert!(snapshot_json.get("app_settings").is_none());
    assert!(snapshot_json.get("integrations").is_none());
    assert!(snapshot_json
        .get("status")
        .and_then(|status| status.get("active_adapter_id"))
        .is_some());
    assert!(snapshot_json
        .get("status")
        .and_then(|status| status.get("active_integration_id"))
        .is_none());
    assert!(snapshot_json
        .get("profileResolution")
        .and_then(|resolution| resolution.get("activeAdapterId"))
        .is_some());
    assert!(snapshot_json
        .get("profileResolution")
        .and_then(|resolution| resolution.get("activeIntegrationId"))
        .is_none());
    assert!(snapshot_json
        .get("modules")
        .and_then(|modules| modules.as_array())
        .is_some_and(|modules| modules.iter().any(|module| {
            module.get("id").and_then(|id| id.as_str()) == Some("forza-data-out")
                && module.get("kind").and_then(|kind| kind.as_str()) == Some("adapter")
        })));

    let effect: EffectTestResponse = post_json(
        router,
        "/api/controllers/current/test-effect",
        r#"{"target":"r2","mode":"adaptive_resistance","intensity":64,"durationMs":650}"#,
        StatusCode::ACCEPTED,
    )
    .await;
    assert!(effect.accepted);
    assert!(effect.dry_run);
}

async fn assert_status(router: axum::Router, uri: &str, expected: StatusCode) {
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), expected);
}

async fn get_json<T>(router: axum::Router, uri: &str, expected: StatusCode) -> T
where
    T: serde::de::DeserializeOwned,
{
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), expected);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn post_json<T>(router: axum::Router, uri: &str, body: &str, expected: StatusCode) -> T
where
    T: serde::de::DeserializeOwned,
{
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), expected);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
