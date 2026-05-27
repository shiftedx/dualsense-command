use super::*;

pub(crate) async fn get_status(State(state): State<AgentState>) -> Json<StatusResponse> {
    let game_detection = state.cached_game_detection().await;
    Json(state.status_with_detection(Some(&game_detection)).await)
}

pub(crate) async fn get_update_check(State(state): State<AgentState>) -> Json<UpdateCheckResponse> {
    Json(state.update_check().await)
}

pub(crate) async fn get_app_settings(State(state): State<AgentState>) -> Json<AppSettingsResponse> {
    let inner = state.inner.read().await;
    Json(state.app_settings_response(&inner.app_settings))
}

pub(crate) async fn update_app_settings(
    State(state): State<AgentState>,
    Json(request): Json<UpdateAppSettingsRequest>,
) -> Result<Json<AppSettingsResponse>, (StatusCode, String)> {
    if request.listen_on_all_interfaces == Some(true) && !lan_api_enabled() {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "LAN API access requires explicit opt-in. Set {LAN_API_ENABLE_ENV}=1 before enabling all-interface binding."
            ),
        ));
    }

    let glyph_result = if let Some(glyphs) = request.forza_playstation_glyphs.clone() {
        let persisted_install_path = {
            let inner = state.inner.read().await;
            inner
                .app_settings
                .forza_playstation_glyphs
                .install_path
                .clone()
        };
        let configured_path = glyphs
            .install_path
            .as_deref()
            .or(persisted_install_path.as_deref())
            .map(|path| resolve_forza_horizon6_install_path(Some(path)));
        let steam_path = supported_game_install_path(
            &state.cached_steam_game_catalog().await,
            "forza-horizon-6",
        );
        let install_path = trusted_forza_horizon6_install_path(configured_path, steam_path);
        let requested_enabled = glyphs.enabled;
        let path_for_task = install_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            if requested_enabled {
                install_forza_playstation_glyphs(path_for_task)
            } else {
                restore_forza_original_glyphs(path_for_task)
            }
        })
        .await
        .map_err(|error| format!("glyph installer task failed: {error}"))
        .and_then(|result| result.map_err(|error| error.to_string()));
        Some((requested_enabled, install_path, result))
    } else {
        None
    };

    let (response, to_save) = {
        let mut inner = state.inner.write().await;
        let mut settings = inner.app_settings.clone();
        if let Some(listen) = request.listen_on_all_interfaces {
            settings.listen_on_all_interfaces = listen;
        }
        if let Some((requested_enabled, install_path, result)) = glyph_result {
            settings.forza_playstation_glyphs.install_path =
                Some(install_path.display().to_string());
            match result {
                Ok(message) => {
                    settings.forza_playstation_glyphs.enabled = requested_enabled;
                    settings.forza_playstation_glyphs.last_status = if requested_enabled {
                        "installed".to_string()
                    } else {
                        "restored".to_string()
                    };
                    settings.forza_playstation_glyphs.last_message = message;
                }
                Err(message) => {
                    settings.forza_playstation_glyphs.last_status = "error".to_string();
                    settings.forza_playstation_glyphs.last_message = message;
                }
            }
        }
        inner.app_settings = settings.clone();
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: "Application settings updated".to_string(),
            timestamp: current_timestamp(),
        });
        (
            state.app_settings_response(&settings),
            build_persist_snapshot(&inner),
        )
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("app-settings-updated".to_string()),
    });
    Ok(Json(response))
}

pub(crate) async fn get_snapshot(State(state): State<AgentState>) -> Json<AgentSnapshotResponse> {
    Json(state.snapshot().await)
}

pub(crate) async fn list_controllers(
    State(state): State<AgentState>,
) -> Json<Vec<ControllerSummary>> {
    let inner = state.inner.read().await;
    Json(apply_controller_names(
        inner.controllers.summaries(),
        &inner.controller_names,
    ))
}

