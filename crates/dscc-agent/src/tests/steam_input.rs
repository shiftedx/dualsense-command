use super::support::*;
use super::*;

#[test]
fn missing_button_assignments_normalize_to_defaults() {
    let mut controller_value = serde_json::to_value(ControllerConfig::default_for(
        "edge-defaults",
        "DualSense Edge",
    ))
    .expect("controller config serializes");
    controller_value
        .as_object_mut()
        .expect("controller config object")
        .remove("buttons");
    let controller_config: ControllerConfig =
        serde_json::from_value(controller_value).expect("controller config deserializes");
    let controller_config = controller_config.normalized();
    assert!(controller_config
        .buttons
        .iter()
        .any(|button| button.key == "Cross" && button.label == "Cross"));
    assert!(controller_config
        .buttons
        .iter()
        .any(|button| button.key == "Back Left" && button.label == "L3"));

    let mut profile_value =
        serde_json::to_value(ProfileConfig::default()).expect("profile config serializes");
    profile_value
        .as_object_mut()
        .expect("profile config object")
        .remove("buttons");
    let profile_config: ProfileConfig =
        serde_json::from_value(profile_value).expect("profile config deserializes");
    let profile_config = profile_config.normalized_for_model("DualSense Edge");
    assert!(profile_config
        .buttons
        .iter()
        .any(|button| button.key == "Cross" && button.label == "Cross"));
    assert!(profile_config
        .buttons
        .iter()
        .any(|button| button.key == "Back Left" && button.label == "L3"));
}

#[test]
fn steam_input_layout_parser_extracts_readable_bindings() {
    let root = FsPath::new("C:/Program Files (x86)/Steam");
    let file = root.join("userdata/123456/1551360/remote/test_controller_config.vdf");
    let layout = parse_steam_input_layout(
        root,
        &file,
        r##""controller_mappings"
{
"title" "Forza Layout"
"controller_type" "controller_ps5"
"group"
{
    "ID" "1"
    "mode" "dpad"
    "inputs"
    {
        "dpad_north"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press UP_ARROW"
                    }
                }
            }
        }
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button LEFT_SHOULDER"
                    }
                }
            }
        }
    }
}
}"##,
    )
    .expect("layout parses");

    assert_eq!(layout.app_id.as_deref(), Some("1551360"));
    assert_eq!(layout.title, "Forza Layout");
    assert_eq!(layout.controller_type.as_deref(), Some("controller_ps5"));
    assert_eq!(layout.controller_label.as_deref(), Some("DualSense"));
    assert!(layout.source.contains("<steam-user>"));
    assert_eq!(layout.bindings[0].input, "D-Pad Up");
    assert_eq!(layout.bindings[0].binding, "Up Arrow Key");
    assert_eq!(layout.bindings[0].kind, "Key");
    assert_eq!(layout.bindings[1].input, "Back Left");
}

#[test]
fn steam_input_layout_parser_keeps_input_id_for_non_full_activators() {
    let root = FsPath::new("C:/Program Files (x86)/Steam");
    let file = root.join("userdata/123456/1551360/remote/test_controller_config.vdf");
    let layout = parse_steam_input_layout(
        root,
        &file,
        r##""controller_mappings"
{
"title" "Forza Layout"
"controller_type" "controller_ps5"
"group"
{
    "ID" "1"
    "mode" "dpad"
    "inputs"
    {
        "dpad_north"
        {
            "activators"
            {
                "Long_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press UP_ARROW"
                    }
                }
            }
        }
    }
}
}"##,
    )
    .expect("layout parses");

    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.bindings[0].input_id, "dpad_north");
    assert_eq!(layout.bindings[0].input, "D-Pad Up");
    assert_eq!(layout.bindings[0].activator.as_deref(), Some("Long Press"));
}

