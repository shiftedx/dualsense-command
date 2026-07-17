use super::*;

pub(crate) const EDGE_ONBOARD_PROFILE_NAME_MAX_CHARS: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeProfileSupportState {
    Unsupported,
    Unknown,
    ReadOnly,
    ReadWrite,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeProfileSlotState {
    Default,
    Assigned,
    Empty,
    Active,
    Unknown,
    Faulted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileSlot {
    pub slot_id: String,
    pub shortcut: String,
    pub name: Option<String>,
    pub state: EdgeProfileSlotState,
    pub editable: bool,
    pub hardware_synced: bool,
    pub staged: Option<EdgeProfileSlotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfilesResponse {
    pub controller_id: String,
    pub support_state: EdgeProfileSupportState,
    pub warning: String,
    pub slots: Vec<EdgeProfileSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileSlotConfig {
    pub name: String,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
    pub updated_at: String,
    pub hardware_synced: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeProfileStore {
    pub slots: BTreeMap<String, EdgeProfileSlotConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEdgeProfileRequest {
    pub name: String,
    pub trigger: TriggerConfig,
    #[serde(default)]
    pub lightbar: LightbarConfig,
    pub sticks: StickConfig,
    #[serde(default)]
    pub buttons: Vec<ButtonAssignmentConfig>,
}

impl EdgeProfileStore {
    pub(crate) fn normalized(mut self) -> Self {
        self.slots = self
            .slots
            .into_iter()
            .map(|(slot, config)| (slot, config.normalized()))
            .collect();
        self
    }
}

impl EdgeProfileSlotConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.name = normalize_edge_onboard_profile_name(&self.name);
        self.trigger = self.trigger.normalized();
        self.lightbar = self.lightbar.normalized();
        self.sticks = self.sticks.normalized();
        self.buttons = normalize_controller_button_assignments(self.buttons, true);
        self
    }
}

impl EdgeProfilesResponse {
    fn for_controller(detail: &ControllerDetail, store: Option<&EdgeProfileStore>) -> Self {
        if detail.model != "DualSense Edge" {
            return Self {
                controller_id: detail.id.clone(),
                support_state: EdgeProfileSupportState::Unsupported,
                warning:
                    "Onboard profile read/write is only planned for DualSense Edge controllers."
                        .to_string(),
                slots: Vec::new(),
            };
        }

        Self {
            controller_id: detail.id.clone(),
            support_state: EdgeProfileSupportState::Unknown,
            warning: "Edge onboard slot editing is available as DSCC staged configuration. Connect the DualSense Edge over USB or Bluetooth to read slots and sync controller memory when HID writes are available.".to_string(),
            slots: edge_profile_slots(store),
        }
    }

    fn for_controller_with_hardware(
        detail: &ControllerDetail,
        store: Option<&EdgeProfileStore>,
        hardware_profiles: Option<&[EdgeOnboardProfile]>,
        hardware_warning: Option<String>,
        hardware_writes_enabled: bool,
    ) -> Self {
        if detail.model != "DualSense Edge" {
            return Self::for_controller(detail, store);
        }

        let Some(hardware_profiles) = hardware_profiles else {
            let warning = hardware_warning.unwrap_or_else(|| {
                "Edge onboard slots can be staged locally. Connect the DualSense Edge over USB or Bluetooth and refresh to read slots and sync controller memory when HID writes are available.".to_string()
            });
            return Self {
                controller_id: detail.id.clone(),
                support_state: EdgeProfileSupportState::Unknown,
                warning,
                slots: edge_profile_slots(store),
            };
        };

        let transport_kind = match detail.transport.as_str() {
            "usb" => DeviceTransportKind::Usb,
            "bluetooth" => DeviceTransportKind::Bluetooth,
            _ => DeviceTransportKind::Unknown,
        };
        let write_supported =
            hardware_writes_enabled && edge_onboard_write_transport_supported(transport_kind);
        let (support_state, warning) = if write_supported {
            (
                EdgeProfileSupportState::ReadWrite,
                format!("DualSense Edge onboard slots were read over {}. DSCC can write static onboard trigger, stick, and button settings, then verifies the slot with acknowledgement and readback; live telemetry effects still require DSCC to be running.", detail.transport),
            )
        } else {
            (
                EdgeProfileSupportState::ReadOnly,
                format!("DualSense Edge onboard slots were read over {}, but hardware writes are disabled by DSCC output mode. Slot changes will be staged locally.", detail.transport),
            )
        };

        Self {
            controller_id: detail.id.clone(),
            support_state,
            warning,
            slots: edge_profile_slots_with_hardware(store, hardware_profiles),
        }
    }
}

pub(crate) fn edge_profile_slots(store: Option<&EdgeProfileStore>) -> Vec<EdgeProfileSlot> {
    let staged = |slot: &str| store.and_then(|store| store.slots.get(slot)).cloned();
    let slot_state = |slot: &str| {
        if staged(slot).is_some() {
            EdgeProfileSlotState::Assigned
        } else {
            EdgeProfileSlotState::Unknown
        }
    };

    vec![
        EdgeProfileSlot {
            slot_id: "default".to_string(),
            shortcut: "Fn + Triangle".to_string(),
            name: Some("Default Profile".to_string()),
            state: EdgeProfileSlotState::Default,
            editable: false,
            hardware_synced: true,
            staged: None,
        },
        EdgeProfileSlot {
            slot_id: "circle".to_string(),
            shortcut: "Fn + Circle".to_string(),
            name: staged("circle").map(|profile| profile.name),
            state: slot_state("circle"),
            editable: true,
            hardware_synced: false,
            staged: staged("circle"),
        },
        EdgeProfileSlot {
            slot_id: "cross".to_string(),
            shortcut: "Fn + Cross".to_string(),
            name: staged("cross").map(|profile| profile.name),
            state: slot_state("cross"),
            editable: true,
            hardware_synced: false,
            staged: staged("cross"),
        },
        EdgeProfileSlot {
            slot_id: "square".to_string(),
            shortcut: "Fn + Square".to_string(),
            name: staged("square").map(|profile| profile.name),
            state: slot_state("square"),
            editable: true,
            hardware_synced: false,
            staged: staged("square"),
        },
    ]
}

pub(crate) fn edge_profile_slots_with_hardware(
    store: Option<&EdgeProfileStore>,
    hardware_profiles: &[EdgeOnboardProfile],
) -> Vec<EdgeProfileSlot> {
    [
        EdgeOnboardSlotId::Default,
        EdgeOnboardSlotId::Square,
        EdgeOnboardSlotId::Cross,
        EdgeOnboardSlotId::Circle,
    ]
    .into_iter()
    .map(|slot| {
        let slot_id = slot.as_str();
        let staged = store.and_then(|store| store.slots.get(slot_id)).cloned();
        let hardware = hardware_profiles
            .iter()
            .find(|profile| profile.slot == slot)
            .cloned();

        if slot == EdgeOnboardSlotId::Default {
            return EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: hardware
                    .as_ref()
                    .filter(|profile| profile.assigned && !profile.name.trim().is_empty())
                    .map(|profile| profile.name.clone())
                    .or_else(|| Some("Default Profile".to_string())),
                state: EdgeProfileSlotState::Default,
                editable: false,
                hardware_synced: true,
                staged: hardware.as_ref().map(edge_profile_config_from_hardware),
            };
        }

        if let Some(staged) = staged.filter(|profile| !profile.hardware_synced) {
            return EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: Some(staged.name.clone()),
                state: EdgeProfileSlotState::Assigned,
                editable: true,
                hardware_synced: false,
                staged: Some(staged),
            };
        }

        match hardware {
            Some(profile) if profile.assigned => {
                let config = edge_profile_config_from_hardware(&profile);
                EdgeProfileSlot {
                    slot_id: slot_id.to_string(),
                    shortcut: slot.shortcut().to_string(),
                    name: Some(config.name.clone()),
                    state: EdgeProfileSlotState::Assigned,
                    editable: true,
                    hardware_synced: true,
                    staged: Some(config),
                }
            }
            Some(_) => EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: None,
                state: EdgeProfileSlotState::Empty,
                editable: true,
                hardware_synced: true,
                staged: None,
            },
            None => EdgeProfileSlot {
                slot_id: slot_id.to_string(),
                shortcut: slot.shortcut().to_string(),
                name: store
                    .and_then(|store| store.slots.get(slot_id))
                    .map(|profile| profile.name.clone()),
                state: EdgeProfileSlotState::Unknown,
                editable: true,
                hardware_synced: false,
                staged: store.and_then(|store| store.slots.get(slot_id)).cloned(),
            },
        }
    })
    .collect()
}

pub(crate) fn edge_profile_config_from_request(
    request: UpdateEdgeProfileRequest,
) -> EdgeProfileSlotConfig {
    EdgeProfileSlotConfig {
        name: normalize_edge_onboard_profile_name(&request.name),
        trigger: request.trigger.normalized(),
        lightbar: request.lightbar.normalized(),
        sticks: request.sticks.normalized(),
        buttons: normalize_controller_button_assignments(request.buttons, true),
        updated_at: current_timestamp(),
        hardware_synced: false,
    }
}

pub(crate) fn edge_slot_id_from_api(slot: &str) -> Option<EdgeOnboardSlotId> {
    match slot {
        "default" | "triangle" => Some(EdgeOnboardSlotId::Default),
        "square" => Some(EdgeOnboardSlotId::Square),
        "cross" => Some(EdgeOnboardSlotId::Cross),
        "circle" => Some(EdgeOnboardSlotId::Circle),
        _ => None,
    }
}

pub(crate) fn edge_profile_config_from_hardware(
    profile: &EdgeOnboardProfile,
) -> EdgeProfileSlotConfig {
    let mut trigger = TriggerConfig::default();
    trigger.same_range = profile.trigger_deadzone.unified;
    trigger.l2_from = profile.trigger_deadzone.left[0].min(100);
    trigger.l2_to = profile.trigger_deadzone.left[1].clamp(trigger.l2_from, 100);
    trigger.r2_from = profile.trigger_deadzone.right[0].min(100);
    trigger.r2_to = profile.trigger_deadzone.right[1].clamp(trigger.r2_from, 100);
    trigger.intensity = edge_trigger_intensity_label(profile.trigger_effect_intensity).to_string();
    trigger.vibration = edge_vibration_label(profile.vibration_intensity).to_string();

    EdgeProfileSlotConfig {
        name: normalize_edge_onboard_profile_name_or(&profile.name, profile.slot.shortcut()),
        trigger: trigger.normalized(),
        lightbar: LightbarConfig::default(),
        sticks: StickConfig {
            left_curve: profile.left_stick.preset.as_str().to_string(),
            left_curve_amount: 50,
            left_deadzone: 0,
            right_curve: profile.right_stick.preset.as_str().to_string(),
            right_curve_amount: 50,
            right_deadzone: 0,
        }
        .normalized(),
        buttons: edge_button_assignments_from_hardware(&profile.button_mappings),
        updated_at: edge_profile_updated_at(profile.updated_at_ms),
        hardware_synced: true,
    }
}

pub(crate) fn edge_profile_from_slot_config(
    slot: EdgeOnboardSlotId,
    config: &EdgeProfileSlotConfig,
) -> EdgeOnboardProfile {
    let config = config.clone().normalized();
    let mut profile = EdgeOnboardProfile::new(slot, config.name.clone());
    profile.trigger_deadzone = EdgeTriggerDeadzone {
        left: [config.trigger.l2_from, config.trigger.l2_to],
        right: [config.trigger.r2_from, config.trigger.r2_to],
        unified: config.trigger.same_range,
    };
    profile.left_stick = EdgeStickProfile {
        preset: EdgeStickPreset::from_label(&config.sticks.left_curve),
        ..EdgeStickProfile::default()
    };
    profile.right_stick = EdgeStickProfile {
        preset: EdgeStickPreset::from_label(&config.sticks.right_curve),
        ..EdgeStickProfile::default()
    };
    profile.trigger_effect_intensity =
        edge_profile_intensity_from_trigger(&config.trigger.intensity);
    profile.vibration_intensity = edge_profile_intensity_from_vibration(&config.trigger.vibration);
    profile.button_mappings = edge_button_mappings_from_config(&config.buttons);
    profile.updated_at_ms = current_timestamp_millis();
    profile
}

pub(crate) fn normalize_edge_onboard_profile_name(name: &str) -> String {
    normalize_edge_onboard_profile_name_or(name, "Untitled Edge Profile")
}

pub(crate) fn normalize_edge_onboard_profile_name_or(name: &str, fallback: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        fallback
            .trim()
            .chars()
            .take(EDGE_ONBOARD_PROFILE_NAME_MAX_CHARS)
            .collect()
    } else {
        name.chars()
            .take(EDGE_ONBOARD_PROFILE_NAME_MAX_CHARS)
            .collect()
    }
}

