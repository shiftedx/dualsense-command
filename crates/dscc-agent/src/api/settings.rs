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