#[test]
fn steam_input_layout_parser_mirrors_fh6_active_sources() {
    let root = FsPath::new("C:/Program Files (x86)/Steam");
    let file = root
        .join("steamapps/common/Steam Controller Configs/123456/config/2483190/controller_ps5.vdf");
    let layout = parse_steam_input_layout(
        root,
        &file,
        r##""controller_mappings"
{
"title" "#Title"
"controller_type" "controller_ps5_edge"
"localization"
{
    "english"
    {
        "title" "Gamepad"
    }
}
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_menu"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button select, , "
                    }
                }
            }
        }
        "button_escape"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button start, , "
                    }
                }
            }
        }
        "button_back_left_upper"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press M, , "
                    }
                }
            }
        }
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press Q, , "
                    }
                }
            }
        }
    }
}
"group"
{
    "id" "14"
    "mode" "2dscroll"
    "inputs"
    {
        "dpad_north"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press EQUALS, , "
                    }
                }
            }
        }
        "dpad_south"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press DASH, , "
                    }
                }
            }
        }
    }
}
"preset"
{
    "id" "0"
    "name" "Default"
    "group_source_bindings"
    {
        "7" "switch active"
        "14" "center_trackpad active"
    }
}
}"##,
    )
    .expect("layout parses");

    let find = |input_id: &str, group_id: &str| {
        layout
            .bindings
            .iter()
            .find(|binding| {
                binding.input_id == input_id && binding.group_id.as_deref() == Some(group_id)
            })
            .expect("binding exists")
    };

    let create = find("button_menu", "7");
    assert_eq!(create.input, "Create");
    assert_eq!(create.binding, "Select");
    let options = find("button_escape", "7");
    assert_eq!(options.input, "Options");
    assert_eq!(options.binding, "Start");
    let fn_left = find("button_back_left_upper", "7");
    assert_eq!(fn_left.binding, "M Key");
    let swipe_up = find("dpad_north", "14");
    assert_eq!(swipe_up.input, "Swipe Up");
    assert_eq!(swipe_up.binding, "= Key");
    assert_eq!(swipe_up.source.as_deref(), Some("Center Trackpad"));
    assert_eq!(swipe_up.source_mode.as_deref(), Some("Directional Swipe"));
    let swipe_down = find("dpad_south", "14");
    assert_eq!(swipe_down.input, "Swipe Down");
    assert_eq!(swipe_down.binding, "- Key");
}

