use super::support::*;
use super::*;
use crate::edge_profiles::{
    edge_profile_config_from_hardware, edge_profile_from_slot_config,
    edge_slot_config_keeps_custom_curve,
};

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

fn custom_curve_hardware_profile(slot: EdgeOnboardSlotId) -> EdgeOnboardProfile {
    let mut profile = EdgeOnboardProfile::new(slot, "Onboard Custom");
    profile.left_stick = EdgeStickProfile {
        preset: EdgeStickPreset::Custom,
        curve_points: [3, 9, 60, 90, 140, 180, 210, 250],
    };
    profile.right_stick = EdgeStickProfile {
        preset: EdgeStickPreset::Custom,
        curve_points: [1, 7, 55, 85, 150, 190, 220, 240],
    };
    profile
}

#[test]
fn custom_stick_curve_round_trips_through_slot_config_save_path() {
    let previous = custom_curve_hardware_profile(EdgeOnboardSlotId::Square);

    let config = edge_profile_config_from_hardware(&previous);
    assert_eq!(config.sticks.left_curve, "Custom");
    assert_eq!(config.sticks.right_curve, "Custom");
    assert!(edge_slot_config_keeps_custom_curve(&config));

    let rebuilt =
        edge_profile_from_slot_config(EdgeOnboardSlotId::Square, &config, Some(&previous));
    assert_eq!(rebuilt.left_stick, previous.left_stick);
    assert_eq!(rebuilt.right_stick, previous.right_stick);

    let encoded = dscc_device::encode_edge_onboard_profile(&rebuilt).unwrap();
    assert_eq!(encoded[2][30], 0xff);
    assert_eq!(encoded[2][32], 0xff);
}

#[test]
fn named_preset_labels_still_map_without_previous_curve_leakage() {
    let previous = custom_curve_hardware_profile(EdgeOnboardSlotId::Cross);
    let mut config = edge_profile_config_from_hardware(&previous);
    config.sticks.left_curve = "Quick".to_string();
    config.sticks.right_curve = "Steady".to_string();
    assert!(!edge_slot_config_keeps_custom_curve(&config));

    let rebuilt = edge_profile_from_slot_config(EdgeOnboardSlotId::Cross, &config, Some(&previous));
    assert_eq!(
        rebuilt.left_stick,
        EdgeStickProfile {
            preset: EdgeStickPreset::Quick,
            ..EdgeStickProfile::default()
        }
    );
    assert_eq!(
        rebuilt.right_stick,
        EdgeStickProfile {
            preset: EdgeStickPreset::Steady,
            ..EdgeStickProfile::default()
        }
    );
}

#[test]
fn custom_label_without_previous_read_keeps_custom_preset() {
    let previous = custom_curve_hardware_profile(EdgeOnboardSlotId::Circle);
    let config = edge_profile_config_from_hardware(&previous);

    let rebuilt = edge_profile_from_slot_config(EdgeOnboardSlotId::Circle, &config, None);
    assert_eq!(rebuilt.left_stick.preset, EdgeStickPreset::Custom);
    assert_eq!(rebuilt.right_stick.preset, EdgeStickPreset::Custom);
    assert_eq!(
        rebuilt.left_stick.curve_points,
        EdgeStickProfile::default().curve_points
    );
}
