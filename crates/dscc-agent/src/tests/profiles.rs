use super::support::*;
use super::*;

fn cycle_summary(id: &str) -> ProfileSummary {
    ProfileSummary {
        id: id.to_string(),
        name: id.to_string(),
        built_in: false,
        active: false,
        game_id: None,
    }
}

#[test]
fn cycled_profile_wraps_in_ui_order() {
    let profiles = vec![cycle_summary("a"), cycle_summary("b"), cycle_summary("c")];
    assert_eq!(
        cycled_profile_id(&profiles, Some("a"), true).as_deref(),
        Some("b")
    );
    assert_eq!(
        cycled_profile_id(&profiles, Some("c"), true).as_deref(),
        Some("a")
    );
    assert_eq!(
        cycled_profile_id(&profiles, Some("a"), false).as_deref(),
        Some("c")
    );
}

#[test]
fn cycled_profile_requires_two_profiles() {
    assert_eq!(
        cycled_profile_id(&[cycle_summary("only")], Some("only"), true),
        None
    );
    assert_eq!(cycled_profile_id(&[], None, true), None);
}

#[test]
fn cycled_profile_starts_from_first_when_none_active() {
    let profiles = vec![cycle_summary("a"), cycle_summary("b")];
    assert_eq!(
        cycled_profile_id(&profiles, None, true).as_deref(),
        Some("b")
    );
    assert_eq!(
        cycled_profile_id(&profiles, Some("missing"), false).as_deref(),
        Some("b")
    );
}

#[tokio::test]
async fn profile_can_be_created_and_activated() {
    let router = app(AgentState::mock());
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Track Focus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles/track-focus/activate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let status: StatusResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(status.active_profile_id.as_deref(), Some("track-focus"));
}

#[tokio::test]
async fn profile_create_and_export_preserve_game_scope() {
    let router = app(AgentState::mock());
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"Horizon Rally","gameId":"forza-horizon-6"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let created: ProfileSummary = serde_json::from_slice(&body).unwrap();
    assert_eq!(created.game_id.as_deref(), Some("forza-horizon-6"));

    let exported: ExportedProfile =
        get_json(router, "/api/profiles/horizon-rally/export", StatusCode::OK).await;
    assert_eq!(exported.game_id.as_deref(), Some("forza-horizon-6"));
}

#[tokio::test]
async fn custom_profile_can_be_renamed() {
    let router = app(AgentState::mock());
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Track Focus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/profiles/track-focus")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Endurance Focus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let renamed: ProfileSummary = serde_json::from_slice(&body).unwrap();
    assert_eq!(renamed.id, "track-focus");
    assert_eq!(renamed.name, "Endurance Focus");
    assert!(!renamed.built_in);

    let profile: ProfileSummary =
        get_json(router, "/api/profiles/track-focus", StatusCode::OK).await;
    assert_eq!(profile.name, "Endurance Focus");
}

#[tokio::test]
async fn built_in_profile_cannot_be_renamed() {
    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/profiles/forza-horizon")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Renamed Built In"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn custom_profile_can_be_deleted_and_active_profile_falls_back() {
    let router = app(AgentState::mock());
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Track Focus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles/track-focus/activate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/profiles/track-focus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let accepted: ActionAccepted = serde_json::from_slice(&body).unwrap();
    assert!(accepted.accepted);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let status: StatusResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        status.active_profile_id.as_deref(),
        Some(DEFAULT_PROFILE_ID)
    );

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/profiles/track-focus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn profiles_can_be_exported_and_imported() {
    let router = app(AgentState::mock());

    let exported: ExportedProfile = get_json(
        router.clone(),
        "/api/profiles/global/export",
        StatusCode::OK,
    )
    .await;
    assert_eq!(exported.schema, "dev.dscc.profile.v1");
    assert_eq!(exported.id, DEFAULT_PROFILE_ID);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"schema":"dev.dscc.profile.v1","id":"imported-road","name":"Imported Road"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bad_schema = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"schema":"dev.dscc.profile.v0","id":"bad-road","name":"Bad Road"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_schema.status(), StatusCode::BAD_REQUEST);

    let imported: ProfileSummary =
        get_json(router, "/api/profiles/imported-road", StatusCode::OK).await;
    assert_eq!(imported.name, "Imported Road");
    assert!(!imported.built_in);
}

