use super::*;

pub(crate) async fn list_controllers(
    State(state): State<AgentState>,
) -> Json<Vec<ControllerSummary>> {
    let (controllers, configs) = {
        let inner = state.inner.read().await;
        (
            apply_controller_names(inner.controllers.summaries(), &inner.controller_names),
            inner.controller_configs.clone(),
        )
    };
    let diagnostics = state.output_diagnostics_snapshot();
    Json(state.apply_power_diagnostics_to_controllers(controllers, &diagnostics, &configs))
}

pub(crate) async fn get_controller(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerDetail>, StatusCode> {
    let (detail, configs) = {
        let inner = state.inner.read().await;
        (
            inner
                .controllers
                .detail(&id)
                .map(|detail| apply_controller_name(detail, &inner.controller_names))
                .ok_or(StatusCode::NOT_FOUND)?,
            inner.controller_configs.clone(),
        )
    };
    let diagnostics = state.output_diagnostics_snapshot();
    Ok(Json(state.apply_power_diagnostics_to_controller_detail(
        detail,
        &diagnostics,
        &configs,
    )))
}

pub(crate) async fn update_controller(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateControllerRequest>,
) -> Result<Json<ControllerDetail>, StatusCode> {
    let name = normalize_controller_display_name(&request.name).ok_or(StatusCode::BAD_REQUEST)?;
    let (detail, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        inner.controller_names.insert(id.clone(), name.clone());
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Controller {id} renamed to {name}"),
            timestamp: current_timestamp(),
        });
        (
            apply_controller_name(detail, &inner.controller_names),
            build_persist_snapshot(&inner),
        )
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("controller-renamed".to_string()),
    });
    Ok(Json(detail))
}

pub(crate) async fn get_controller_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerConfig>, StatusCode> {
    // Reads compute the effective config without inserting or persisting;
    // configs are created and saved on PUT.
    let config = {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        match inner.controller_configs.get(&id) {
            Some(config) => config.clone(),
            None => {
                let mut config = ControllerConfig::default_for(&id, detail.model);
                if let Some(profile_config) = inner
                    .active_profile_id
                    .as_deref()
                    .and_then(|profile_id| inner.profile_configs.get(profile_id))
                {
                    profile_config.apply_to_controller_config(&mut config);
                }
                config
            }
        }
        .normalized()
    };
    Ok(Json(config))
}

pub(crate) async fn update_controller_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateControllerConfigRequest>,
) -> Result<Json<ControllerConfig>, StatusCode> {
    let (config, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let existing_input_bridge = inner
            .controller_configs
            .get(&id)
            .map(|config| config.input_bridge.clone());
        let config =
            ControllerConfig::from_update(id.clone(), detail.model, request, existing_input_bridge);
        inner.controller_configs.insert(id.clone(), config.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Configuration saved for controller {id}"),
            timestamp: current_timestamp(),
        });
        (config, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(config))
}

pub(crate) async fn get_controller_input(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerInputResponse>, StatusCode> {
    Ok(Json(read_controller_input_state(id, state).await?))
}

pub(crate) async fn get_current_controller_input(
    State(state): State<AgentState>,
) -> Result<Json<ControllerInputResponse>, StatusCode> {
    let id = {
        let inner = state.inner.read().await;
        inner
            .controllers
            .summaries()
            .into_iter()
            .find(|controller| controller.connected)
            .map(|controller| controller.id)
            .ok_or(StatusCode::NOT_FOUND)?
    };

    Ok(Json(read_controller_input_state(id, state).await?))
}

pub(crate) async fn read_controller_input_state(
    id: String,
    state: AgentState,
) -> Result<ControllerInputResponse, StatusCode> {
    {
        let inner = state.inner.read().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
    }

    if state.input_bridge.is_active(&id) {
        return match state.cached_input_state(&id, INPUT_BRIDGE_STALE_AFTER) {
            Some(sample) => Ok(controller_input_available(id, sample)),
            None => Ok(controller_input_unavailable(
                id,
                "hid",
                "Waiting for a fresh DSCC Input Bridge input sample".to_string(),
            )),
        };
    }

    match state
        .read_cached_or_live_input_state_for_controller(
            &id,
            ControllerInputReadOptions::bridge_poll(),
            CONTROLLER_INPUT_UI_CACHE_TTL,
        )
        .await
    {
        Ok(Some(input)) => Ok(controller_input_available(id, input)),
        Ok(None) => Ok(controller_input_unavailable(
            id,
            "hid",
            "No fresh DualSense input report was available".to_string(),
        )),
        Err(error) => Ok(controller_input_unavailable(
            id,
            "hid",
            format!("DualSense input read failed: {error}"),
        )),
    }
}

fn controller_input_available(
    controller_id: String,
    sample: LatestControllerInput,
) -> ControllerInputResponse {
    let age_ms = input_sample_age_ms(&sample);
    let input = sample.state;
    ControllerInputResponse {
        controller_id,
        available: true,
        source: "hid".to_string(),
        message: "Live DualSense input is available".to_string(),
        sampled_at_ms: Some(sample.sampled_at_ms),
        age_ms: Some(age_ms),
        axes: ControllerInputAxesResponse {
            left_stick: ControllerInputStickResponse {
                x: input.left_stick.x,
                y: input.left_stick.y,
                magnitude: input.left_stick.magnitude,
            },
            right_stick: ControllerInputStickResponse {
                x: input.right_stick.x,
                y: input.right_stick.y,
                magnitude: input.right_stick.magnitude,
            },
        },
        triggers: ControllerInputTriggersResponse {
            l2: input.l2,
            r2: input.r2,
        },
        buttons: input
            .buttons
            .into_iter()
            .map(|button| ControllerInputButtonResponse {
                id: button.id.to_string(),
                label: button.label.to_string(),
                pressed: button.pressed,
                value: button.value,
            })
            .collect(),
    }
}

fn input_sample_age_ms(sample: &LatestControllerInput) -> u64 {
    sample
        .sampled_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn controller_input_unavailable(
    controller_id: String,
    source: &str,
    message: String,
) -> ControllerInputResponse {
    ControllerInputResponse {
        controller_id,
        available: false,
        source: source.to_string(),
        message,
        sampled_at_ms: None,
        age_ms: None,
        axes: ControllerInputAxesResponse {
            left_stick: ControllerInputStickResponse::default(),
            right_stick: ControllerInputStickResponse::default(),
        },
        triggers: ControllerInputTriggersResponse::default(),
        buttons: Vec::new(),
    }
}