pub(crate) fn edge_button_assignments_from_hardware(
    mappings: &[EdgeButtonMapping],
) -> Vec<ButtonAssignmentConfig> {
    let mut assignments: Vec<_> = mappings
        .iter()
        .filter(|mapping| mapping.source != mapping.target)
        .map(|mapping| ButtonAssignmentConfig {
            key: mapping.source.label().to_string(),
            label: mapping.target.label().to_string(),
        })
        .collect();

    if assignments.is_empty() {
        assignments = default_button_assignments(true);
    }

    normalize_controller_button_assignments(assignments, true)
}

pub(crate) fn edge_button_mappings_from_config(
    buttons: &[ButtonAssignmentConfig],
) -> Vec<EdgeButtonMapping> {
    let mut mappings = dscc_device::default_button_mappings().to_vec();
    for button in buttons {
        let Some(source) = EdgeButton::from_label(&button.key) else {
            continue;
        };
        let Some(target) = EdgeButton::from_label(&button.label) else {
            continue;
        };
        if let Some(mapping) = mappings.iter_mut().find(|mapping| mapping.source == source) {
            mapping.target = target;
        } else {
            mappings.push(EdgeButtonMapping { source, target });
        }
    }
    mappings
}

pub(crate) fn edge_trigger_intensity_label(value: EdgeProfileIntensity) -> &'static str {
    match value {
        EdgeProfileIntensity::Off => "Off",
        EdgeProfileIntensity::Weak => "Weak",
        EdgeProfileIntensity::Medium => "Medium",
        EdgeProfileIntensity::Strong => "Strong (Standard)",
    }
}

