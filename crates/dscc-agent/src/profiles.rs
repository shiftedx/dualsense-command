use super::*;

pub(crate) fn default_profiles() -> Vec<ProfileSummary> {
    vec![
        ProfileSummary {
            id: DEFAULT_PROFILE_ID.to_string(),
            name: "Global".to_string(),
            built_in: true,
            active: true,
            game_id: None,
        },
        ProfileSummary {
            id: FORZA_HORIZON_PROFILE_ID.to_string(),
            name: "Base".to_string(),
            built_in: true,
            active: false,
            game_id: None,
        },
        ProfileSummary {
            id: IMMERSIVE_PROFILE_ID.to_string(),
            name: "Immersive".to_string(),
            built_in: true,
            active: false,
            game_id: None,
        },
        ProfileSummary {
            id: ASSETTO_CORSA_RALLY_PROFILE_ID.to_string(),
            name: "Rally".to_string(),
            built_in: true,
            active: false,
            game_id: Some("assetto-corsa-rally".to_string()),
        },
    ]
}

pub(crate) fn merge_profiles(persisted_profiles: Vec<ProfileSummary>) -> Vec<ProfileSummary> {
    let mut profiles = default_profiles();
    for mut profile in persisted_profiles {
        profile.built_in = false;
        profile.active = false;
        profile.game_id = normalize_optional_profile_game_id(profile.game_id);
        if !profile.id.trim().is_empty() && !profiles.iter().any(|item| item.id == profile.id) {
            profiles.push(profile);
        }
    }
    profiles
}

pub(crate) fn normalize_optional_profile_game_id(game_id: Option<String>) -> Option<String> {
    game_id
        .map(|id| id.trim().chars().take(96).collect::<String>())
        .filter(|id| !id.is_empty() && id != "all" && id != "global")
}

pub(crate) fn profiles_with_active(
    mut profiles: Vec<ProfileSummary>,
    active_profile_id: &Option<String>,
) -> Vec<ProfileSummary> {
    let active = active_profile_id.as_deref().unwrap_or(DEFAULT_PROFILE_ID);
    for profile in &mut profiles {
        profile.active = profile.id == active;
    }
    profiles
}

pub(crate) fn profile_exists_in_defaults_or_persisted(
    id: &str,
    persisted_profiles: &[ProfileSummary],
) -> bool {
    is_default_profile_id(id) || persisted_profiles.iter().any(|profile| profile.id == id)
}

pub(crate) fn is_default_profile_id(id: &str) -> bool {
    default_profiles().iter().any(|profile| profile.id == id)
}

#[derive(Clone)]
pub(crate) enum SelectedProfileConfig {
    Full(ProfileConfig),
    BuiltInPreset {
        trigger: Option<TriggerConfig>,
        forza: ForzaTelemetryConfig,
    },
}

pub(crate) fn selected_profile_config(
    inner: &AgentStateInner,
    profile_id: &str,
) -> Option<SelectedProfileConfig> {
    if let Some(forza) = forza_preset_for_profile(profile_id) {
        return Some(SelectedProfileConfig::BuiltInPreset {
            trigger: forza_trigger_preset_for_profile(profile_id),
            forza,
        });
    }

    if is_default_profile_id(profile_id) {
        return None;
    }

    inner
        .profile_configs
        .get(profile_id)
        .cloned()
        .map(SelectedProfileConfig::Full)
        .or_else(|| {
            forza_preset_for_profile(profile_id).map(|forza| SelectedProfileConfig::BuiltInPreset {
                trigger: forza_trigger_preset_for_profile(profile_id),
                forza,
            })
        })
}

pub(crate) fn forza_trigger_preset_for_profile(profile_id: &str) -> Option<TriggerConfig> {
    matches!(
        profile_id,
        FORZA_HORIZON_PROFILE_ID | IMMERSIVE_PROFILE_ID | ASSETTO_CORSA_RALLY_PROFILE_ID
    )
    .then(forza_horizon_trigger_preset)
}

pub(crate) fn apply_selected_profile_config(
    config: &mut ControllerConfig,
    selected: &SelectedProfileConfig,
) {
    match selected {
        SelectedProfileConfig::Full(profile_config) => {
            profile_config.apply_to_controller_config(config);
        }
        SelectedProfileConfig::BuiltInPreset { trigger, forza } => {
            if let Some(trigger) = trigger {
                config.trigger = trigger.clone();
            }
            config.forza = forza.clone();
        }
    }
}

