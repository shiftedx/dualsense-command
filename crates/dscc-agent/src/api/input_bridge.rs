use super::*;

pub(crate) async fn get_input_bridge_status(
    State(state): State<AgentState>,
) -> Json<InputBridgeStatusResponse> {
    Json(state.input_bridge.status_response())
}

pub(crate) async fn get_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Json<InputBridgeSessionSummary> {
    Json(state.input_bridge.session_summary(&controller_id))
}

pub(crate) async fn start_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<InputBridgeSessionSummary>, (StatusCode, Json<serde_json::Value>)> {
    {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&controller_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "controller not found"})),
            )
        })?;
        if !detail.connected {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "controller is not connected"})),
            ));
        }
        let config = inner
            .controller_configs
            .get(&controller_id)
            .cloned()
            .unwrap_or_else(|| ControllerConfig::default_for(&controller_id, detail.model));
        if config.input_mode != ControllerInputMode::DsccInputBridge || !config.input_bridge.enabled
        {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "DSCC Input Bridge must be explicitly enabled for this controller"
                })),
            ));
        }
    }
    let detection = state
        .cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
        .await;
    if !detection_allows_input_bridge(&detection) {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "DSCC Input Bridge can only start while a local app is active"
            })),
        ));
    }
    if !local_app_execution_verified_for_input_bridge(&state, &detection).await {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "DSCC Input Bridge can only start while the registered local app executable is running"
            })),
        ));
    }
    let existing = state.input_bridge.session_summary(&controller_id);
    if existing.state == InputBridgeSessionState::Active {
        return Ok(Json(existing));
    }
    let summary = state
        .input_bridge
        .start_session(
            &controller_id,
            VirtualOutputKind::Xbox360,
            current_timestamp_millis(),
        )
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": error})),
            )
        })?;
    let loop_state = state.clone();
    let loop_controller_id = controller_id.clone();
    tokio::spawn(async move {
        run_input_bridge_session_loop(loop_state, loop_controller_id).await;
    });
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("input-bridge-started".to_string()),
    });
    Ok(Json(summary))
}

pub(crate) async fn stop_input_bridge_session(
    Path(controller_id): Path<String>,
    State(state): State<AgentState>,
) -> Json<InputBridgeSessionSummary> {
    let summary = state
        .input_bridge
        .stop_session(&controller_id, current_timestamp_millis());
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("input-bridge-stopped".to_string()),
    });
    Json(summary)
}

pub(crate) async fn run_input_bridge_session_loop(state: AgentState, controller_id: String) {
    let mut last_input_at = Instant::now();
    let mut last_submitted_sequence: Option<u64> = None;
    let mut last_game_check_at = Instant::now();
    let mut last_config_check_at: Option<Instant> = None;
    let mut active_config: Option<InputBridgeConfig> = None;
    let mut process_interval = tokio::time::interval(INPUT_BRIDGE_PROCESS_INTERVAL);
    process_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        process_interval.tick().await;
        if !state.input_bridge.is_active(&controller_id) {
            break;
        }

        if last_config_check_at
            .map(|checked_at| checked_at.elapsed() >= INPUT_BRIDGE_CONFIG_REFRESH_INTERVAL)
            .unwrap_or(true)
        {
            let inner = state.inner.read().await;
            let Some(detail) = inner.controllers.detail(&controller_id) else {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-controller-disconnected");
                break;
            };
            if !detail.connected {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-controller-disconnected");
                break;
            }
            let config = inner
                .controller_configs
                .get(&controller_id)
                .cloned()
                .unwrap_or_else(|| ControllerConfig::default_for(&controller_id, detail.model));
            if config.input_mode != ControllerInputMode::DsccInputBridge
                || !config.input_bridge.enabled
            {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-config-disabled");
                break;
            }
            active_config = Some(config.input_bridge);
            last_config_check_at = Some(Instant::now());
        }
        let Some(config) = active_config.as_ref() else {
            continue;
        };

        if last_game_check_at.elapsed() >= HARDWARE_GAME_DETECTION_INTERVAL {
            let detection = state
                .cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
                .await;
            if !detection_allows_input_bridge(&detection) {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-local-app-inactive");
                break;
            }
            if !local_app_execution_verified_for_input_bridge(&state, &detection).await {
                state
                    .input_bridge
                    .stop_session(&controller_id, current_timestamp_millis());
                send_input_bridge_invalidation(&state, "input-bridge-local-app-unverified");
                break;
            }
            last_game_check_at = Instant::now();
        }

        match state
            .read_cached_or_live_input_state_for_controller(
                &controller_id,
                ControllerInputReadOptions::bridge_poll(),
                INPUT_BRIDGE_PROCESS_INTERVAL,
            )
            .await
        {
            Ok(Some(sample)) => {
                last_input_at = sample.sampled_at;
                if last_submitted_sequence == Some(sample.sequence) {
                    continue;
                }
                last_submitted_sequence = Some(sample.sequence);
                if state
                    .input_bridge
                    .submit_controller_input(
                        &controller_id,
                        &sample.state,
                        config,
                        current_timestamp_millis(),
                    )
                    .is_err()
                {
                    state.input_bridge.neutralize_session(
                        &controller_id,
                        InputBridgeSessionState::Faulted,
                        "DSCC Input Bridge backend fault; virtual output was neutralized.",
                        current_timestamp_millis(),
                    );
                    tracing::warn!(controller_id = %controller_id, "DSCC Input Bridge backend fault");
                    send_input_bridge_invalidation(&state, "input-bridge-backend-fault");
                    break;
                }
            }
            Ok(None) => {
                if last_input_at.elapsed() >= INPUT_BRIDGE_STALE_AFTER {
                    state.input_bridge.neutralize_session(
                        &controller_id,
                        InputBridgeSessionState::Stale,
                        "DSCC Input Bridge neutralized output after stale controller input.",
                        current_timestamp_millis(),
                    );
                    send_input_bridge_invalidation(&state, "input-bridge-input-stale");
                    last_input_at = Instant::now();
                }
            }
            Err(_) => {
                state.input_bridge.neutralize_session(
                    &controller_id,
                    InputBridgeSessionState::Faulted,
                    "DSCC Input Bridge input read failed; virtual output was neutralized.",
                    current_timestamp_millis(),
                );
                tracing::warn!(controller_id = %controller_id, "DSCC Input Bridge input read failed");
                send_input_bridge_invalidation(&state, "input-bridge-input-fault");
                break;
            }
        }
    }
}