pub(crate) fn edge_vibration_label(value: EdgeProfileIntensity) -> &'static str {
    match value {
        EdgeProfileIntensity::Off => "Off",
        EdgeProfileIntensity::Weak => "Low",
        EdgeProfileIntensity::Medium => "Medium",
        EdgeProfileIntensity::Strong => "High",
    }
}

pub(crate) fn edge_profile_intensity_from_trigger(value: &str) -> EdgeProfileIntensity {
    EdgeProfileIntensity::from_label(value.trim().strip_suffix(" (Standard)").unwrap_or(value))
}

pub(crate) fn edge_profile_intensity_from_vibration(value: &str) -> EdgeProfileIntensity {
    EdgeProfileIntensity::from_label(match value.trim() {
        "Low" => "Weak",
        "High" => "Strong",
        other => other,
    })
}

pub(crate) fn edge_profile_updated_at(updated_at_ms: u64) -> String {
    if updated_at_ms == 0 {
        return current_timestamp();
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at_ms as i64)
        .map(|timestamp| timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(current_timestamp)
}

pub(crate) struct EdgeHardwareProfilesRead {
    profiles: Vec<EdgeOnboardProfile>,
    hardware_writes_enabled: bool,
}

pub(crate) enum EdgeHardwareProfileWriteResult {
    Written,
    StagedOnly(String),
    Failed(String),
}

pub(crate) async fn read_edge_profiles_from_hardware(
    state: &AgentState,
    controller_id: &str,
) -> Result<EdgeHardwareProfilesRead, String> {
    let manager = state
        .output_manager
        .clone()
        .ok_or_else(|| "HID output manager is unavailable".to_string())?;
    let target = {
        let inner = state.inner.read().await;
        controller_output_target_or_reason(&inner, controller_id)?
    };
    if !edge_onboard_transport_supported(target.transport) {
        return Err(
            "DualSense Edge onboard profile reads require USB or Bluetooth HID feature report access"
                .to_string(),
        );
    }

    let hardware_writes_enabled = manager.hardware_writes_enabled();
    let profiles = tokio::task::spawn_blocking(move || manager.read_edge_onboard_profiles(&target))
        .await
        .map_err(|error| format!("DualSense Edge profile read task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    Ok(EdgeHardwareProfilesRead {
        profiles,
        hardware_writes_enabled,
    })
}

pub(crate) async fn write_edge_profile_to_hardware(
    state: &AgentState,
    controller_id: &str,
    profile: EdgeOnboardProfile,
) -> EdgeHardwareProfileWriteResult {
    let Some(manager) = state.output_manager.clone() else {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "HID output manager is unavailable".to_string(),
        );
    };
    if !manager.hardware_writes_enabled() {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "hardware writes are disabled by DSCC output mode".to_string(),
        );
    }

    let target = {
        let inner = state.inner.read().await;
        match controller_output_target_or_reason(&inner, controller_id) {
            Ok(target) => target,
            Err(error) => return EdgeHardwareProfileWriteResult::StagedOnly(error),
        }
    };
    if !edge_onboard_write_transport_supported(target.transport) {
        return EdgeHardwareProfileWriteResult::StagedOnly(
            "DualSense Edge onboard profile writes require USB or Bluetooth HID feature report access"
                .to_string(),
        );
    }

    match tokio::task::spawn_blocking(move || manager.write_edge_onboard_profile(&target, &profile))
        .await
    {
        Ok(Ok(())) => EdgeHardwareProfileWriteResult::Written,
        Ok(Err(error)) => EdgeHardwareProfileWriteResult::Failed(error.to_string()),
        Err(error) => EdgeHardwareProfileWriteResult::Failed(format!(
            "DualSense Edge profile write task failed: {error}"
        )),
    }
}

