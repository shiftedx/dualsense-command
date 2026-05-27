use super::*;

#[test]
fn decodes_classic_tray_messages() {
    assert_eq!(
        tray_icon_action(TRAY_ICON_ID as WPARAM, WM_RBUTTONUP as LPARAM),
        Some(TrayIconAction::ShowMenu)
    );
    assert_eq!(
        tray_icon_action(TRAY_ICON_ID as WPARAM, WM_LBUTTONUP as LPARAM),
        Some(TrayIconAction::OpenUi)
    );
}

#[test]
fn decodes_notifyicon_version_4_tray_messages() {
    let context_menu = ((TRAY_ICON_ID as usize) << 16) | WM_CONTEXTMENU as usize;
    let keyboard_select = ((TRAY_ICON_ID as usize) << 16) | NIN_KEYSELECT as usize;

    assert_eq!(
        tray_icon_action(0, context_menu as LPARAM),
        Some(TrayIconAction::ShowMenu)
    );
    assert_eq!(
        tray_icon_action(0, keyboard_select as LPARAM),
        Some(TrayIconAction::OpenUi)
    );
}

#[test]
fn ignores_messages_for_other_icons() {
    let other_icon = (((TRAY_ICON_ID + 1) as usize) << 16) | WM_CONTEXTMENU as usize;

    assert_eq!(tray_icon_action(0, other_icon as LPARAM), None);
    assert_eq!(tray_icon_action(999, WM_RBUTTONUP as LPARAM), None);
}

#[test]
fn tray_state_debounces_duplicate_open_ui_requests() {
    let (health_refresh_tx, _health_refresh_rx) = mpsc::sync_channel(1);
    let mut state = TrayState {
        agent: None,
        install_dir: PathBuf::new(),
        last_open_ui: None,
        health_cache: Arc::new(Mutex::new(TrayHealthCache {
            summary: refreshing_health_summary(),
            refreshed_at: Instant::now(),
        })),
        health_refresh_tx,
    };

    assert!(state.claim_open_ui(DASHBOARD_URL));
    assert!(!state.claim_open_ui(DASHBOARD_URL));
    assert!(state.claim_open_ui(HAPTICS_URL));
    assert!(state.claim_open_ui(BUTTON_MAPPING_URL));

    state.last_open_ui = Some((
        Instant::now() - Duration::from_millis(OPEN_UI_DEBOUNCE_MS + 1),
        BUTTON_MAPPING_URL.to_string(),
    ));
    assert!(state.claim_open_ui(BUTTON_MAPPING_URL));
}

#[test]
fn tray_agent_launch_grants_lan_toggle_capability() {
    let mut command = Command::new("dscc-agent.exe");
    configure_agent_command(
        &mut command,
        Path::new("."),
        PathBuf::from("web").join("dist"),
    );

    let lan_env = command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new(LAN_API_ENABLE_ENV))
        .and_then(|(_, value)| value);

    assert_eq!(lan_env, Some(OsStr::new("1")));
}

