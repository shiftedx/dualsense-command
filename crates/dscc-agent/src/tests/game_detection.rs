use super::support::*;
use super::*;

#[test]
fn steam_libraryfolders_parser_discovers_primary_and_secondary_libraries() {
    let libraries = parse_steam_library_folders(include_str!(
        "../../tests/fixtures/steam/libraryfolders_fh5_fh6.vdf"
    ));

    assert!(libraries.contains(&PathBuf::from("C:\\Program Files (x86)\\Steam")));
    assert!(libraries.contains(&PathBuf::from("D:\\SteamLibrary")));
}

#[test]
fn steam_appmanifests_match_fh5_and_fh6_supported_games() {
    let primary = FsPath::new("C:/Program Files (x86)/Steam");
    let secondary = FsPath::new("D:/SteamLibrary");
    let fh5_manifest = parse_steam_app_manifest(
        primary,
        include_str!("../../tests/fixtures/steam/appmanifest_1551360.acf"),
    )
    .expect("FH5 manifest parses");
    let fh6_manifest = parse_steam_app_manifest(
        secondary,
        include_str!("../../tests/fixtures/steam/appmanifest_2483190.acf"),
    )
    .expect("FH6 manifest parses");

    let catalog = build_supported_steam_game_catalog(
        primary,
        &[primary.to_path_buf(), secondary.to_path_buf()],
        &[fh5_manifest, fh6_manifest],
    );

    let fh5 = catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == "forza-horizon-5")
        .expect("FH5 is discovered from Steam appmanifest");
    assert_eq!(fh5.app_id.as_deref(), Some(FORZA_HORIZON5_STEAM_APP_ID));
    assert!(fh5.install_path.as_deref().is_some_and(|path| {
        path.ends_with("steamapps\\common\\ForzaHorizon5")
            || path.ends_with("steamapps/common/ForzaHorizon5")
    }));

    let fh6 = catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == "forza-horizon-6")
        .expect("FH6 is discovered from Steam appmanifest");
    assert_eq!(fh6.app_id.as_deref(), Some(FORZA_HORIZON6_STEAM_APP_ID));
    assert_eq!(fh6.support_level, "telemetry");
}

#[test]
fn steam_appmanifest_matches_assetto_corsa_rally_supported_game() {
    let secondary = FsPath::new("D:/SteamLibrary");
    let acr_manifest = parse_steam_app_manifest(
        secondary,
        r#""AppState"
{
"appid"     "3917090"
"name"      "Assetto Corsa Rally"
"StateFlags"    "4"
"installdir"    "Assetto Corsa Rally"
}"#,
    )
    .expect("ACR manifest parses");

    let catalog =
        build_supported_steam_game_catalog(secondary, &[secondary.to_path_buf()], &[acr_manifest]);

    let acr = catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == "assetto-corsa-rally")
        .expect("Assetto Corsa Rally is discovered from Steam appmanifest");
    assert_eq!(acr.app_id.as_deref(), Some("3917090"));
    assert!(acr.install_path.as_deref().is_some_and(|path| {
        path.ends_with("steamapps\\common\\Assetto Corsa Rally")
            || path.ends_with("steamapps/common/Assetto Corsa Rally")
    }));
    assert_eq!(acr.support_level, "telemetry");
}

#[test]
fn built_in_game_modules_have_unique_game_ids_and_non_empty_core_ids() {
    let mut game_ids = std::collections::BTreeSet::new();
    let built_in_module_ids: std::collections::BTreeSet<_> =
        built_in_adapters().iter().map(|module| module.id).collect();
    let built_in_profile_ids: std::collections::BTreeSet<_> = default_profiles()
        .iter()
        .map(|profile| profile.id.clone())
        .collect();

    for game in built_in_game_modules() {
        assert!(
            !game.id.trim().is_empty(),
            "built-in game id must not be empty for {game:?}"
        );
        assert!(
            game_ids.insert(game.id),
            "duplicate built-in game id: {}",
            game.id
        );
        assert!(
            !game.adapter_id.trim().is_empty(),
            "module id must not be empty for {}",
            game.id
        );
        assert!(
            built_in_module_ids.contains(game.adapter_id),
            "{} references unknown module id {}",
            game.id,
            game.adapter_id
        );
        assert!(
            !game.default_profile_id.trim().is_empty(),
            "default profile id must not be empty for {}",
            game.id
        );
        assert!(
            built_in_profile_ids.contains(game.default_profile_id),
            "{} references unknown default profile id {}",
            game.id,
            game.default_profile_id
        );
    }
}