pub(crate) async fn get_edge_profiles(
    Path(id): Path<String>,
    State(state): State<AgentState>,
) -> Result<Json<EdgeProfilesResponse>, StatusCode> {
    let (detail, store, to_save) = {
        let mut inner = state.inner.write().await;
        let detail = inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        let snapshot = if let Some(store) = inner.edge_profiles.remove(&id) {
            inner.edge_profiles.insert(id.clone(), store.normalized());
            build_persist_snapshot(&inner)
        } else {
            None
        };
        (detail, inner.edge_profiles.get(&id).cloned(), snapshot)
    };
    persist_snapshot(&state, to_save).await;

    if detail.model != "DualSense Edge" {
        return Ok(Json(EdgeProfilesResponse::for_controller(
            &detail,
            store.as_ref(),
        )));
    }

    let response = match read_edge_profiles_from_hardware(&state, &id).await {
        Ok(hardware) => EdgeProfilesResponse::for_controller_with_hardware(
            &detail,
            store.as_ref(),
            Some(&hardware.profiles),
            None,
            hardware.hardware_writes_enabled,
        ),
        Err(error) => EdgeProfilesResponse::for_controller_with_hardware(
            &detail,
            store.as_ref(),
            None,
            Some(error),
            false,
        ),
    };
    Ok(Json(response))
}

