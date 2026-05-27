use super::support::*;
use super::*;

#[test]
fn user_game_id_uses_custom_prefix() {
    assert_eq!(user_game_id_for_app_id("12345"), "custom-12345");
}

#[test]
fn user_game_process_candidates_filter_known_uninstaller_patterns() {
    let install_dir = temp_test_dir("dscc-user-game-procs");
    fs::create_dir_all(&install_dir).expect("install dir");
    for name in [
        "Game.exe",
        "GameLauncher.exe",
        "UnityCrashHandler.exe",
        "uninstall.exe",
        "setup.exe",
        "vcredist_x64.exe",
        "EasyAntiCheat.exe",
        "readme.txt",
    ] {
        fs::write(install_dir.join(name), [0_u8; 4]).expect("touch file");
    }
    let candidates = discover_user_game_process_candidates(&install_dir);
    assert!(candidates.iter().any(|n| n == "Game.exe"));
    assert!(candidates.iter().any(|n| n == "GameLauncher.exe"));
    assert!(!candidates
        .iter()
        .any(|n| n.eq_ignore_ascii_case("UnityCrashHandler.exe")));
    assert!(!candidates
        .iter()
        .any(|n| n.eq_ignore_ascii_case("uninstall.exe")));
    assert!(!candidates
        .iter()
        .any(|n| n.eq_ignore_ascii_case("setup.exe")));
    assert!(!candidates
        .iter()
        .any(|n| n.eq_ignore_ascii_case("vcredist_x64.exe")));
    assert!(!candidates
        .iter()
        .any(|n| n.eq_ignore_ascii_case("EasyAntiCheat.exe")));
    let _ = fs::remove_dir_all(install_dir);
}