#[test]
fn every_built_in_game_has_detection_metadata() {
    for game in built_in_game_modules() {
        assert!(
            !game.display_name.trim().is_empty(),
            "game name must not be empty for {}",
            game.id
        );
        assert!(
            !game.process_names.is_empty(),
            "{} must declare at least one process name",
            game.id
        );

        for process_name in game.process_names {
            assert!(
                !process_name.trim().is_empty(),
                "{} contains an empty process name",
                game.id
            );
            let detection = detect_running_game_from_processes([*process_name]);
            assert_eq!(
                detection.active_game_id.as_deref(),
                Some(game.id),
                "{} should detect from process {}",
                game.id,
                process_name
            );
            assert_eq!(
                detection.module_id.as_deref(),
                Some(game.id),
                "{} should detect game module {}",
                game.id,
                game.id
            );
            assert_eq!(
                detection.adapter_id.as_deref(),
                Some(game.adapter_id),
                "{} should detect adapter {}",
                game.id,
                game.adapter_id
            );
            assert_eq!(
                detection.profile_id.as_deref(),
                Some(game.default_profile_id),
                "{} should detect default profile {}",
                game.id,
                game.default_profile_id
            );
            assert_eq!(detection.candidates.len(), 1);
        }
    }
}

#[test]
fn forza_games_are_distinct_game_modules_sharing_forza_data_out() {
    let forza_games: Vec<_> = built_in_game_modules()
        .iter()
        .filter(|game| game.adapter_id == FORZA_DATA_OUT_ADAPTER_ID)
        .collect();
    let forza_game_ids: std::collections::BTreeSet<_> =
        forza_games.iter().map(|game| game.id).collect();

    assert!(forza_game_ids.contains("forza-horizon-5"));
    assert!(forza_game_ids.contains("forza-horizon-6"));
    assert!(
        forza_games.len() >= 2,
        "Forza titles should stay separate game entries"
    );
    assert_eq!(
        forza_game_ids.len(),
        forza_games.len(),
        "Forza titles must have distinct game ids"
    );
    assert!(
        forza_games
            .iter()
            .all(|game| game.adapter_id == FORZA_DATA_OUT_ADAPTER_ID),
        "Forza titles should share the Forza Data Out adapter id"
    );

    let fh5 = detect_running_game_from_processes(["ForzaHorizon5.exe"]);
    let fh6 = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    assert_ne!(fh5.active_game_id, fh6.active_game_id);
    assert_eq!(fh5.module_id.as_deref(), Some("forza-horizon-5"));
    assert_eq!(fh6.module_id.as_deref(), Some("forza-horizon-6"));
    assert_eq!(fh5.adapter_id.as_deref(), Some(FORZA_DATA_OUT_ADAPTER_ID));
    assert_eq!(fh6.adapter_id.as_deref(), Some(FORZA_DATA_OUT_ADAPTER_ID));
}