pub(crate) async fn write_edge_profile(
    Path((id, slot)): Path<(String, String)>,
    State(state): State<AgentState>,
    Json(request): Json<UpdateEdgeProfileRequest>,
) -> Result<(StatusCode, Json<ActionAccepted>), StatusCode> {
    let Some(slot_id) = edge_slot_id_from_api(&slot).filter(|slot| slot.assignable()) else {
        return Ok((
            StatusCode::CONFLICT,
            Json(ActionAccepted {
                accepted: false,
                message: "Only Fn + Circle, Fn + Cross, and Fn + Square are editable Edge slots."
                    .to_string(),
                dry_run: Some(true),
            }),
        ));
    };

    let detail = {
        let inner = state.inner.read().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?
    };
    let response = EdgeProfilesResponse::for_controller(&detail, None);
    if response.support_state == EdgeProfileSupportState::Unsupported {
        return Ok((
            StatusCode::CONFLICT,
            Json(ActionAccepted {
                accepted: false,
                message: format!(
                    "Edge onboard profile slot {slot} was not written. {}",
                    response.warning
                ),
                dry_run: Some(true),
            }),
        ));
    }

    let mut config = edge_profile_config_from_request(request);
    let hardware_profile = edge_profile_from_slot_config(slot_id, &config);
    let write_result = write_edge_profile_to_hardware(&state, &id, hardware_profile).await;
    let (status, message, dry_run, level) = match write_result {
        EdgeHardwareProfileWriteResult::Written => {
            config.hardware_synced = true;
            (
                StatusCode::ACCEPTED,
                format!("Wrote Edge slot {slot} to controller memory for {id}"),
                false,
                "info",
            )
        }
        EdgeHardwareProfileWriteResult::StagedOnly(reason) => {
            config.hardware_synced = false;
            (
                StatusCode::ACCEPTED,
                format!("Staged Edge slot {slot} for controller {id}; no hardware write was attempted: {reason}"),
                true,
                "info",
            )
        }
        EdgeHardwareProfileWriteResult::Failed(error) => {
            config.hardware_synced = false;
            (
                StatusCode::CONFLICT,
                format!("Staged Edge slot {slot} locally, but the hardware write failed: {error}"),
                false,
                "warn",
            )
        }
    };

    let to_save = {
        let mut inner = state.inner.write().await;
        inner.controllers.detail(&id).ok_or(StatusCode::NOT_FOUND)?;
        inner
            .edge_profiles
            .entry(id.clone())
            .or_default()
            .slots
            .insert(slot.clone(), config);
        inner.push_log(LogEntry {
            level: level.to_string(),
            message: message.clone(),
            timestamp: current_timestamp(),
        });
        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;

    Ok((
        status,
        Json(ActionAccepted {
            accepted: status == StatusCode::ACCEPTED,
            message,
            dry_run: Some(dry_run),
        }),
    ))
}
