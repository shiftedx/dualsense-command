use super::support::*;
use super::*;

#[tokio::test]
async fn edge_onboard_profiles_are_visible_and_stageable() {
    let router = app(AgentState::from_controller_events([attach_event(
        "edge-onboard",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        None,
    )]));

    let profiles: EdgeProfilesResponse = get_json(
        router.clone(),
        "/api/controllers/edge-onboard/edge-profiles",
        StatusCode::OK,
    )
    .await;
    assert_eq!(profiles.support_state, EdgeProfileSupportState::Unknown);
    assert_eq!(profiles.slots.len(), 4);
    assert!(profiles
        .slots
        .iter()
        .any(|slot| slot.slot_id == "circle" && slot.editable));

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/controllers/edge-onboard/edge-profiles/circle")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "name":"Track Focus",
                        "trigger":{
                            "sameRange":false,
                            "l2From":5,
                            "l2To":95,
                            "r2From":0,
                            "r2To":100,
                            "effect":"Adaptive resistance",
                            "intensity":"Medium",
                            "vibration":"Medium"
                        },
                        "sticks":{
                            "leftCurve":"Quick",
                            "leftCurveAmount":55,
                            "leftDeadzone":4,
                            "rightCurve":"Default",
                            "rightCurveAmount":60,
                            "rightDeadzone":8
                        },
                        "buttons":[{"key":"Back Left","label":"Shift down"}]
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let accepted: ActionAccepted = serde_json::from_slice(&body).unwrap();
    assert!(accepted.accepted);

    let profiles: EdgeProfilesResponse = get_json(
        router,
        "/api/controllers/edge-onboard/edge-profiles",
        StatusCode::OK,
    )
    .await;
    let circle = profiles
        .slots
        .iter()
        .find(|slot| slot.slot_id == "circle")
        .expect("circle slot exists");
    assert_eq!(circle.state, EdgeProfileSlotState::Assigned);
    assert_eq!(circle.name.as_deref(), Some("Track Focus"));
    assert!(!circle.hardware_synced);
}
