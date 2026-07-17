use super::*;

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
        // Imports cross a trust boundary: bound the id and name and run the
        // config through the same normalization as PUT config below.
        let name = request.name.trim().chars().take(96).collect::<String>();
        let mut id = request
            .id
            .map(|id| id.trim().chars().take(96).collect::<String>())
            .unwrap_or_default();
        let game_id = normalize_optional_profile_game_id(request.game_id);
        if id.is_empty() {
            id = slugify(&name);
        }
        if inner.profiles.iter().any(|profile| profile.id == id) {
            return Ok((
                StatusCode::CONFLICT,
                Json(ProfileSummary {
                    id,
                    name,
                    built_in: false,
                    active: false,
                    game_id,
                }),
            ));
        }

        let profile = ProfileSummary {
            id,
            name,
            built_in: false,
            active: false,
            game_id,
        };
        if let Some(config) = request.config {
            let model_hint = model_hint_for_profile_buttons(&config.buttons);
            inner
                .profile_configs
                .insert(profile.id.clone(), config.normalized_for_model(model_hint));
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