#[tokio::test]
async fn steam_library_endpoint_lists_installed_games() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-steam-lib-list");
    install_test_steam_manifest(
        &steam_root,
        FORZA_HORIZON5_STEAM_APP_ID,
        "Forza Horizon 5",
        "ForzaHorizon5",
        &["ForzaHorizon5.exe"],
    );
    install_test_steam_manifest(
        &steam_root,
        "987654321",
        "Imaginary Indie Racer",
        "ImaginaryRacer",
        &["ImaginaryRacer.exe", "uninstall.exe"],
    );
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/games/steam-library")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let games = parsed
        .get("games")
        .and_then(|games| games.as_array())
        .expect("games array");
    let fh5 = games
        .iter()
        .find(|game| {
            game.get("appId").and_then(|app_id| app_id.as_str())
                == Some(FORZA_HORIZON5_STEAM_APP_ID)
        })
        .expect("FH5 present");
    assert_eq!(
        fh5.get("alreadyInCatalog")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        fh5.get("suggestedGameId").and_then(|value| value.as_str()),
        Some(format!("custom-{FORZA_HORIZON5_STEAM_APP_ID}").as_str())
    );
    let indie = games
        .iter()
        .find(|game| game.get("appId").and_then(|value| value.as_str()) == Some("987654321"))
        .expect("imaginary indie present");
    assert_eq!(
        indie
            .get("alreadyInCatalog")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    let process_candidates = indie
        .get("processCandidates")
        .and_then(|value| value.as_array())
        .expect("process candidates");
    let process_names: Vec<&str> = process_candidates
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert!(process_names.contains(&"ImaginaryRacer.exe"));
    assert!(!process_names
        .iter()
        .any(|name| name.eq_ignore_ascii_case("uninstall.exe")));
    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn add_custom_game_rejects_unknown_app_id() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-add-custom-404");
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/custom")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"appId":"42"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn add_custom_game_rejects_duplicate_registration() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-add-custom-dup");
    install_test_steam_manifest(
        &steam_root,
        "555000",
        "Sample Racer",
        "SampleRacer",
        &["SampleRacer.exe"],
    );
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    let router = app(AgentState::mock());
    let first = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/custom")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"appId":"555000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let body = to_bytes(first.into_body(), 64 * 1024).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let game = parsed.get("game").expect("game key");
    assert_eq!(
        game.get("gameId").and_then(|value| value.as_str()),
        Some("custom-555000")
    );
    assert_eq!(
        game.get("supportLevel").and_then(|value| value.as_str()),
        Some("custom")
    );

    let duplicate = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/custom")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"appId":"555000"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn add_and_remove_custom_game_round_trips() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-add-remove-custom");
    install_test_steam_manifest(
        &steam_root,
        "777111",
        "Removable Racer",
        "RemovableRacer",
        &["RemovableRacer.exe"],
    );
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    let state = AgentState::mock();
    let router = app(state.clone());

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/games/custom")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"appId":"777111"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    {
        let inner = state.inner.read().await;
        assert!(inner.user_games.contains_key("custom-777111"));
    }

    let delete_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/games/custom/custom-777111")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let missing_delete = router
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/games/custom/custom-777111")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_delete.status(), StatusCode::NOT_FOUND);

    {
        let inner = state.inner.read().await;
        assert!(!inner.user_games.contains_key("custom-777111"));
    }
    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn browse_steam_library_lists_root_entries() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-browse-root");
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    install_test_steam_manifest(
        &steam_root,
        "9911",
        "Browse Test Game",
        "BrowseTestGame",
        &["LauncherA.exe", "GameB.exe"],
    );
    // Add a nested directory so we can confirm directories are surfaced.
    let install_path = steam_root
        .join("steamapps")
        .join("common")
        .join("BrowseTestGame");
    fs::create_dir_all(install_path.join("Binaries").join("Win64")).expect("nested dirs");
    fs::write(
        install_path
            .join("Binaries")
            .join("Win64")
            .join("Game-Shipping.exe"),
        [0_u8; 4],
    )
    .expect("nested exe");

    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/games/steam-library/browse?appId=9911&path=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let payload: SteamLibraryBrowseResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload.app_id, "9911");
    assert_eq!(payload.relative_path, "");
    assert!(!payload.truncated);
    let names: Vec<_> = payload.entries.iter().map(|e| e.name.as_str()).collect();
    // Directories sort first, then exes — alphabetical within each group.
    assert_eq!(names, vec!["Binaries", "GameB.exe", "LauncherA.exe"]);
    let kinds: Vec<_> = payload.entries.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, vec!["dir", "exe", "exe"]);

    // Walk into the nested directory.
    let nested = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/games/steam-library/browse?appId=9911&path=Binaries/Win64")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(nested.status(), StatusCode::OK);
    let body = to_bytes(nested.into_body(), 1024 * 1024).await.unwrap();
    let payload: SteamLibraryBrowseResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload.relative_path, "Binaries/Win64");
    let names: Vec<_> = payload.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["Game-Shipping.exe"]);

    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn browse_steam_library_blocks_path_traversal() {
    let _env = TestEnv::new(&[
        "DSCC_STEAM_ROOT",
        "ProgramFiles(x86)",
        "ProgramFiles",
        "LOCALAPPDATA",
    ]);
    let steam_root = make_test_steam_root("dscc-browse-traversal");
    std::env::set_var("DSCC_STEAM_ROOT", &steam_root);
    std::env::remove_var("ProgramFiles(x86)");
    std::env::remove_var("ProgramFiles");
    std::env::remove_var("LOCALAPPDATA");

    install_test_steam_manifest(
        &steam_root,
        "9912",
        "Traversal Test",
        "TraversalTest",
        &["GameOnly.exe"],
    );

    let response = app(AgentState::mock())
        .oneshot(
            Request::builder()
                .uri("/api/games/steam-library/browse?appId=9912&path=../..")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = fs::remove_dir_all(&steam_root);
}

#[tokio::test]
async fn snapshot_supported_games_includes_user_games_with_custom_support_level() {
    let state = AgentState::mock();
    {
        let mut inner = state.inner.write().await;
        inner.user_games.insert(
            "custom-12345".to_string(),
            make_user_game(
                "12345",
                "Test Custom Game",
                "C:/dscc/fake/install",
                &["TestCustomGame.exe"],
            ),
        );
    }
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/snapshot")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let snapshot: AgentSnapshotResponse = serde_json::from_slice(&body).unwrap();
    let custom = snapshot
        .game_detection
        .supported_games
        .iter()
        .find(|game| game.game_id == "custom-12345")
        .expect("custom game appears in supported_games");
    assert_eq!(custom.support_level, "custom");
    assert_eq!(custom.app_id.as_deref(), Some("12345"));
    assert_eq!(custom.name, "Test Custom Game");
}

#[test]
fn persisted_state_round_trips_user_games() {
    let mut inner_user_games = BTreeMap::new();
    inner_user_games.insert(
        "custom-99887".to_string(),
        make_user_game(
            "99887",
            "Round Trip Racer",
            "C:/dscc/round-trip",
            &["RoundTrip.exe"],
        ),
    );
    let persisted = PersistedAgentState {
        version: PERSISTED_STATE_VERSION,
        user_games: inner_user_games.clone(),
        ..Default::default()
    };
    let serialized = serde_json::to_string(&persisted).expect("serialize");
    let deserialized: PersistedAgentState = serde_json::from_str(&serialized).expect("deserialize");
    let normalized = deserialized.normalized();
    assert!(normalized.user_games.contains_key("custom-99887"));
    let restored = normalized
        .user_games
        .get("custom-99887")
        .expect("user game survives");
    assert_eq!(restored.name, "Round Trip Racer");
    assert_eq!(restored.process_names, vec!["RoundTrip.exe".to_string()]);
}

#[test]
fn normalization_drops_user_games_that_collide_with_built_in_modules() {
    let mut user_games = BTreeMap::new();
    // Use a built-in module id (forza-horizon-5) as the user game id;
    // normalization should drop it.
    user_games.insert(
        "forza-horizon-5".to_string(),
        UserGameConfig {
            game_id: "forza-horizon-5".to_string(),
            app_id: FORZA_HORIZON5_STEAM_APP_ID.to_string(),
            name: "Bad clone".to_string(),
            install_dir: "FH5".to_string(),
            install_path: "C:/whatever".to_string(),
            process_names: vec!["ForzaHorizon5.exe".to_string()],
            added_at: current_timestamp(),
        },
    );
    let persisted = PersistedAgentState {
        version: PERSISTED_STATE_VERSION,
        user_games,
        ..Default::default()
    };
    let normalized = persisted.normalized();
    assert!(normalized.user_games.is_empty());
}

#[test]
fn process_detection_matches_user_game_process_name() {
    let mut user_games = BTreeMap::new();
    user_games.insert(
        "custom-99887".to_string(),
        make_user_game(
            "99887",
            "Round Trip Racer",
            "C:/dscc/round-trip",
            &["RoundTrip.exe"],
        ),
    );
    let detection =
        detect_running_game_from_processes_with_user_games(["RoundTrip.exe"], &user_games);
    assert_eq!(detection.active_game_id.as_deref(), Some("custom-99887"));
    assert_eq!(detection.module_id.as_deref(), Some("custom-99887"));
    // Custom games do not have a telemetry adapter; the response omits
    // the adapter id and profile id.
    assert!(detection.adapter_id.is_none());
    assert!(detection.profile_id.is_none());
    assert_eq!(detection.candidates.len(), 1);
}

#[test]
fn user_game_detection_keeps_global_profile_until_supported_module_matches() {
    let mut user_games = BTreeMap::new();
    user_games.insert(
        "custom-99887".to_string(),
        make_user_game(
            "99887",
            "Round Trip Racer",
            "C:/dscc/round-trip",
            &["RoundTrip.exe"],
        ),
    );
    let detection =
        detect_running_game_from_processes_with_user_games(["RoundTrip.exe"], &user_games);
    let mut state = AgentStateInner {
        controllers: ControllerRegistry::default(),
        controller_names: BTreeMap::new(),
        profiles: profiles_with_active(default_profiles(), &Some(DEFAULT_PROFILE_ID.to_string())),
        adapters: default_adapters(),
        telemetry: SignalSnapshot::default(),
        logs: Vec::new(),
        device_backend: DeviceBackendSummary::mock(),
        storage: None,
        controller_configs: BTreeMap::new(),
        profile_configs: BTreeMap::new(),
        profile_overrides: BTreeMap::new(),
        edge_profiles: BTreeMap::new(),
        app_settings: AppSettings::default(),
        active_profile_id: Some(DEFAULT_PROFILE_ID.to_string()),
        active_adapter_id: None,
        auto_loaded_profile_id: None,
        adapter_runtimes: default_adapter_runtimes(),
        forza_effect_runtime: ForzaEffectRuntime::default(),
        effect_revision: 0,
        user_games,
    };

    assert!(!sync_auto_loaded_profile_for_detection(
        &mut state, &detection
    ));
    assert_eq!(state.active_profile_id.as_deref(), Some(DEFAULT_PROFILE_ID));
    assert_eq!(state.auto_loaded_profile_id, None);
}