#[test]
fn steam_local_stats_parser_extracts_playtime_and_achievements() {
    let local_stats = parse_steam_localconfig_stats(
        r#""UserLocalConfigStore"
{
"Software"
{
    "Valve"
    {
        "Steam"
        {
            "apps"
            {
                "2483190"
                {
                    "LastPlayed" "1779141250"
                    "Playtime" "843"
                }
            }
        }
    }
}
}"#,
    );
    let game_stats = local_stats
        .get(FORZA_HORIZON6_STEAM_APP_ID)
        .expect("FH6 playtime parsed");
    assert_eq!(game_stats.playtime_minutes, Some(843));
    assert_eq!(game_stats.last_played_unix, Some(1779141250));

    let achievements = parse_steam_achievement_progress_cache(
        r#"{"mapCache":[[2483190,{"appid":2483190,"unlocked":29,"total":57}]]}"#,
    );
    assert_eq!(
        achievements.get(FORZA_HORIZON6_STEAM_APP_ID),
        Some(&SteamAchievementStats {
            unlocked: 29,
            total: 57
        })
    );
}

#[test]
fn steam_game_artwork_uses_local_route_urls_for_discovered_cache_files() {
    let steam_root = std::env::temp_dir().join(format!(
        "dscc-agent-steam-art-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let app_cache = steam_root
        .join("appcache")
        .join("librarycache")
        .join(FORZA_HORIZON6_STEAM_APP_ID);
    fs::create_dir_all(app_cache.join("hash-header")).unwrap();
    fs::create_dir_all(app_cache.join("hash-hero")).unwrap();
    fs::create_dir_all(app_cache.join("hash-capsule")).unwrap();
    fs::write(
        app_cache.join("hash-header").join("library_header.jpg"),
        [1_u8, 2, 3],
    )
    .unwrap();
    fs::write(
        app_cache.join("hash-hero").join("library_hero.jpg"),
        [1_u8, 2, 3],
    )
    .unwrap();
    fs::write(
        app_cache.join("hash-capsule").join("library_capsule.jpg"),
        [1_u8, 2, 3],
    )
    .unwrap();

    let manifest = SteamAppManifest {
        app_id: FORZA_HORIZON6_STEAM_APP_ID.to_string(),
        name: "Forza Horizon 6".to_string(),
        install_dir: "ForzaHorizon6".to_string(),
        install_path: steam_root
            .join("steamapps")
            .join("common")
            .join("ForzaHorizon6"),
    };
    let catalog = build_supported_steam_game_catalog(
        &steam_root,
        std::slice::from_ref(&steam_root),
        &[manifest],
    );
    let fh6 = catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == "forza-horizon-6")
        .expect("FH6 is present in supported games");

    assert_eq!(
        fh6.artwork.banner_url.as_deref(),
        Some("/api/games/art/forza-horizon-6/banner")
    );
    assert_eq!(
        fh6.artwork.hero_url.as_deref(),
        Some("/api/games/art/forza-horizon-6/hero")
    );
    assert_eq!(
        fh6.artwork.capsule_url.as_deref(),
        Some("/api/games/art/forza-horizon-6/capsule")
    );
    assert_eq!(
        fh6.artwork.icon_url.as_deref(),
        Some("/api/games/art/forza-horizon-6/capsule")
    );
    assert!(catalog
        .artwork_paths
        .contains_key(&("forza-horizon-6".to_string(), "banner".to_string())));
}

#[test]
fn game_detection_is_enriched_with_supported_steam_game_selection() {
    let fh5 = test_game_module_by_id("forza-horizon-5");
    let catalog = SteamGameCatalog {
        supported_games: vec![supported_game_summary(
            fh5,
            Some(FORZA_HORIZON5_STEAM_APP_ID.to_string()),
            Some(PathBuf::from(
                "D:/SteamLibrary/steamapps/common/ForzaHorizon5",
            )),
            GameArtwork {
                banner_url: Some("/api/games/art/forza-horizon-5/banner".to_string()),
                ..GameArtwork::default()
            },
            SteamGameStats::default(),
        )],
        artwork_paths: BTreeMap::new(),
    };

    let detection = detect_running_game_from_processes(["ForzaHorizon5.exe"]);
    let enriched = enrich_game_detection(detection, &catalog);

    assert_eq!(enriched.active_game_id.as_deref(), Some("forza-horizon-5"));
    assert_eq!(
        enriched
            .selected_game
            .as_ref()
            .map(|game| game.game_id.as_str()),
        Some("forza-horizon-5")
    );
    assert!(enriched
        .supported_games
        .iter()
        .any(|game| game.game_id == "forza-horizon-5" && game.running));
}

#[test]
fn installed_supported_games_do_not_become_selected_without_detection() {
    let fh6 = test_game_module_by_id("forza-horizon-6");
    let catalog = SteamGameCatalog {
        supported_games: vec![SupportedGameSummary {
            installed: true,
            ..supported_game_summary(
                fh6,
                Some(FORZA_HORIZON6_STEAM_APP_ID.to_string()),
                None,
                GameArtwork::default(),
                SteamGameStats::default(),
            )
        }],
        artwork_paths: BTreeMap::new(),
    };

    let enriched = enrich_game_detection(no_game_detection("none"), &catalog);

    assert!(enriched.active_game_id.is_none());
    assert!(enriched.selected_game.is_none());
    assert_eq!(enriched.supported_games.len(), 1);
    assert!(!enriched.supported_games[0].running);
}
#[tokio::test]
async fn process_detection_maps_forza_to_edge_profile() {
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
    assert_eq!(detection.profile_id.as_deref(), Some(IMMERSIVE_PROFILE_ID));
}

#[test]
fn unix_process_scan_extracts_proton_windows_executable_names() {
    let names = parse_unix_process_names(
        "ForzaHorizon6.e /home/user/.steam/steamapps/common/ForzaHorizon6/ForzaHorizon6.exe -windowed\n\
         pressure-vessel pressure-vessel-wrap C:\\\\SteamLibrary\\\\ForzaHorizon5\\\\ForzaHorizon5.exe",
    );

    assert!(names.iter().any(|name| name == "ForzaHorizon6.exe"));
    assert!(names.iter().any(|name| name == "ForzaHorizon5.exe"));

    let detection = detect_running_game_from_processes(names.iter().map(String::as_str));
    assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
}

#[test]
fn telemetry_source_detection_recovers_forza_when_process_scan_misses_proton() {
    let state = AgentState::from_controller_events([attach_event(
        "linux-dualsense",
        ControllerFamily::DualSense,
        ControllerTransportKind::Usb,
        Some(25),
    )]);
    {
        let mut inner = state.inner.blocking_write();
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_packet(324, 1);
        inner.active_adapter_id = Some(FORZA_DATA_OUT_ADAPTER_ID.to_string());
        inner.telemetry = SignalSnapshot::from_updates([
            signal_update("source.id", FORZA_DATA_OUT_ADAPTER_ID),
            signal_update("game.state", "driving"),
            signal_update("input.brake", 0.20),
            signal_update("input.throttle", 0.45),
            signal_update("vehicle.speed_kmh", 80.0),
            signal_update("drivetrain.shift_event", "none"),
        ]);
    }

    let inner = state.inner.blocking_read();
    let detection = telemetry_game_detection(&inner, &SteamGameCatalog::default())
        .expect("live Forza Data Out packets should recover game detection");

    assert_eq!(detection.source, "telemetry_source");
    assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-6"));
    assert_eq!(
        detection.adapter_id.as_deref(),
        Some(FORZA_DATA_OUT_ADAPTER_ID)
    );
    assert_eq!(detection.profile_id.as_deref(), Some(IMMERSIVE_PROFILE_ID));

    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
    assert!(hardware_output_runtime_allowed_for_resolution(
        &inner,
        Some(&detection),
        &resolution
    ));
    assert!(support_telemetry_summary(&inner, None).live);
}

#[test]
fn global_profile_override_does_not_block_detected_supported_game() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-forza",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]);
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    {
        let mut inner = state.inner.blocking_write();
        inner.profile_overrides.insert(
            profile_override_key(None, None),
            ProfileOverride {
                controller_id: None,
                game_id: None,
                profile_id: DEFAULT_PROFILE_ID.to_string(),
            },
        );
    }

    let inner = state.inner.blocking_read();
    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(resolution.reason, "foreground_game");
    assert_eq!(resolution.override_profile_id, None);
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
}

