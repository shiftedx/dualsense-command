use super::*;

mod controllers;
mod effects;
mod input_bridge;
mod profiles;
mod settings;

pub(crate) use controllers::*;
pub(crate) use effects::*;
pub(crate) use input_bridge::*;
pub(crate) use profiles::*;
pub(crate) use settings::*;
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
