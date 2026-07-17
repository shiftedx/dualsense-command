use super::*;

#[cfg(all(not(test), target_os = "windows"))]
use super::process_scanning::windows_process_image_paths_matching;

#[derive(Debug, Clone)]
pub(crate) struct LocalGameValidation {
    response: ValidateLocalGameResponse,
    canonical_executable: PathBuf,
    install_path: PathBuf,
}

pub(crate) const USER_GAME_PROCESS_CANDIDATE_LIMIT: usize = 8;

pub(crate) const USER_GAME_PROCESS_SCAN_LIMIT: usize = 256;

/// Build the synthesized user-game id for a Steam app.
pub(crate) fn user_game_id_for_app_id(app_id: &str) -> String {
    format!("custom-{}", app_id.trim())
}

/// Scan the top level of a Steam game's install path for plausible launcher
/// executables. Recursive scans are intentionally avoided so we don't walk
/// large game directories during a snapshot/library call.
/// Normalise an incoming list of process-name overrides. Trims whitespace,
/// strips any path separators (user might paste a full path), drops empty
/// entries, enforces a .exe suffix, deduplicates case-insensitively, and caps
/// the list at `USER_GAME_PROCESS_CANDIDATE_LIMIT` entries.
pub(crate) fn sanitize_user_game_process_names(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in raw {
        // Strip any directory components — only the file name is meaningful for
        // process matching.
        let name = std::path::Path::new(value.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        if !name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        if out
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        out.push(name);
        if out.len() >= USER_GAME_PROCESS_CANDIDATE_LIMIT {
            break;
        }
    }
    out
}

pub(crate) fn discover_user_game_process_candidates(install_path: &FsPath) -> Vec<String> {
    let Ok(entries) = fs::read_dir(install_path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten().take(USER_GAME_PROCESS_SCAN_LIMIT) {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.to_ascii_lowercase().ends_with(".exe") {
            continue;
        }
        if is_excluded_user_game_process(file_name) {
            continue;
        }
        // Confirm it's a real file rather than a directory entry that happens
        // to end in .exe.
        let is_file = entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false);
        if !is_file {
            continue;
        }
        names.push(file_name.to_string());
        if names.len() >= USER_GAME_PROCESS_CANDIDATE_LIMIT {
            break;
        }
    }
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

pub(crate) fn is_excluded_user_game_process(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.starts_with("uninst")
        || lower.starts_with("setup")
        || lower.starts_with("unitycrashhandler")
        || lower.starts_with("ueprereqsetup")
        || lower.contains("crash")
        || lower.starts_with("vc_redist")
        || lower.starts_with("vcredist")
        || lower.starts_with("dotnetfx")
        || lower.starts_with("eossdk")
        || lower.starts_with("eacrash")
        || lower.starts_with("easetup")
        || lower.starts_with("easyanticheat")
        || lower.starts_with("redist")
        || lower.contains("installer")
        || lower.contains("launcher_setup")
}

pub(crate) fn user_game_to_supported_summary(
    game: &UserGameConfig,
    steam_root: Option<&FsPath>,
    stats: SteamGameStats,
) -> SupportedGameSummary {
    let install_path = PathBuf::from(&game.install_path);
    let installed = !game.install_path.is_empty() && install_path.is_dir();
    let artwork = match steam_root {
        Some(root) => user_game_artwork_for_app(root, &game.app_id),
        None => {
            let mut artwork = GameArtwork::default();
            apply_steam_cdn_artwork_fallback(&mut artwork, &game.app_id);
            artwork
        }
    };
    SupportedGameSummary {
        game_id: game.game_id.clone(),
        name: game.name.clone(),
        source: if game.game_id.starts_with("local-") {
            "local_app".to_string()
        } else {
            "steam".to_string()
        },
        input_provider: if game.game_id.starts_with("local-") {
            "dscc_input_bridge".to_string()
        } else {
            "steam_input".to_string()
        },
        app_id: (!game.app_id.is_empty()).then(|| game.app_id.clone()),
        install_path: if game.game_id.starts_with("local-") {
            None
        } else {
            (!game.install_path.is_empty()).then(|| game.install_path.clone())
        },
        process_names: game.process_names.clone(),
        executable_name: game.process_names.first().cloned(),
        installed,
        running: false,
        support_level: "custom".to_string(),
        artwork,
        stats,
    }
}

pub(crate) fn detection_allows_input_bridge(detection: &GameDetectionResponse) -> bool {
    let Some(active_game_id) = detection.active_game_id.as_deref() else {
        return false;
    };
    if !active_game_id.starts_with("local-") {
        return false;
    }
    detection.selected_game.as_ref().is_some_and(|game| {
        game.game_id == active_game_id
            && game.source == "local_app"
            && game.input_provider == "dscc_input_bridge"
    })
}

#[cfg(test)]
pub(crate) async fn local_app_execution_verified_for_input_bridge(
    _state: &AgentState,
    detection: &GameDetectionResponse,
) -> bool {
    detection_allows_input_bridge(detection)
}

#[cfg(not(test))]
pub(crate) async fn local_app_execution_verified_for_input_bridge(
    state: &AgentState,
    detection: &GameDetectionResponse,
) -> bool {
    let Some(active_game_id) = detection.active_game_id.as_deref() else {
        return false;
    };
    let user_game = {
        let inner = state.inner.read().await;
        inner.user_games.get(active_game_id).cloned()
    };
    let Some(user_game) = user_game else {
        return false;
    };
    tokio::task::spawn_blocking(move || registered_local_app_is_running(&user_game))
        .await
        .unwrap_or(false)
}

#[cfg(not(test))]
pub(crate) fn registered_local_app_is_running(game: &UserGameConfig) -> bool {
    if game.process_names.is_empty() {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        let Ok(install_root) = PathBuf::from(&game.install_path).canonicalize() else {
            return false;
        };
        windows_process_image_paths_matching(&game.process_names)
            .map(|paths| {
                paths
                    .iter()
                    .any(|path| local_app_process_path_allowed(game, &install_root, path))
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn local_app_process_path_allowed(
    game: &UserGameConfig,
    install_root: &FsPath,
    process_path: &FsPath,
) -> bool {
    let Some(file_name) = process_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !game
        .process_names
        .iter()
        .any(|process| process.eq_ignore_ascii_case(file_name))
    {
        return false;
    }
    process_path
        .canonicalize()
        .map(|path| path.starts_with(install_root))
        .unwrap_or(false)
}

pub(crate) async fn validate_local_game(
    Json(request): Json<ValidateLocalGameRequest>,
) -> Result<Json<ValidateLocalGameResponse>, (StatusCode, Json<serde_json::Value>)> {
    validate_local_game_request(
        request.name.as_deref(),
        &request.executable_path,
        &request.process_names,
    )
    .map(|validation| validation.response)
    .map(Json)
}

pub(crate) async fn add_local_game(
    State(state): State<AgentState>,
    Json(request): Json<AddLocalGameRequest>,
) -> Result<(StatusCode, Json<AddUserGameResponse>), (StatusCode, Json<serde_json::Value>)> {
    let validation = validate_local_game_request(
        Some(&request.name),
        &request.executable_path,
        &request.process_names,
    )?;
    let canonical_exe = validation.canonical_executable.clone();
    let install_path = validation.install_path.display().to_string();
    let game_id = local_game_id(&validation.response.name, &canonical_exe);
    let new_game = UserGameConfig {
        game_id: game_id.clone(),
        app_id: format!(
            "local:{}",
            short_stable_hash(&canonical_exe.display().to_string())
        ),
        name: validation.response.name.clone(),
        install_dir: validation
            .install_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("LocalApp")
            .to_string(),
        install_path,
        process_names: validation.response.process_names.clone(),
        added_at: current_timestamp(),
    };
    let (summary, to_save) = {
        let mut inner = state.inner.write().await;
        if inner.user_games.contains_key(&game_id) {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "Local app already registered",
                    "gameId": game_id,
                })),
            ));
        }
        if inner
            .user_games
            .values()
            .any(|game| game.app_id == new_game.app_id)
        {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "Local app executable already registered"
                })),
            ));
        }
        inner.user_games.insert(game_id.clone(), new_game.clone());
        inner.push_log(LogEntry {
            level: "info".to_string(),
            message: format!(
                "Registered local app {} ({} processes)",
                new_game.name,
                new_game.process_names.len()
            ),
            timestamp: current_timestamp(),
        });
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let summary = user_game_to_supported_summary(&new_game, None, SteamGameStats::default());
        (summary, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.value = None;
        cache.refreshed_at = None;
    }
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("local-game-added".to_string()),
    });
    Ok((
        StatusCode::CREATED,
        Json(AddUserGameResponse { game: summary }),
    ))
}