#[tokio::test]
async fn cached_game_detection_keeps_standard_five_second_cache() {
    let state = AgentState::mock();
    let cached = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    let cached_game_id = cached.active_game_id.clone();
    let cached_at = Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.store(cached, cached_at);
    }

    let detection = state.cached_game_detection().await;
    let refreshed_at = {
        let cache = state.discovery_cache.game_detection.lock().await;
        cache.refreshed_at
    };

    assert_eq!(detection.active_game_id, cached_game_id);
    assert_eq!(refreshed_at, Some(cached_at));
}

#[tokio::test]
async fn cached_hardware_game_detection_refreshes_after_hardware_interval() {
    let state = AgentState::mock();
    let cached = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    let cached_at = Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL - Duration::from_millis(25);

    {
        let mut catalog = state.discovery_cache.steam_game_catalog.lock().await;
        catalog.store(SteamGameCatalog::default(), Instant::now());
    }
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.store(cached, cached_at);
    }

    let _detection = state.cached_hardware_game_detection().await;
    let refreshed_at = {
        let cache = state.discovery_cache.game_detection.lock().await;
        cache.refreshed_at
    };

    assert!(refreshed_at.is_some_and(|refreshed_at| refreshed_at > cached_at));
}

#[tokio::test]
async fn detected_forza_game_materializes_listener_and_profile_resolution() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-forza",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]);
    let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(FORZA_DATA_OUT_ADAPTER_ID)
            .mark_bound("127.0.0.1:5300".parse().unwrap());
    }

    let inner = state.inner.read().await;
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
    let forza = adapters
        .iter()
        .find(|adapter| adapter.id == "forza-data-out")
        .expect("Forza adapter exists");
    assert!(forza.enabled);
    assert_eq!(forza.state, "needs_setup");
    assert!(forza.setup_hint.contains("no Data Out packets"));

    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(
        resolution.active_adapter_id.as_deref(),
        Some("forza-data-out")
    );
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(IMMERSIVE_PROFILE_ID)
    );
    assert_eq!(resolution.reason, "foreground_game");

    let telemetry = materialized_telemetry_response(&inner, Some(&detection));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "game.state" && signal.value == serde_json::json!("awaiting_data_out")
    }));
}