#[test]
fn steam_input_writer_replaces_only_selected_binding() {
    let source = r##""controller_mappings"
{
"title" "Forza Layout"
"revision" "4"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press Q, , "
                    }
                }
            }
        }
        "button_back_right"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press E, , "
                    }
                }
            }
        }
    }
}
}"##;
    let request = SteamInputBindingWriteRequest {
        layout_source:
            "steamapps/common/Steam Controller Configs/123/config/2483190/controller_ps5.vdf"
                .to_string(),
        app_id: Some("2483190".to_string()),
        input_id: "button_back_left".to_string(),
        group_id: Some("7".to_string()),
        activator: Some("Full Press".to_string()),
        raw_binding: "key_press M, , ".to_string(),
        profile_name: Some("Immersive / active".to_string()),
        dry_run: true,
    };

    let updated = replace_steam_binding_value(source, &request, "key_press M, , ")
        .expect("binding can be replaced")
        .expect("source changes");
    let updated = mark_dscc_steam_profile_metadata(&updated, request.profile_name.as_deref());

    assert!(updated.contains(r#""binding" "key_press M, , ""#));
    assert!(updated.contains(r#""binding" "key_press E, , ""#));
    assert!(updated.contains(r#""title" "DSCC / Immersive / active""#));
    assert!(updated.contains(r#""revision" "5""#));
    assert!(!updated.contains(r#""binding" "key_press Q, , ""#));
}

#[test]
fn steam_input_writer_updates_center_trackpad_without_touching_dpad() {
    let source = r##""controller_mappings"
{
"title" "Forza Layout"
"revision" "2"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "9"
    "mode" "dpad"
    "inputs"
    {
        "dpad_north"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button DPAD_UP, , "
                    }
                }
            }
        }
    }
}
"group"
{
    "id" "14"
    "mode" "2dscroll"
    "inputs"
    {
        "dpad_north"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press EQUALS, , "
                    }
                }
            }
        }
    }
}
}"##;
    let request = SteamInputBindingWriteRequest {
        layout_source:
            "steamapps/common/Steam Controller Configs/123/config/2483190/controller_ps5.vdf"
                .to_string(),
        app_id: Some("2483190".to_string()),
        input_id: "dpad_north".to_string(),
        group_id: Some("14".to_string()),
        activator: Some("Full Press".to_string()),
        raw_binding: "key_press TAB, , ".to_string(),
        profile_name: Some("Immersive / active".to_string()),
        dry_run: true,
    };

    let updated = replace_steam_binding_value(source, &request, "key_press TAB, , ")
        .expect("binding can be replaced")
        .expect("source changes");

    assert!(updated.contains(r#""binding" "xinput_button DPAD_UP, , ""#));
    assert!(updated.contains(r#""binding" "key_press TAB, , ""#));
    assert!(!updated.contains(r#""binding" "key_press EQUALS, , ""#));
}

#[test]
fn steam_input_paddle_preset_writes_only_edge_back_paddles_and_creates_backup() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-paddle-preset");
    let layout_dir = root
        .join("steamapps")
        .join("common")
        .join("Steam Controller Configs")
        .join("123456")
        .join("config")
        .join("2483190");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_ps5.vdf");
    let original = r##""controller_mappings"
{
"title" "Forza Layout"
"revision" "5"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_menu"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button select, , "
                    }
                }
            }
        }
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button joystick_left, , "
                    }
                }
            }
        }
        "button_back_right"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "xinput_button joystick_right, , "
                    }
                }
            }
        }
        "button_back_left_upper"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press M, , "
                    }
                }
            }
        }
    }
}
}"##;
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");
    std::env::set_var("DSCC_STEAM_ROOT", &root);

    let response = write_steam_input_paddle_preset(SteamInputPaddlePresetRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        left_key: None,
        right_key: None,
        profile_name: Some("Forza Paddle Shift".to_string()),
        dry_run: false,
    })
    .expect("paddle preset writes");

    assert!(response.accepted);
    assert!(!response.dry_run);
    assert_eq!(response.paddles.len(), 2);
    assert_eq!(response.paddles[0].input_id, "button_back_left");
    assert_eq!(response.paddles[0].key, "Q");
    assert_eq!(response.paddles[0].binding.binding, "Q Key");
    assert_eq!(response.paddles[1].input_id, "button_back_right");
    assert_eq!(response.paddles[1].key, "E");
    assert_eq!(response.paddles[1].binding.binding, "E Key");

    let backup_path = response
        .backup_path
        .as_deref()
        .map(PathBuf::from)
        .expect("backup path is reported");
    assert_eq!(
        fs::read_to_string(&backup_path).expect("backup layout is readable"),
        original
    );
    let updated = fs::read_to_string(&layout_file).expect("updated layout is readable");
    assert!(updated.contains(r#""binding" "key_press Q, , ""#));
    assert!(updated.contains(r#""binding" "key_press E, , ""#));
    assert!(updated.contains(r#""binding" "xinput_button select, , ""#));
    assert!(updated.contains(r#""binding" "key_press M, , ""#));
    assert!(updated.contains(r#""title" "DSCC / Forza Paddle Shift""#));
    assert!(updated.contains(r#""revision" "6""#));
    assert!(!updated.contains(r#""binding" "xinput_button joystick_left, , ""#));
    assert!(!updated.contains(r#""binding" "xinput_button joystick_right, , ""#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_paddle_preset_uses_configurable_keys_in_dry_run() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-paddle-preset-dry");
    let layout_dir = root
        .join("steamapps")
        .join("common")
        .join("Steam Controller Configs")
        .join("123456")
        .join("config")
        .join("2483190");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_ps5.vdf");
    let original = r##""controller_mappings"
{
"title" "Forza Layout"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press Q, , "
                    }
                }
            }
        }
        "button_back_right"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press E, , "
                    }
                }
            }
        }
    }
}
}"##;
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");
    std::env::set_var("DSCC_STEAM_ROOT", &root);

    let response = write_steam_input_paddle_preset(SteamInputPaddlePresetRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        left_key: Some("page up".to_string()),
        right_key: Some("page_down".to_string()),
        profile_name: Some("Forza Paddle Shift".to_string()),
        dry_run: true,
    })
    .expect("paddle preset dry run validates");

    assert!(response.dry_run);
    assert_eq!(response.backup_path, None);
    assert_eq!(response.paddles[0].key, "PAGE_UP");
    assert_eq!(response.paddles[0].binding.binding, "Page Up Key");
    assert_eq!(response.paddles[1].key, "PAGE_DOWN");
    assert_eq!(response.paddles[1].binding.binding, "Page Down Key");
    assert_eq!(
        fs::read_to_string(&layout_file).expect("layout still readable"),
        original
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_paddle_preset_rejects_missing_or_non_edge_bindings_cleanly() {
    let non_edge = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: Some("controller_ps5".to_string()),
        controller_label: Some("DualSense".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 0,
        bindings: Vec::new(),
    };
    let error = ensure_dualsense_edge_steam_layout(&non_edge)
        .expect_err("non-Edge layout should be rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("DualSense Edge"),
        "unexpected message: {}",
        error.message
    );

    let inferred_edge = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: None,
        controller_label: Some("DualSense Edge".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 2,
        bindings: vec![
            SteamInputBinding {
                input: "Back Left".to_string(),
                input_id: "button_back_left".to_string(),
                binding: "L3".to_string(),
                raw_binding: "xinput_button joystick_left, , ".to_string(),
                kind: "Gamepad".to_string(),
                source: Some("Switches".to_string()),
                source_mode: Some("Switches".to_string()),
                activator: Some("Full Press".to_string()),
                group_id: Some("7".to_string()),
            },
            SteamInputBinding {
                input: "Back Right".to_string(),
                input_id: "button_back_right".to_string(),
                binding: "R3".to_string(),
                raw_binding: "xinput_button joystick_right, , ".to_string(),
                kind: "Gamepad".to_string(),
                source: Some("Switches".to_string()),
                source_mode: Some("Switches".to_string()),
                activator: Some("Full Press".to_string()),
                group_id: Some("7".to_string()),
            },
        ],
    };
    ensure_dualsense_edge_steam_layout(&inferred_edge)
        .expect("layouts with both Edge back paddles are accepted");

    let edge_missing_right = SteamInputLayout {
        app_id: Some("2483190".to_string()),
        title: "Forza Layout".to_string(),
        controller_type: Some("controller_ps5_edge".to_string()),
        controller_label: Some("DualSense Edge".to_string()),
        source: "controller_ps5.vdf".to_string(),
        binding_count: 1,
        bindings: vec![SteamInputBinding {
            input: "Back Left".to_string(),
            input_id: "button_back_left".to_string(),
            binding: "Q Key".to_string(),
            raw_binding: "key_press Q, , ".to_string(),
            kind: "Key".to_string(),
            source: Some("Switches".to_string()),
            source_mode: Some("Switches".to_string()),
            activator: Some("Full Press".to_string()),
            group_id: Some("7".to_string()),
        }],
    };
    let error = steam_edge_paddle_binding(&edge_missing_right, STEAM_EDGE_BACK_RIGHT_INPUT_ID)
        .expect_err("missing right paddle should be rejected");
    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert!(
        error.message.contains("Back Right"),
        "unexpected message: {}",
        error.message
    );
}

#[test]
fn steam_input_writer_dry_run_uses_temp_steam_root_without_writing() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-input-test");
    let layout_dir = root
        .join("steamapps")
        .join("common")
        .join("Steam Controller Configs")
        .join("123456")
        .join("config")
        .join("2483190");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_ps5.vdf");
    let original = r##""controller_mappings"
{
"title" "Gamepad"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press Q, , "
                    }
                }
            }
        }
    }
}
}"##;
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");

    std::env::set_var("DSCC_STEAM_ROOT", &root);
    let response = write_steam_input_binding(SteamInputBindingWriteRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        input_id: "button_back_left".to_string(),
        group_id: Some("7".to_string()),
        activator: Some("Full Press".to_string()),
        raw_binding: "key_press M".to_string(),
        profile_name: Some("Base".to_string()),
        dry_run: true,
    })
    .expect("dry run succeeds");

    assert!(response.accepted);
    assert!(response.dry_run);
    assert_eq!(response.backup_path, None);
    assert_eq!(response.binding.binding, "M Key");
    assert_eq!(
        fs::read_to_string(&layout_file).expect("layout still readable"),
        original
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_writer_creates_backup_before_writing() {
    let _env = TestEnv::new(&["DSCC_STEAM_ROOT"]);
    let root = temp_test_dir("dscc-steam-input-write-test");
    let layout_dir = root
        .join("steamapps")
        .join("common")
        .join("Steam Controller Configs")
        .join("123456")
        .join("config")
        .join("2483190");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_ps5.vdf");
    let original = r##""controller_mappings"
{
"title" "Gamepad"
"revision" "1"
"controller_type" "controller_ps5_edge"
"group"
{
    "id" "7"
    "mode" "switches"
    "inputs"
    {
        "button_back_left"
        {
            "activators"
            {
                "Full_Press"
                {
                    "bindings"
                    {
                        "binding" "key_press Q, , "
                    }
                }
            }
        }
    }
}
}"##;
    fs::write(&layout_file, original).expect("layout fixture");
    let source = sanitized_steam_path(&root, &layout_file).expect("sanitized source");

    std::env::set_var("DSCC_STEAM_ROOT", &root);
    let response = write_steam_input_binding(SteamInputBindingWriteRequest {
        layout_source: source,
        app_id: Some("2483190".to_string()),
        input_id: "button_back_left".to_string(),
        group_id: Some("7".to_string()),
        activator: Some("Full Press".to_string()),
        raw_binding: "key_press M".to_string(),
        profile_name: Some("Base".to_string()),
        dry_run: false,
    })
    .expect("write succeeds");

    assert!(response.accepted);
    assert!(!response.dry_run);
    assert_eq!(response.binding.binding, "M Key");
    let backup_path = response
        .backup_path
        .as_deref()
        .map(PathBuf::from)
        .expect("backup path is reported");
    assert_eq!(
        fs::read_to_string(&backup_path).expect("backup layout is readable"),
        original
    );
    let updated = fs::read_to_string(&layout_file).expect("updated layout is readable");
    assert!(updated.contains(r#""binding" "key_press M, , ""#));
    assert!(updated.contains(r#""title" "DSCC / Base""#));
    assert!(updated.contains(r#""revision" "2""#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_writer_rejects_layouts_outside_steam_root() {
    let root = temp_test_dir("dscc-steam-root-test");
    let outside_root = temp_test_dir("dscc-steam-outside-test");
    fs::create_dir_all(&root).expect("steam root fixture");
    fs::create_dir_all(&outside_root).expect("outside fixture");
    let outside_file = outside_root.join("controller_ps5.vdf");
    fs::write(&outside_file, "\"controller_mappings\"\n{}").expect("outside layout fixture");

    let error = validated_steam_input_layout_path(root.clone(), outside_file)
        .expect_err("outside layout should be rejected");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("inside the Steam install path"),
        "unexpected message: {}",
        error.message
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside_root);
}

#[test]
fn steam_input_writer_rejects_non_controller_layout_names() {
    let root = temp_test_dir("dscc-steam-name-test");
    let layout_dir = root.join("userdata").join("123456").join("config");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_base.vdf");
    fs::write(&layout_file, "\"controller_mappings\"\n{}").expect("layout fixture");

    let error = validated_steam_input_layout_path(root.clone(), layout_file)
        .expect_err("base layout should be rejected");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("controller_*.vdf"),
        "unexpected message: {}",
        error.message
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn steam_input_writer_rejects_layouts_over_guarded_size_limit() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let root = temp_test_dir("dscc-steam-large-test");
    let layout_dir = root
        .join("userdata")
        .join("123456")
        .join("2483190")
        .join("remote");
    fs::create_dir_all(&layout_dir).expect("layout fixture directory");
    let layout_file = layout_dir.join("controller_ps5.vdf");
    fs::write(&layout_file, vec![b'a'; 256 * 1024 + 1]).expect("large layout fixture");

    std::env::set_var("DSCC_STEAM_ROOT", &root);
    std::env::set_var("ProgramFiles(x86)", root.join("missing-pf86"));
    std::env::set_var("ProgramFiles", root.join("missing-pf"));
    std::env::set_var("LOCALAPPDATA", root.join("missing-local-app-data"));
    let error = write_steam_input_binding(SteamInputBindingWriteRequest {
        layout_source: layout_file.display().to_string(),
        app_id: Some("2483190".to_string()),
        input_id: "button_back_left".to_string(),
        group_id: None,
        activator: None,
        raw_binding: "key_press M".to_string(),
        profile_name: None,
        dry_run: false,
    })
    .expect_err("large layout should be rejected");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("guarded write limit"),
        "unexpected message: {}",
        error.message
    );
    assert!(
        fs::read_dir(&layout_dir)
            .expect("layout directory is readable")
            .all(|entry| entry
                .expect("layout entry is readable")
                .file_name()
                .to_string_lossy()
                == "controller_ps5.vdf"),
        "large rejected layout should not create backups"
    );
    let _ = fs::remove_dir_all(root);
}