pub(crate) async fn get_controller(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ControllerDetail>, StatusCode> {
    let inner = state.inner.read().await;
    inner
        .controllers
        .detail(&id)
        .map(|detail| apply_controller_name(detail, &inner.controller_names))
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
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
    let (config, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let active_profile_config = inner
            .active_profile_id
            .as_deref()
            .and_then(|profile_id| inner.profile_configs.get(profile_id))
            .cloned();
        let model = detail.model;
        let config = inner
            .controller_configs
            .entry(id.clone())
            .or_insert_with(|| {
                let mut config = ControllerConfig::default_for(id, model);
                if let Some(profile_config) = active_profile_config.as_ref() {
                    profile_config.apply_to_controller_config(&mut config);
                }
                config
            })
            .clone()
            .normalized();
        inner
            .controller_configs
            .insert(config.controller_id.clone(), config.clone());
        (config, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
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

pub(crate) async fn test_effect(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    run_effect_test_for_controller(id, state, request).await
}

pub(crate) async fn test_current_effect(
    State(state): State<AgentState>,
    Json(request): Json<EffectTestRequest>,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
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

    run_effect_test_for_controller(id, state, request).await
}

pub(crate) async fn run_effect_test_for_controller(
    id: String,
    state: AgentState,
    request: EffectTestRequest,
) -> Result<(StatusCode, Json<EffectTestResponse>), StatusCode> {
    {
        let inner = state.inner.read().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;

        if detail.permission == ControllerPermissionState::Denied {
            return Ok((
                StatusCode::CONFLICT,
                Json(EffectTestResponse {
                    accepted: false,
                    message: format!(
                        "Controller {id} requires device permission before effect tests"
                    ),
                    dry_run: true,
                    duration_ms: 0,
                    output: ControllerOutputFrame::default(),
                }),
            ));
        }
    }

    let target = request.target.as_deref().unwrap_or("r2").to_string();
    let mode = request
        .mode
        .as_deref()
        .unwrap_or("adaptive_resistance")
        .to_string();
    let stop_manual_override = target == "base_feel" && mode == "off";
    let duration_ms = if stop_manual_override {
        0
    } else if target == "base_feel" {
        request
            .duration_ms
            .unwrap_or(DEFAULT_BASE_FEEL_TEST_DURATION_MS)
            .clamp(500, MAX_BASE_FEEL_TEST_DURATION_MS)
    } else {
        request
            .duration_ms
            .unwrap_or(DEFAULT_EFFECT_TEST_DURATION_MS)
            .clamp(100, MAX_EFFECT_TEST_DURATION_MS)
    };
    let output = if stop_manual_override {
        ControllerOutputFrame::default()
    } else {
        effect_test_output_frame(&request)
    };
    let base_feel_trigger = if target == "base_feel" && !stop_manual_override {
        Some(request.trigger.clone().unwrap_or_default())
    } else {
        None
    };
    let hardware_output_enabled = state.hardware_output_enabled();
    let mut accepted = true;
    let mut status = StatusCode::ACCEPTED;
    let mut message = if hardware_output_enabled {
        if stop_manual_override {
            state.clear_manual_output_override();
        }
        let generation = if stop_manual_override {
            None
        } else {
            Some(state.begin_manual_output_override(Duration::from_millis(duration_ms)))
        };
        match state.write_output_frame_to_controller(&id, &output).await {
            Ok(write) => {
                if let Some(generation) = generation {
                    let state_for_reset = state.clone();
                    let id_for_reset = id.clone();
                    let output_for_refresh = output.clone();
                    let base_feel_trigger = base_feel_trigger.clone();
                    tokio::spawn(async move {
                        let deadline = Instant::now() + Duration::from_millis(duration_ms);
                        let refresh_interval = if base_feel_trigger.is_some() {
                            BASE_FEEL_OUTPUT_REFRESH_INTERVAL
                        } else {
                            MANUAL_OUTPUT_REFRESH_INTERVAL
                        };
                        loop {
                            let now = Instant::now();
                            if now >= deadline {
                                break;
                            }
                            let sleep_for =
                                refresh_interval.min(deadline.saturating_duration_since(now));
                            tokio::time::sleep(sleep_for).await;
                            if !state_for_reset.manual_output_override_active_for(generation) {
                                if !state_for_reset
                                    .manual_output_override_generation_matches(generation)
                                {
                                    return;
                                }
                                break;
                            }
                            if Instant::now() >= deadline {
                                break;
                            }
                            let output_for_refresh = if let Some(trigger_config) =
                                base_feel_trigger.as_ref()
                            {
                                match state_for_reset
                                    .read_input_state_for_controller(&id_for_reset)
                                    .await
                                {
                                    Ok(Some(input)) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        Some(input.l2),
                                        Some(input.r2),
                                    ),
                                    Ok(None) => base_feel_test_output_frame(
                                        trigger_config.clone(),
                                        None,
                                        None,
                                    ),
                                    Err(error) => {
                                        state_for_reset
                                                .note_hardware_output_error(format!(
                                                    "Hardware effect test input read for controller {id_for_reset} failed: {error}"
                                                ))
                                                .await;
                                        output_for_refresh.clone()
                                    }
                                }
                            } else {
                                output_for_refresh.clone()
                            };
                            if let Err(error) = state_for_reset
                                .write_output_frame_to_controller(
                                    &id_for_reset,
                                    &output_for_refresh,
                                )
                                .await
                            {
                                state_for_reset
                                    .note_hardware_output_error(format!(
                                        "Hardware effect test refresh for controller {id_for_reset} failed: {error}"
                                    ))
                                    .await;
                                break;
                            }
                        }

                        if state_for_reset.manual_output_override_generation_matches(generation) {
                            let _ = state_for_reset
                                .write_output_frame_to_controller(
                                    &id_for_reset,
                                    &ControllerOutputFrame::default(),
                                )
                                .await;
                            state_for_reset
                                .release_output_session_for_controller(&id_for_reset)
                                .await;
                            state_for_reset.clear_manual_output_override_if_generation(generation);
                        }
                    });
                    format!(
                        "Queued hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                } else {
                    state.release_output_session_for_controller(&id).await;
                    format!(
                        "Stopped hardware effect test for controller {id} ({} byte {:?} report)",
                        write.bytes, write.report_kind
                    )
                }
            }
            Err(error) => {
                if !stop_manual_override {
                    state.clear_manual_output_override();
                } else {
                    state.release_output_session_for_controller(&id).await;
                }
                accepted = false;
                status = StatusCode::CONFLICT;
                format!("Hardware effect test for controller {id} was blocked: {error}")
            }
        }
    } else {
        format!("Queued effect test preview for controller {id}")
    };

    {
        let mut inner = state.inner.write().await;
        inner.logs.push(LogEntry {
            level: if accepted { "info" } else { "warn" }.to_string(),
            message: format!("{}: target={} mode={}", message, target, mode),
            timestamp: current_timestamp(),
        });
    }

    if !accepted && message.is_empty() {
        message = format!("Hardware effect test for controller {id} was blocked");
    }

    Ok((
        status,
        Json(EffectTestResponse {
            accepted,
            message,
            dry_run: !hardware_output_enabled,
            duration_ms,
            output,
        }),
    ))
}

pub(crate) async fn list_profiles(State(state): State<AgentState>) -> Json<Vec<ProfileSummary>> {
    let inner = state.inner.read().await;
    Json(inner.profiles.clone())
}

pub(crate) async fn create_profile(
    State(state): State<AgentState>,
    Json(request): Json<CreateProfileRequest>,
) -> impl IntoResponse {
    let (profile, to_save) = {
        let mut inner = state.inner.write().await;
        let id = slugify(&request.name);
        let game_id = normalize_optional_profile_game_id(request.game_id);
        if inner.profiles.iter().any(|profile| profile.id == id) {
            return (
                StatusCode::CONFLICT,
                Json(ProfileSummary {
                    id,
                    name: request.name,
                    built_in: false,
                    active: false,
                    game_id,
                }),
            );
        }

        let profile = ProfileSummary {
            id,
            name: request.name,
            built_in: false,
            active: false,
            game_id,
        };
        inner.profiles.push(profile.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        (profile, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    (StatusCode::CREATED, Json(profile))
}

pub(crate) async fn get_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ProfileSummary>, StatusCode> {
    let inner = state.inner.read().await;
    inner
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub(crate) async fn export_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ExportedProfile>, StatusCode> {
    let inner = state.inner.read().await;
    let profile = inner
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ExportedProfile {
        schema: "dev.dscc.profile.v1".to_string(),
        config: inner.profile_configs.get(&profile.id).cloned(),
        id: profile.id,
        name: profile.name,
        built_in: profile.built_in,
        active: profile.active,
        game_id: profile.game_id,
    }))
}

pub(crate) async fn import_profile(
    State(state): State<AgentState>,
    Json(request): Json<ImportProfileRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if request.schema != "dev.dscc.profile.v1" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (profile, to_save) = {
        let mut inner = state.inner.write().await;
        let mut id = request.id.unwrap_or_else(|| slugify(&request.name));
        let game_id = normalize_optional_profile_game_id(request.game_id);
        if id.trim().is_empty() {
            id = slugify(&request.name);
        }
        if inner.profiles.iter().any(|profile| profile.id == id) {
            return Ok((
                StatusCode::CONFLICT,
                Json(ProfileSummary {
                    id,
                    name: request.name,
                    built_in: false,
                    active: false,
                    game_id,
                }),
            ));
        }

        let profile = ProfileSummary {
            id,
            name: request.name,
            built_in: false,
            active: false,
            game_id,
        };
        if let Some(config) = request.config {
            inner.profile_configs.insert(profile.id.clone(), config);
        }
        inner.profiles.push(profile.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        (profile, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok((StatusCode::CREATED, Json(profile)))
}

pub(crate) async fn update_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<ProfileSummary>, StatusCode> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (updated, to_save) = {
        let mut inner = state.inner.write().await;
        let profile_index = inner
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        if inner.profiles[profile_index].built_in {
            return Err(StatusCode::FORBIDDEN);
        }

        if inner
            .profiles
            .iter()
            .any(|profile| profile.id != id && profile.name.trim().eq_ignore_ascii_case(name))
        {
            return Err(StatusCode::CONFLICT);
        }

        inner.profiles[profile_index].name = name.to_string();
        let updated = inner.profiles[profile_index].clone();
        for config in inner.controller_configs.values_mut() {
            for assignment in &mut config.profile_assignments {
                if assignment.profile_id == id {
                    assignment.profile_name = updated.name.clone();
                }
            }
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Renamed profile {}", updated.name),
            timestamp: current_timestamp(),
        });
        (updated, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-renamed".to_string()),
    });
    Ok(Json(updated))
}

pub(crate) async fn update_profile_config(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateProfileConfigRequest>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let model_hint = request
        .model
        .clone()
        .unwrap_or_else(|| model_hint_for_profile_buttons(&request.buttons).to_string());
    let (profile_name, to_save) = {
        let mut inner = state.inner.write().await;
        let profile_name = inner
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| {
                if profile.built_in {
                    None
                } else {
                    Some(profile.name.clone())
                }
            })
            .ok_or(StatusCode::NOT_FOUND)?;
        let Some(profile_name) = profile_name else {
            return Err(StatusCode::FORBIDDEN);
        };
        let existing_input_bridge = inner
            .profile_configs
            .get(&id)
            .map(|config| config.input_bridge.clone())
            .unwrap_or_default();
        let profile_config = ProfileConfig {
            input_mode: request.input_mode,
            trigger: request.trigger,
            lightbar: request.lightbar,
            forza: request.forza,
            sticks: request.sticks,
            buttons: request.buttons,
            input_bridge: request.input_bridge.unwrap_or(existing_input_bridge),
        }
        .normalized_for_model(&model_hint);

        inner
            .profile_configs
            .insert(id.clone(), profile_config.clone());
        if inner.active_profile_id.as_deref() == Some(id.as_str())
            || inner.auto_loaded_profile_id.as_deref() == Some(id.as_str())
        {
            apply_profile_config_to_controllers(
                &mut inner,
                &SelectedProfileConfig::Full(profile_config.clone()),
            );
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Profile settings saved for {profile_name}"),
            timestamp: current_timestamp(),
        });
        (profile_name, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-config-saved".to_string()),
    });

    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Saved profile {profile_name}"),
        dry_run: None,
    }))
}

pub(crate) async fn delete_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let (deleted_name, to_save) = {
        let mut inner = state.inner.write().await;
        let profile = inner
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        if profile.built_in {
            return Err(StatusCode::FORBIDDEN);
        }
        let deleted_name = profile.name.clone();

        inner.profiles.retain(|profile| profile.id != id);
        inner.profile_configs.remove(&id);
        inner
            .profile_overrides
            .retain(|_, override_profile| override_profile.profile_id != id);
        for config in inner.controller_configs.values_mut() {
            config
                .profile_assignments
                .retain(|assignment| assignment.profile_id != id);
        }
        if inner.active_profile_id.as_deref() == Some(id.as_str()) {
            inner.active_profile_id = Some(DEFAULT_PROFILE_ID.to_string());
            apply_profile_selection_config(&mut inner, DEFAULT_PROFILE_ID);
        }
        if inner.auto_loaded_profile_id.as_deref() == Some(id.as_str()) {
            inner.auto_loaded_profile_id = None;
        }
        let active_profile_id = inner.active_profile_id.clone();
        for profile in &mut inner.profiles {
            profile.active = active_profile_id.as_deref() == Some(profile.id.as_str());
        }
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        inner.logs.push(LogEntry {
            level: "info".to_string(),
            message: format!("Deleted profile {deleted_name}"),
            timestamp: current_timestamp(),
        });
        (deleted_name, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("profile-deleted".to_string()),
    });
    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Deleted profile {deleted_name}"),
        dry_run: None,
    }))
}