fn send_input_bridge_invalidation(state: &AgentState, message: &str) {
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some(message.to_string()),
    });
}

pub(crate) async fn write_input_bridge_binding(
    State(state): State<AgentState>,
    Json(request): Json<InputBridgeBindingWriteRequest>,
) -> Result<Json<InputBridgeBindingWriteResponse>, (StatusCode, Json<serde_json::Value>)> {
    let input_id = request.input_id.trim();
    let target = request.target.trim();
    if input_id.is_empty() || target.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "inputId and target are required"})),
        ));
    }
    let source = bridge_source_from_input_id(input_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unsupported DSCC Input Bridge source"})),
        )
    })?;
    let target = bridge_target_from_raw(target).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Unsupported DSCC Input Bridge target"})),
        )
    })?;
    let binding = InputBridgeBindingConfig { source, target };
    let mut warnings = Vec::new();
    if !request.dry_run {
        let to_save = {
            let mut inner = state.inner.write().await;
            let profile_id = request.profile_id.as_deref().map(str::trim).unwrap_or("");
            let wrote_profile = if !profile_id.is_empty()
                && inner
                    .profiles
                    .iter()
                    .any(|profile| profile.id == profile_id && !profile.built_in)
            {
                let config = inner
                    .profile_configs
                    .entry(profile_id.to_string())
                    .or_insert_with(ProfileConfig::default);
                upsert_input_bridge_binding(&mut config.input_bridge, binding.clone());
                true
            } else {
                false
            };

            if !wrote_profile {
                let controller_id = request
                    .controller_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("");
                if controller_id.is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "controllerId is required when no writable profileId is provided"
                        })),
                    ));
                }
                let model = inner
                    .controllers
                    .detail(controller_id)
                    .map(|detail| detail.model)
                    .ok_or_else(|| {
                        (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"error": "controller not found"})),
                        )
                    })?;
                let config = inner
                    .controller_configs
                    .entry(controller_id.to_string())
                    .or_insert_with(|| ControllerConfig::default_for(controller_id, model));
                upsert_input_bridge_binding(&mut config.input_bridge, binding.clone());
                warnings.push(
                    "Wrote bridge binding to controller config because the profile is built-in or absent."
                        .to_string(),
                );
            }
            inner.effect_revision = inner.effect_revision.saturating_add(1);
            build_persist_snapshot(&inner)
        };
        persist_snapshot(&state, to_save).await;
        let _ = state.event_tx.send(RealtimeMessage {
            kind: "snapshot_invalidated".to_string(),
            controller: None,
            message: Some("input-bridge-binding-updated".to_string()),
        });
    }
    Ok(Json(InputBridgeBindingWriteResponse {
        accepted: true,
        message: if request.dry_run {
            format!("Validated DSCC Input Bridge binding for {input_id}.")
        } else {
            format!("Saved DSCC Input Bridge binding for {input_id}.")
        },
        dry_run: request.dry_run,
        warnings,
    }))
}