pub(crate) fn apply_profile_config_to_controllers(
    inner: &mut AgentStateInner,
    selected_config: &SelectedProfileConfig,
) {
    let connected_models: BTreeMap<String, String> = inner
        .controllers
        .summaries()
        .into_iter()
        .filter(|controller| controller.connected)
        .map(|controller| (controller.id, controller.model))
        .collect();
    let mut controller_ids: Vec<String> = inner.controller_configs.keys().cloned().collect();
    for controller_id in connected_models.keys() {
        if !controller_ids.iter().any(|id| id == controller_id) {
            controller_ids.push(controller_id.clone());
        }
    }

    for controller_id in controller_ids {
        let model = connected_models
            .get(&controller_id)
            .cloned()
            .or_else(|| {
                inner
                    .controller_configs
                    .get(&controller_id)
                    .map(|config| config.model.clone())
            })
            .unwrap_or_else(|| "DualSense".to_string());
        let config = inner
            .controller_configs
            .entry(controller_id.clone())
            .or_insert_with(|| ControllerConfig::default_for(controller_id.clone(), model));
        apply_selected_profile_config(config, selected_config);
    }
}

pub(crate) fn apply_profile_selection_config(inner: &mut AgentStateInner, profile_id: &str) {
    if let Some(selected_config) = selected_profile_config(inner, profile_id) {
        apply_profile_config_to_controllers(inner, &selected_config);
    }
}

pub(crate) fn sync_auto_loaded_profile_for_detection(
    inner: &mut AgentStateInner,
    game_detection: &GameDetectionResponse,
) -> bool {
    let target_profile_id = if game_detection.profile_id.is_some() {
        profile_resolution(inner, Some(game_detection)).selected_profile_id
    } else {
        None
    };

    if inner.auto_loaded_profile_id == target_profile_id {
        return false;
    }

    match target_profile_id.as_deref() {
        Some(profile_id) => {
            apply_profile_selection_config(inner, profile_id);
        }
        None => {
            let fallback_profile_id = inner
                .active_profile_id
                .clone()
                .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
            apply_profile_selection_config(inner, &fallback_profile_id);
        }
    }

    inner.auto_loaded_profile_id = target_profile_id;
    inner.effect_revision = inner.effect_revision.saturating_add(1);
    true
}

pub(crate) fn default_profile_assignments(edge: bool) -> Vec<ProfileAssignmentConfig> {
    vec![
        ProfileAssignmentConfig {
            game_id: "forza-horizon-6".to_string(),
            game_name: "Forza Horizon 6".to_string(),
            profile_id: IMMERSIVE_PROFILE_ID.to_string(),
            profile_name: "Immersive".to_string(),
            state: "ready".to_string(),
            detail: if edge {
                "Throttle, brake, slip, road texture (Edge)"
            } else {
                "Throttle, brake, slip, road texture"
            }
            .to_string(),
        },
        ProfileAssignmentConfig {
            game_id: "forza-horizon-5".to_string(),
            game_name: "Forza Horizon 5".to_string(),
            profile_id: IMMERSIVE_PROFILE_ID.to_string(),
            profile_name: "Immersive".to_string(),
            state: "ready".to_string(),
            detail: "Horizon 5-compatible Data Out signals".to_string(),
        },
        ProfileAssignmentConfig {
            game_id: "assetto-corsa-rally".to_string(),
            game_name: "Assetto Corsa Rally".to_string(),
            profile_id: ASSETTO_CORSA_RALLY_PROFILE_ID.to_string(),
            profile_name: "Rally".to_string(),
            state: "ready".to_string(),
            detail: "Shared-memory rally telemetry".to_string(),
        },
    ]
}

pub(crate) fn model_hint_for_profile_buttons(buttons: &[ButtonAssignmentConfig]) -> &'static str {
    if buttons.iter().any(|button| {
        matches!(
            button.key.as_str(),
            "Back Left" | "Back Right" | "Fn Left" | "Fn Right"
        )
    }) {
        "DualSense Edge"
    } else {
        "DualSense"
    }
}

pub(crate) fn normalize_profile_assignments(
    assignments: Vec<ProfileAssignmentConfig>,
) -> Vec<ProfileAssignmentConfig> {
    assignments
        .into_iter()
        .filter(|assignment| {
            !assignment.game_id.trim().is_empty() && !assignment.profile_id.trim().is_empty()
        })
        .take(12)
        .collect()
}