pub(crate) async fn activate_profile(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<ActionAccepted>, StatusCode> {
    let to_save = {
        let mut inner = state.inner.write().await;
        if !inner.profiles.iter().any(|profile| profile.id == id) {
            return Err(StatusCode::NOT_FOUND);
        }

        for profile in &mut inner.profiles {
            profile.active = profile.id == id;
        }
        inner.active_profile_id = Some(id.clone());
        inner.effect_revision = inner.effect_revision.saturating_add(1);

        apply_profile_selection_config(&mut inner, &id);

        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;

    Ok(Json(ActionAccepted {
        accepted: true,
        message: format!("Activated profile {id}"),
        dry_run: None,
    }))
}

pub(crate) async fn list_adapters(State(state): State<AgentState>) -> Json<Vec<AdapterSummary>> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(materialized_adapters(
        &inner.adapters,
        &inner.adapter_runtimes,
        Some(&game_detection),
    ))
}

pub(crate) async fn update_adapter(
    Path(id): Path<String>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateAdapterRequest>,
) -> Result<Json<AdapterSummary>, StatusCode> {
    let game_detection = state.cached_game_detection().await;
    let (updated, to_save) = {
        let mut inner = state.inner.write().await;
        let adapter = inner
            .adapters
            .iter_mut()
            .find(|adapter| adapter.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;

        adapter.enabled = request.enabled;
        adapter.state = if request.enabled {
            "needs_setup".to_string()
        } else {
            "disabled".to_string()
        };
        let mut updated = adapter.clone();
        if let Some(runtime) = inner.adapter_runtime(&updated.id) {
            apply_adapter_runtime_summary(
                &mut updated,
                runtime,
                Some(&game_detection),
                Instant::now(),
            );
        }
        (updated, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(updated))
}

pub(crate) async fn get_steam_input_status(
    State(state): State<AgentState>,
) -> Json<SteamInputStatus> {
    Json(state.cached_steam_input_status().await)
}

pub(crate) async fn update_steam_input_binding(
    State(state): State<AgentState>,
    Json(request): Json<SteamInputBindingWriteRequest>,
) -> Result<Json<SteamInputBindingWriteResponse>, (StatusCode, String)> {
    let dry_run = request.dry_run;
    let response = tokio::task::spawn_blocking(move || write_steam_input_binding(request))
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Steam Input writer task failed: {error}"),
            )
        })?
        .map_err(|error| (error.status, error.message))?;

    if !dry_run {
        state.spawn_steam_input_refresh();
    }

    Ok(Json(response))
}