#[tokio::test]
async fn detected_assetto_game_materializes_shared_memory_and_profile_resolution() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-assetto",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]);
    let detection = detect_running_game_from_processes(["acr.exe"]);
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .mark_ready();
    }

    let inner = state.inner.read().await;
    let adapters =
        materialized_adapters(&inner.adapters, &inner.adapter_runtimes, Some(&detection));
    let assetto = adapters
        .iter()
        .find(|adapter| adapter.id == ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        .expect("Assetto shared-memory adapter exists");
    assert!(assetto.enabled);
    assert_eq!(assetto.state, "needs_setup");
    assert!(assetto.setup_hint.contains("shared memory"));

    let resolution = profile_resolution(&inner, Some(&detection));
    assert_eq!(
        resolution.active_adapter_id.as_deref(),
        Some(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
    );
    assert_eq!(
        resolution.selected_profile_id.as_deref(),
        Some(ASSETTO_CORSA_RALLY_PROFILE_ID)
    );
    assert_eq!(resolution.reason, "foreground_game");

    let telemetry = materialized_telemetry_response(&inner, Some(&detection));
    assert!(telemetry.iter().any(|signal| {
        signal.name == "game.state" && signal.value == serde_json::json!("awaiting_shared_memory")
    }));
}

#[tokio::test]
async fn supported_game_detection_writes_only_lightbar_until_telemetry_is_live() {
    let state = AgentState::from_controller_events([attach_event(
        "edge-assetto",
        ControllerFamily::DualSenseEdge,
        ControllerTransportKind::Bluetooth,
        Some(84),
    )]);
    let detection = detect_running_game_from_processes(["acr.exe"]);
    {
        let mut inner = state.inner.write().await;
        inner
            .adapter_runtime_mut(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
            .mark_ready();
    }

    let inner = state.inner.read().await;
    let resolution = profile_resolution(&inner, Some(&detection));
    assert!(!hardware_output_runtime_allowed_for_resolution(
        &inner,
        Some(&detection),
        &resolution
    ));
    assert!(hardware_output_detection_lightbar_allowed_for_resolution(
        &inner,
        Some(&detection),
        &resolution
    ));

    let (controller_id, frame) = state
        .output_frame_for_current_resolution_cached(
            &inner,
            Some(&detection),
            EffectEnginePurpose::Hardware,
        )
        .expect("detection lightbar frame is produced");

    assert_eq!(controller_id, "edge-assetto");
    assert_eq!(frame.l2, TriggerOutput::Off);
    assert_eq!(frame.r2, TriggerOutput::Off);
    assert!(frame.rumble.is_none());
    assert!(frame.player_leds.is_none());
    assert_eq!(
        frame.lightbar,
        Some(LightbarOutput {
            color: RgbColor {
                red: 0xff,
                green: 0x3b,
                blue: 0x30
            },
            brightness: 0.62,
        })
    );

    let preview = current_effect_response(&inner, Some(&detection), false);
    assert_eq!(preview.output.l2, TriggerOutput::Off);
    assert_eq!(preview.output.r2, TriggerOutput::Off);
    assert!(preview.output.rumble.is_none());
    assert_eq!(preview.output.lightbar, frame.lightbar);
}

#[test]
fn assetto_shared_memory_prefix_normalizes_racing_signals() {
    let mut physics = vec![0_u8; ASSETTO_PHYSICS_MIN_LEN];
    write_i32(&mut physics, 0, 27);
    write_f32(&mut physics, 4, 0.65);
    write_f32(&mut physics, 8, 0.25);
    write_i32(&mut physics, 16, 4);
    write_i32(&mut physics, 20, 6_300);
    write_f32(&mut physics, 24, 0.30);
    write_f32(&mut physics, 28, 102.0);
    write_f32(&mut physics, 44, 0.20);
    write_f32(&mut physics, 48, 0.35);
    write_f32(&mut physics, 52, 0.10);
    write_f32(&mut physics, 56, 0.11);
    write_f32(&mut physics, 60, 0.18);
    write_f32(&mut physics, 64, 0.42);
    write_f32(&mut physics, 68, 0.36);

    let mut graphics = vec![0_u8; ASSETTO_GRAPHICS_MIN_LEN];
    write_i32(&mut graphics, 4, ASSETTO_AC_LIVE);

    let mut static_page = vec![0_u8; ASSETTO_STATIC_MIN_LEN];
    write_i32(&mut static_page, ASSETTO_STATIC_MAX_RPM_OFFSET, 9_000);

    let (_, updates) = parse_assetto_shared_memory_pages(
        AssettoSharedMemoryPages {
            physics: &physics,
            graphics: Some(&graphics),
            static_page: Some(&static_page),
        },
        42,
    )
    .expect("Assetto shared-memory prefix parses");
    let snapshot = SignalSnapshot::from_updates(updates);

    assert_eq!(
        snapshot.text("source.id"),
        Some(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
    );
    assert_eq!(snapshot.text("game.state"), Some("driving"));
    assert_eq!(snapshot.number("vehicle.speed_kmh"), Some(102.0));
    assert_eq!(snapshot.number("drivetrain.gear"), Some(3.0));
    assert!(snapshot
        .number("input.throttle")
        .is_some_and(|value| (value - 0.65).abs() < 0.000_001));
    assert!(snapshot
        .number("vehicle.rpm_ratio")
        .is_some_and(|value| (value - 0.7).abs() < 0.000_001));
    assert!(snapshot
        .number("wheel.slip.max")
        .is_some_and(|value| (value - 0.42).abs() < 0.000_001));
}