pub(crate) fn normalize_existing_profile_assignments(
    assignments: Vec<ProfileAssignmentConfig>,
    persisted_profiles: &[ProfileSummary],
) -> Vec<ProfileAssignmentConfig> {
    normalize_profile_assignments(assignments)
        .into_iter()
        .filter(|assignment| {
            profile_exists_in_defaults_or_persisted(&assignment.profile_id, persisted_profiles)
        })
        .collect()
}

pub(crate) fn profile_override_key(controller_id: Option<&str>, game_id: Option<&str>) -> String {
    format!(
        "{}:{}",
        controller_id.unwrap_or("*"),
        game_id.unwrap_or("*")
    )
}

pub(crate) fn profile_resolution(
    inner: &AgentStateInner,
    game_detection: Option<&GameDetectionResponse>,
) -> ProfileResolutionResponse {
    let controller_id = inner
        .controllers
        .summaries()
        .into_iter()
        .find(|controller| controller.connected)
        .map(|controller| controller.id);
    let detected_game_id = game_detection.and_then(|detection| detection.active_game_id.clone());
    let detected_adapter_id = game_detection.and_then(|detection| {
        detection
            .active_game_id
            .as_ref()
            .and_then(|_| detection.adapter_id.clone())
    });
    let active_adapter_id = detected_adapter_id
        .clone()
        .or_else(|| inner.active_adapter_id.clone())
        .or_else(|| inner.telemetry.text("source.id").map(str::to_string));
    let override_key = profile_override_key(controller_id.as_deref(), detected_game_id.as_deref());
    let fallback_override_key = profile_override_key(None, detected_game_id.as_deref());
    let controller_global_override_key = profile_override_key(controller_id.as_deref(), None);
    let global_override_key = profile_override_key(None, None);
    let override_profile = inner
        .profile_overrides
        .get(&override_key)
        .or_else(|| inner.profile_overrides.get(&fallback_override_key))
        .or_else(|| {
            detected_game_id
                .is_none()
                .then(|| inner.profile_overrides.get(&controller_global_override_key))
                .flatten()
        })
        .or_else(|| {
            detected_game_id
                .is_none()
                .then(|| inner.profile_overrides.get(&global_override_key))
                .flatten()
        });

    let assigned_profile_id = controller_id.as_deref().and_then(|id| {
        if let Some(config) = inner.controller_configs.get(id) {
            assigned_profile_for(config, detected_game_id.as_deref())
        } else {
            inner.controllers.detail(id).and_then(|detail| {
                let config = ControllerConfig::default_for(id, detail.model);
                assigned_profile_for(&config, detected_game_id.as_deref())
            })
        }
    });
    let module_profile_id = game_detection.and_then(|detection| detection.profile_id.clone());
    let selected_profile_id = override_profile
        .map(|profile| profile.profile_id.clone())
        .or_else(|| assigned_profile_id.clone())
        .or_else(|| module_profile_id.clone())
        .or_else(|| inner.active_profile_id.clone());
    let validation = if selected_profile_id
        .as_deref()
        .is_some_and(|id| inner.profiles.iter().any(|profile| profile.id == id))
    {
        "valid"
    } else {
        "missing_profile"
    };

    ProfileResolutionResponse {
        controller_id,
        detected_game_id,
        active_adapter_id,
        selected_profile_id,
        reason: if override_profile.is_some() {
            "manual_override".to_string()
        } else if game_detection.is_some_and(|detection| detection.profile_id.is_some()) {
            "foreground_game".to_string()
        } else if assigned_profile_id.is_some() {
            "telemetry_source".to_string()
        } else if module_profile_id.is_some() {
            "module_template".to_string()
        } else if inner.active_adapter_id.is_some() {
            "active_telemetry_source".to_string()
        } else {
            "global_default".to_string()
        },
        override_profile_id: override_profile.map(|profile| profile.profile_id.clone()),
        validation: validation.to_string(),
    }
}

pub(crate) fn assigned_profile_for(
    config: &ControllerConfig,
    game_id: Option<&str>,
) -> Option<String> {
    let game_id = game_id?;
    config
        .profile_assignments
        .iter()
        .find(|assignment| profile_assignment_matches(&assignment.game_id, game_id))
        .map(|assignment| assignment.profile_id.clone())
}

pub(crate) fn profile_assignment_matches(assignment_game_id: &str, detected_game_id: &str) -> bool {
    assignment_game_id == detected_game_id
}

pub(crate) fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "untitled-profile".to_string()
    } else {
        slug
    }
}