pub(crate) async fn apply_steam_input_paddle_preset(
    State(state): State<AgentState>,
    Json(request): Json<SteamInputPaddlePresetRequest>,
) -> Result<Json<SteamInputPaddlePresetResponse>, (StatusCode, String)> {
    let dry_run = request.dry_run;
    let mut response =
        tokio::task::spawn_blocking(move || write_steam_input_paddle_preset(request))
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Steam Input paddle preset task failed: {error}"),
                )
            })?
            .map_err(|error| (error.status, error.message))?;

    if !dry_run {
        let steam_input = state.cached_steam_input_status_or_refresh().await;
        if !steam_input.running {
            response.warnings.push(
                "Steam is not currently running; restart Steam or reopen the game if the layout is not picked up immediately."
                    .to_string(),
            );
        }
        state.spawn_steam_input_refresh();
    }

    Ok(Json(response))
}

pub(crate) async fn list_modules() -> Json<Vec<ModuleSummary>> {
    Json(module_summaries())
}

pub(crate) async fn get_profile_resolution(
    State(state): State<AgentState>,
) -> Json<ProfileResolutionResponse> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(profile_resolution(&inner, Some(&game_detection)))
}

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

pub(crate) async fn get_current_effect(
    State(state): State<AgentState>,
) -> Json<CurrentEffectResponse> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(state.current_effect_response_cached(
        &inner,
        Some(&game_detection),
        state.hardware_output_enabled(),
        EffectEnginePurpose::Preview,
    ))
}