#[test]
fn tray_menu_exposes_useful_actions_and_agent_state() {
    assert!(DASHBOARD_URL.ends_with("#/games"));
    assert!(HAPTICS_URL.ends_with("#/adaptive-triggers-haptics"));
    assert!(BUTTON_MAPPING_URL.ends_with("#/button-mapping"));

    let running_summary = TrayHealthSummary {
        agent_running: true,
        agent_label: "Agent Online".to_string(),
        agent_detail: "v0.1.9 - local runtime ready".to_string(),
        agent_accent: TrayMenuAccent::Ready,
        profile_label: "Profile: Base".to_string(),
        profile_detail: "forza-horizon".to_string(),
        profile_accent: TrayMenuAccent::Ready,
        controller_label: "Controller: Edge".to_string(),
        controller_detail: "DualSense Edge / Bluetooth".to_string(),
        controller_accent: TrayMenuAccent::Ready,
        diagnostics_label: "Diagnostics Clear".to_string(),
        diagnostics_detail: "7 checks healthy".to_string(),
        diagnostics_accent: TrayMenuAccent::Ready,
    };
    let running = tray_menu_entries(&running_summary, true);
    assert!(running
        .iter()
        .any(|entry| entry.command == CMD_OPEN_BUTTON_MAPPING && !entry.disabled));
    assert!(running
        .iter()
        .any(|entry| entry.descriptor.label == "Agent Online"
            && entry.descriptor.kind == TrayMenuKind::Readout));
    assert!(running
        .iter()
        .any(|entry| entry.descriptor.label == "Profile: Base"
            && entry.descriptor.detail == "forza-horizon"));
    assert!(running.iter().any(|entry| {
        entry.descriptor.label == "Controller: Edge"
            && entry.descriptor.detail == "DualSense Edge / Bluetooth"
    }));
    assert!(running
        .iter()
        .any(|entry| entry.descriptor.label == "Diagnostics Clear"
            && entry.descriptor.kind == TrayMenuKind::Readout));
    assert!(running
        .iter()
        .any(|entry| { entry.command == CMD_OPEN_UI && entry.descriptor.label == "Dashboard" }));
    assert!(running.iter().any(|entry| {
        entry.command == CMD_OPEN_HAPTICS && entry.descriptor.label == "Triggers & Haptics"
    }));
    assert!(running
        .iter()
        .all(|entry| entry.descriptor.label != "Diagnostics Waiting"));
    assert!(running
        .iter()
        .all(|entry| !entry.descriptor.label.contains("JSON")));
    assert!(running.iter().all(|entry| {
        !matches!(
            entry.descriptor.label.as_str(),
            "Open Install Folder" | "Open Config Folder"
        )
    }));
    assert!(running.iter().all(|entry| entry.command != CMD_START));
    assert!(running
        .iter()
        .any(|entry| entry.command == CMD_STOP && !entry.disabled));
    assert!(running
        .iter()
        .any(|entry| entry.command == CMD_CHECK_UPDATES && !entry.disabled));
    assert!(tray_menu_height(&running) < 400);

    let offline_summary = TrayHealthSummary {
        agent_running: false,
        agent_label: "Agent Offline".to_string(),
        agent_detail: "Start the agent to enable controller control".to_string(),
        agent_accent: TrayMenuAccent::Danger,
        profile_label: "Profile Unavailable".to_string(),
        profile_detail: "Start the agent to read profile state".to_string(),
        profile_accent: TrayMenuAccent::Neutral,
        controller_label: "Controller Unavailable".to_string(),
        controller_detail: "Start the agent to read controller state".to_string(),
        controller_accent: TrayMenuAccent::Neutral,
        diagnostics_label: "Diagnostics Unavailable".to_string(),
        diagnostics_detail: "Waiting for the local runtime".to_string(),
        diagnostics_accent: TrayMenuAccent::Neutral,
    };
    let offline = tray_menu_entries(&offline_summary, false);
    assert!(offline
        .iter()
        .any(|entry| entry.command == CMD_START && !entry.disabled));
    assert!(offline.iter().all(|entry| entry.command != CMD_STOP));
    assert!(tray_menu_height(&offline) < 370);

    let external = tray_menu_entries(&running_summary, false);
    assert!(external.iter().all(|entry| entry.command != CMD_STOP));
    assert!(external.iter().all(|entry| entry.command != CMD_RESTART));
    assert!(external.iter().all(|entry| entry.command != CMD_START));
    assert!(external
        .iter()
        .any(|entry| { entry.command == CMD_QUIT && entry.descriptor.detail == "Close tray" }));
}

#[test]
fn tray_snapshot_summary_reads_active_profile_and_diagnostics() {
    let snapshot = serde_json::from_str::<TraySnapshotDto>(
        r#"{
            "status":{
                "version":"0.3.2",
                "healthy":true,
                "active_profile_id":"forza-horizon",
                "active_adapter_id":null
            },
            "profiles":[
                {"id":"forza-horizon","name":"Base","built_in":true,"active":true},
                {"id":"forza-horizon-immersive","name":"Immersive","built_in":true,"active":false}
            ],
            "controllers":[
                {"id":"controller-0001","name":"Edge","model":"dualsense_edge","transport":"bluetooth","connected":true}
            ],
            "profileResolution":{"controllerId":"controller-0001"},
            "diagnostics":{
                "loopback_only":true,
                "hardware_required":false,
                "checks":[
                    {"name":"agent","status":"ok","detail":"ready"},
                    {"name":"hid","status":"connected","detail":"ready"}
                ]
            }
        }"#,
    )
    .expect("snapshot subset parses");
    let summary = tray_health_summary_from_snapshot(&snapshot);

    assert_eq!(summary.agent_label, "Agent Online");
    assert_eq!(summary.agent_detail, "v0.3.2 - profile ready");
    assert_eq!(summary.profile_label, "Profile: Base");
    assert_eq!(summary.profile_detail, "forza-horizon");
    assert_eq!(summary.controller_label, "Controller: Edge");
    assert_eq!(summary.controller_detail, "DualSense Edge / Bluetooth");
    assert_eq!(summary.diagnostics_label, "Diagnostics Clear");
    assert_eq!(
        fallback_profile_name("forza-horizon-immersive"),
        "Immersive"
    );
}

#[test]
fn bundled_tray_icon_contains_usable_images() {
    assert!(icon_image_from_ico(TRAY_ICON_ICO, 16).is_some());
    assert!(icon_image_from_ico(TRAY_ICON_ICO, 32).is_some());
}