#[tokio::test]
async fn imported_profile_config_is_normalized_before_storage() {
    let state = AgentState::mock();
    let router = app(state.clone());

    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/profiles/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    r##"{
                        "schema":"dev.dscc.profile.v1",
                        "id":"imported-hot",
                        "name":"Imported Hot",
                        "config":{
                            "lightbar":{"enabled":true,"color":"#ff0000","brightness":250},
                            "trigger":{
                                "sameRange":false,
                                "l2From":10,
                                "l2To":90,
                                "r2From":0,
                                "r2To":100,
                                "effect":"Wall",
                                "intensity":"Medium",
                                "vibration":"Medium"
                            },
                            "sticks":{
                                "leftCurve":"Default",
                                "leftCurveAmount":50,
                                "leftDeadzone":5,
                                "rightCurve":"Default",
                                "rightCurveAmount":110,
                                "rightDeadzone":42
                            }
                        }
                    }"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let inner = state.inner.read().await;
    let stored = inner
        .profile_configs
        .get("imported-hot")
        .expect("imported config is stored");
    assert_eq!(
        stored.lightbar.brightness, 100,
        "out-of-range brightness must be clamped at import"
    );
    assert_eq!(stored.sticks.right_curve_amount, 100);
    assert_eq!(stored.sticks.right_deadzone, 40);
}

#[tokio::test]
async fn modules_and_profile_resolution_are_api_visible() {
    let router = app(AgentState::mock());

    let modules: Vec<ModuleSummary> =
        get_json(router.clone(), "/api/modules", StatusCode::OK).await;
    assert!(modules
        .iter()
        .any(|module| module.id == "forza-data-out" && module.trusted));

    let resolution: ProfileResolutionResponse =
        get_json(router.clone(), "/api/profile-resolution", StatusCode::OK).await;
    // Mock state has no active telemetry adapter (synthetic-lab removed
    // for production), so resolution falls through to the global default.
    assert_eq!(resolution.reason, "global_default");

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/profile-resolution/override")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"controllerId":null,"gameId":null,"profileId":"global"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let resolution: ProfileResolutionResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(resolution.reason, "manual_override");
    assert_eq!(
        resolution.override_profile_id.as_deref(),
        Some(DEFAULT_PROFILE_ID)
    );
}

#[tokio::test]
async fn profile_override_delete_can_clear_one_game_scope() {
    let state = AgentState::mock();
    let router = app(state.clone());

    for body in [
        r#"{"controllerId":null,"gameId":null,"profileId":"forza-horizon"}"#,
        r#"{"controllerId":null,"gameId":"forza-horizon-6","profileId":"forza-horizon"}"#,
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/profile-resolution/override")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/profile-resolution/override?gameId=forza-horizon-6")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let inner = state.inner.read().await;
    assert!(!inner
        .profile_overrides
        .contains_key(&profile_override_key(None, Some("forza-horizon-6"))));
    assert!(inner
        .profile_overrides
        .contains_key(&profile_override_key(None, None)));
}

#[tokio::test]
async fn controller_global_profile_override_resolves_for_selected_controller() {
    let router = app(AgentState::from_controller_events([attach_event(
        "edge-global",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/profile-resolution/override")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"controllerId":"edge-global","gameId":null,"profileId":"forza-horizon-immersive"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let resolution: ProfileResolutionResponse =
        get_json(router, "/api/profile-resolution", StatusCode::OK).await;
    assert_eq!(resolution.reason, "manual_override");
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
}