pub(crate) async fn set_profile_override(
    State(state): State<AgentState>,
    Json(request): Json<ProfileOverride>,
) -> Result<Json<ProfileResolutionResponse>, StatusCode> {
    let game_detection = state.cached_game_detection().await;
    let (resolution, to_save) = {
        let mut inner = state.inner.write().await;
        if !inner
            .profiles
            .iter()
            .any(|profile| profile.id == request.profile_id)
        {
            return Err(StatusCode::NOT_FOUND);
        }

        inner.profile_overrides.insert(
            profile_override_key(request.controller_id.as_deref(), request.game_id.as_deref()),
            request,
        );
        sync_auto_loaded_profile_for_detection(&mut inner, &game_detection);
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let resolution = profile_resolution(&inner, Some(&game_detection));
        (resolution, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Ok(Json(resolution))
}

pub(crate) async fn clear_profile_override(
    State(state): State<AgentState>,
    Query(scope): Query<ProfileOverrideScope>,
) -> Json<ProfileResolutionResponse> {
    let game_detection = state.cached_game_detection().await;
    let (resolution, to_save) = {
        let mut inner = state.inner.write().await;
        let controller_id = scope.controller_id.as_deref().filter(|id| !id.is_empty());
        let game_id = scope.game_id.as_deref().filter(|id| !id.is_empty());
        if controller_id.is_some() || game_id.is_some() {
            inner
                .profile_overrides
                .remove(&profile_override_key(controller_id, game_id));
        } else {
            inner.profile_overrides.clear();
        }
        sync_auto_loaded_profile_for_detection(&mut inner, &game_detection);
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let resolution = profile_resolution(&inner, Some(&game_detection));
        (resolution, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    Json(resolution)
}

pub(crate) async fn list_telemetry(
    State(state): State<AgentState>,
) -> Json<Vec<TelemetrySignalResponse>> {
    let game_detection = state.cached_game_detection().await;
    let inner = state.inner.read().await;
    Json(materialized_telemetry_response(
        &inner,
        Some(&game_detection),
    ))
}

pub(crate) async fn list_logs(State(state): State<AgentState>) -> Json<Vec<LogEntry>> {
    let inner = state.inner.read().await;
    Json(inner.logs.clone())
}

pub(crate) async fn get_diagnostics(State(state): State<AgentState>) -> Json<DiagnosticsResponse> {
    Json(state.diagnostics().await)
}

pub(crate) async fn get_support_bundle(
    State(state): State<AgentState>,
) -> Json<SupportBundleResponse> {
    Json(state.support_bundle().await)
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<AgentState>,
) -> impl IntoResponse {
    if !request_origin_matches_host(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    ws.on_upgrade(move |socket| websocket_session(socket, state))
        .into_response()
}

pub(crate) async fn websocket_session(mut socket: WebSocket, state: AgentState) {
    let mut events = state.subscribe_events();
    let payload = serde_json::json!({
        "type": "snapshot",
        "snapshot": state.snapshot().await
    });

    if socket
        .send(Message::Text(payload.to_string()))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            maybe_message = socket.recv() => {
                match maybe_message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        let Ok(text) = serde_json::to_string(&event) else {
                            continue;
                        };
                        if socket.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    let _ = socket.close().await;
}