pub(crate) fn validate_local_game_request(
    name: Option<&str>,
    executable_path: &str,
    process_names: &[String],
) -> Result<LocalGameValidation, (StatusCode, Json<serde_json::Value>)> {
    let requested = executable_path.trim();
    if requested.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "executablePath is required"})),
        ));
    }
    let path = PathBuf::from(requested);
    let canonical = path.canonicalize().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Local app path could not be validated"})),
        )
    })?;
    if !canonical.is_file()
        || canonical
            .extension()
            .and_then(|ext| ext.to_str())
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Local app path must point to a .exe file"})),
        ));
    }
    let executable_name = canonical
        .file_name()
        .and_then(|file| file.to_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Local app executable name is invalid"})),
            )
        })?
        .to_string();
    if is_protected_local_app_process(&executable_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Local app executable is a protected system process"
            })),
        ));
    }
    let install_path = canonical.parent().map(PathBuf::from).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Local app parent directory is invalid"})),
        )
    })?;
    let mut processes = sanitize_user_game_process_names(process_names);
    if !processes
        .iter()
        .any(|process| process.eq_ignore_ascii_case(&executable_name))
    {
        processes.insert(0, executable_name.clone());
    }
    if processes.len() > USER_GAME_PROCESS_CANDIDATE_LIMIT {
        processes.truncate(USER_GAME_PROCESS_CANDIDATE_LIMIT);
    }
    let processes = validate_local_app_process_names(&install_path, processes)?;
    let name = name
        .and_then(|name| {
            let trimmed = name.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .unwrap_or_else(|| {
            executable_name
                .strip_suffix(".exe")
                .or_else(|| executable_name.strip_suffix(".EXE"))
                .unwrap_or(&executable_name)
                .to_string()
        });
    Ok(LocalGameValidation {
        response: ValidateLocalGameResponse {
            valid: true,
            name,
            executable_name,
            process_names: processes,
            warnings: Vec::new(),
        },
        canonical_executable: canonical,
        install_path,
    })
}

pub(crate) fn validate_local_app_process_names(
    install_path: &FsPath,
    processes: Vec<String>,
) -> Result<Vec<String>, (StatusCode, Json<serde_json::Value>)> {
    let mut validated = Vec::new();
    for process in processes {
        if is_protected_local_app_process(&process) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Watched process is a protected system process"
                })),
            ));
        }
        let valid = install_path
            .join(&process)
            .canonicalize()
            .ok()
            .filter(|path| path.starts_with(install_path))
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            })
            .is_some();
        if !valid {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Watched processes must be .exe files in the selected app folder"
                })),
            ));
        }
        if !validated
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&process))
        {
            validated.push(process);
        }
    }
    Ok(validated)
}

