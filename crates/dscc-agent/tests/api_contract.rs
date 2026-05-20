use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use dscc_agent::{
    app, AgentSnapshotResponse, AgentState, ControllerSummary, EffectTestResponse,
    IntegrationSummary, TelemetrySignalResponse,
};
use tower::ServiceExt;

#[tokio::test]
async fn agent_api_contract_serves_pre_hardware_runtime_state() {
    std::env::set_var("DSCC_PROCESS_SCAN_FIXTURE", "");
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

    let integrations: Vec<IntegrationSummary> =
        get_json(router.clone(), "/api/integrations", StatusCode::OK).await;
    assert!(integrations
        .iter()
        .any(|integration| integration.id == "forza-data-out"));
    assert!(integrations
        .iter()
        .any(|integration| integration.id == "forza-data-out" && integration.setup_url.is_some()));

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
        vec!["forza-horizon", "forza-horizon-immersive"]
    );
    assert!(snapshot
        .integrations
        .iter()
        .any(|integration| integration.id == "forza-data-out"));
    assert!(snapshot
        .diagnostics
        .checks
        .iter()
        .any(|check| check.name == "api" && check.status == "ok"));
    assert!(snapshot.effect_state.dry_run);
    assert!(snapshot.partial_errors.is_empty());

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