fn upsert_input_bridge_binding(config: &mut InputBridgeConfig, binding: InputBridgeBindingConfig) {
    config
        .bindings
        .retain(|existing| existing.source != binding.source);
    config.bindings.push(binding);
    *config = config.clone().normalized();
}

fn bridge_source_from_input_id(input_id: &str) -> Option<InputBridgeSource> {
    let normalized = input_id.trim().to_ascii_lowercase();
    let source = match normalized.as_str() {
        "button_a" => InputBridgeSource::Button("cross".to_string()),
        "button_b" => InputBridgeSource::Button("circle".to_string()),
        "button_x" => InputBridgeSource::Button("square".to_string()),
        "button_y" => InputBridgeSource::Button("triangle".to_string()),
        "dpad_north" | "dpad_up" => InputBridgeSource::Button("dpad_up".to_string()),
        "dpad_south" | "dpad_down" => InputBridgeSource::Button("dpad_down".to_string()),
        "dpad_west" | "dpad_left" => InputBridgeSource::Button("dpad_left".to_string()),
        "dpad_east" | "dpad_right" => InputBridgeSource::Button("dpad_right".to_string()),
        "left_bumper" | "button_should_left" => InputBridgeSource::Button("l1".to_string()),
        "right_bumper" | "button_should_right" => InputBridgeSource::Button("r1".to_string()),
        "click:left_trigger" | "left_trigger:click" => InputBridgeSource::Axis("l2".to_string()),
        "click:right_trigger" | "right_trigger:click" => InputBridgeSource::Axis("r2".to_string()),
        "button_menu" => InputBridgeSource::Button("create".to_string()),
        "button_escape" => InputBridgeSource::Button("options".to_string()),
        "click:left_joystick" | "left_joystick:click" | "click:joystick" | "joystick:click" => {
            InputBridgeSource::Button("l3".to_string())
        }
        "click:right_joystick" | "right_joystick:click" => {
            InputBridgeSource::Button("r3".to_string())
        }
        "click:left_trackpad" | "left_trackpad:click" => {
            InputBridgeSource::Button("touchpad".to_string())
        }
        "click:right_trackpad" | "right_trackpad:click" => {
            InputBridgeSource::Button("touchpad".to_string())
        }
        "button_back_left" => InputBridgeSource::Button("edge_back_left".to_string()),
        "button_back_right" => InputBridgeSource::Button("edge_back_right".to_string()),
        "button_back_left_upper" => InputBridgeSource::Button("edge_fn_left".to_string()),
        "button_back_right_upper" => InputBridgeSource::Button("edge_fn_right".to_string()),
        _ if normalized.contains("center_trackpad") || normalized.contains("gyro") => return None,
        _ => return None,
    };
    Some(source)
}

fn bridge_target_from_raw(raw: &str) -> Option<InputBridgeTarget> {
    let command = raw.split(',').next()?.trim();
    let mut parts = command.split_whitespace();
    let kind = parts.next()?.to_ascii_lowercase();
    let param = parts.next().unwrap_or("").to_ascii_lowercase();
    if kind != "xinput_button" {
        return None;
    }
    match param.as_str() {
        "a" => Some(InputBridgeTarget::Button(VirtualButton::A)),
        "b" => Some(InputBridgeTarget::Button(VirtualButton::B)),
        "x" => Some(InputBridgeTarget::Button(VirtualButton::X)),
        "y" => Some(InputBridgeTarget::Button(VirtualButton::Y)),
        "dpad_up" => Some(InputBridgeTarget::Button(VirtualButton::DpadUp)),
        "dpad_down" => Some(InputBridgeTarget::Button(VirtualButton::DpadDown)),
        "dpad_left" => Some(InputBridgeTarget::Button(VirtualButton::DpadLeft)),
        "dpad_right" => Some(InputBridgeTarget::Button(VirtualButton::DpadRight)),
        "shoulder_left" => Some(InputBridgeTarget::Button(VirtualButton::LeftShoulder)),
        "shoulder_right" => Some(InputBridgeTarget::Button(VirtualButton::RightShoulder)),
        "trigger_left" => Some(InputBridgeTarget::Axis(VirtualAxis::LeftTrigger)),
        "trigger_right" => Some(InputBridgeTarget::Axis(VirtualAxis::RightTrigger)),
        "joystick_left" => Some(InputBridgeTarget::Button(VirtualButton::LeftThumb)),
        "joystick_right" => Some(InputBridgeTarget::Button(VirtualButton::RightThumb)),
        "select" | "back" => Some(InputBridgeTarget::Button(VirtualButton::Back)),
        "start" => Some(InputBridgeTarget::Button(VirtualButton::Start)),
        "guide" => Some(InputBridgeTarget::Button(VirtualButton::Guide)),
        _ => None,
    }
}