pub(crate) fn is_protected_local_app_process(process: &str) -> bool {
    const PROTECTED: &[&str] = &[
        "csrss.exe",
        "dwm.exe",
        "explorer.exe",
        "lsass.exe",
        "services.exe",
        "smss.exe",
        "spoolsv.exe",
        "svchost.exe",
        "system.exe",
        "taskhostw.exe",
        "wininit.exe",
        "winlogon.exe",
    ];
    PROTECTED
        .iter()
        .any(|protected| protected.eq_ignore_ascii_case(process.trim()))
}

pub(crate) fn local_game_id(name: &str, executable_path: &FsPath) -> String {
    format!(
        "local-{}-{}",
        slug_fragment(name),
        short_stable_hash(&executable_path.display().to_string())
    )
}

pub(crate) fn slug_fragment(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 32 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

pub(crate) fn short_stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")[..8].to_string()
}

pub(crate) async fn add_custom_game(
    State(state): State<AgentState>,
    Json(request): Json<AddUserGameRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let app_id = request.app_id.trim().to_string();
    if app_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId is required"})),
        ));
    }

    let game_id = user_game_id_for_app_id(&app_id);
    if let Some(module) = built_in_game_modules()
        .iter()
        .find(|module| module.steam_app_ids.contains(&app_id.as_str()))
    {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "A built-in module already covers this Steam appId",
                "appId": app_id,
                "gameId": module.id,
            })),
        ));
    }

    // Look up Steam manifest first (outside any lock; this hits the disk).
    let manifest_lookup_app_id = app_id.clone();
    let manifest =
        tokio::task::spawn_blocking(move || locate_steam_manifest(&manifest_lookup_app_id))
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Steam manifest lookup task failed");
                None
            });
    let Some(manifest) = manifest else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Steam app manifest not found",
                "appId": app_id,
            })),
        ));
    };

    // If the client supplied explicit process names, trust them; otherwise scan
    // the install dir. The override path is the escape hatch for games whose
    // .exe lives in a subfolder or is named oddly.
    let process_names = if !request.process_names.is_empty() {
        sanitize_user_game_process_names(&request.process_names)
    } else {
        let process_candidates_path = manifest.install_path.clone();
        tokio::task::spawn_blocking(move || {
            discover_user_game_process_candidates(&process_candidates_path)
        })
        .await
        .unwrap_or_default()
    };

    let added_at = current_timestamp();
    let new_game = UserGameConfig {
        game_id: game_id.clone(),
        app_id: manifest.app_id.clone(),
        name: manifest.name.clone(),
        install_dir: manifest.install_dir.clone(),
        install_path: manifest.install_path.display().to_string(),
        process_names,
        added_at,
    };

    let (summary, to_save) = {
        let mut inner = state.inner.write().await;
        if inner.user_games.contains_key(&game_id) {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "Game already registered",
                    "gameId": game_id,
                })),
            ));
        }
        inner.user_games.insert(game_id.clone(), new_game.clone());
        inner.push_log(LogEntry {
            level: "info".to_string(),
            message: format!(
                "Registered custom Steam game {} ({} processes)",
                new_game.name,
                new_game.process_names.len()
            ),
            timestamp: current_timestamp(),
        });
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        let summary = user_game_to_supported_summary(&new_game, None, SteamGameStats::default());
        (summary, build_persist_snapshot(&inner))
    };
    persist_snapshot(&state, to_save).await;
    // Invalidate the detection cache so the new game shows up immediately.
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.value = None;
        cache.refreshed_at = None;
    }
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("user-game-added".to_string()),
    });

    Ok((
        StatusCode::CREATED,
        Json(AddUserGameResponse { game: summary }),
    ))
}

pub(crate) async fn remove_custom_game(
    Path(game_id): Path<String>,
    State(state): State<AgentState>,
) -> Result<StatusCode, StatusCode> {
    let to_save = {
        let mut inner = state.inner.write().await;
        if inner.user_games.remove(&game_id).is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
        inner.push_log(LogEntry {
            level: "info".to_string(),
            message: format!("Removed custom game {game_id}"),
            timestamp: current_timestamp(),
        });
        inner.effect_revision = inner.effect_revision.saturating_add(1);
        if inner.auto_loaded_profile_id.is_some() {
            // The detection cache is invalidated below; auto-loaded profile
            // re-resolves on the next snapshot pass.
        }
        build_persist_snapshot(&inner)
    };
    persist_snapshot(&state, to_save).await;
    {
        let mut cache = state.discovery_cache.game_detection.lock().await;
        cache.value = None;
        cache.refreshed_at = None;
    }
    let _ = state.event_tx.send(RealtimeMessage {
        kind: "snapshot_invalidated".to_string(),
        controller: None,
        message: Some("user-game-removed".to_string()),
    });
    Ok(StatusCode::NO_CONTENT)
}
