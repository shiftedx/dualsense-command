use super::*;
use axum::{handler::HandlerWithoutStateExt, http::Uri, response::Response};

pub fn app(state: AgentState) -> Router {
    let dist = web_dist_dir();
    let index = dist.join("index.html");
    let spa_fallback = move |uri: Uri| {
        let index = index.clone();
        async move { spa_fallback_response(&index, uri.path()).await }
    };
    let static_assets = ServeDir::new(&dist).fallback(spa_fallback.into_service());

    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/update-check", get(get_update_check))
        .route(
            "/api/app-settings",
            get(get_app_settings).put(update_app_settings),
        )
        .route("/api/snapshot", get(get_snapshot))
        .route("/api/controllers", get(list_controllers))
        .route(
            "/api/controllers/{id}",
            get(get_controller).put(update_controller),
        )
        .route(
            "/api/controllers/{id}/config",
            get(get_controller_config).put(update_controller_config),
        )
        .route("/api/controllers/{id}/input", get(get_controller_input))
        .route(
            "/api/controllers/{id}/edge-profiles",
            get(get_edge_profiles),
        )
        .route(
            "/api/controllers/{id}/edge-profiles/{slot}",
            put(write_edge_profile),
        )
        .route("/api/controllers/{id}/test-effect", post(test_effect))
        .route(
            "/api/controllers/current/test-effect",
            post(test_current_effect),
        )
        .route(
            "/api/controllers/current/input",
            get(get_current_controller_input),
        )
        .route("/api/profiles", get(list_profiles).post(create_profile))
        .route("/api/profiles/import", post(import_profile))
        .route(
            "/api/profiles/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .route("/api/profiles/{id}/config", put(update_profile_config))
        .route("/api/profiles/{id}/export", get(export_profile))
        .route("/api/profiles/{id}/activate", post(activate_profile))
        .route("/api/adapters", get(list_adapters))
        .route("/api/adapters/{id}", put(update_adapter))
        .route("/api/steam-input", get(get_steam_input_status))
        .route(
            "/api/steam-input/bindings",
            post(update_steam_input_binding),
        )
        .route(
            "/api/steam-input/paddle-preset",
            post(apply_steam_input_paddle_preset),
        )
        .route("/api/input-bridge", get(get_input_bridge_status))
        .route(
            "/api/input-bridge/bindings",
            post(write_input_bridge_binding),
        )
        .route(
            "/api/input-bridge/sessions/{controller_id}",
            get(get_input_bridge_session),
        )
        .route(
            "/api/input-bridge/sessions/{controller_id}/start",
            post(start_input_bridge_session),
        )
        .route(
            "/api/input-bridge/sessions/{controller_id}/stop",
            post(stop_input_bridge_session),
        )
        .route("/api/modules", get(list_modules))
        .route("/api/games/detected", get(get_detected_game))
        .route("/api/games/art/{game_id}/{kind}", get(get_game_art))
        .route(
            "/api/games/steam-art/{app_id}/{kind}",
            get(get_steam_app_art),
        )
        .route("/api/games/steam-library", get(list_steam_library))
        .route("/api/games/steam-library/browse", get(browse_steam_library))
        .route("/api/games/local/validate", post(validate_local_game))
        .route("/api/games/local", post(add_local_game))
        .route("/api/games/custom", post(add_custom_game))
        .route("/api/games/custom/{game_id}", delete(remove_custom_game))
        .route("/api/effects/current", get(get_current_effect))
        .route("/api/profile-resolution", get(get_profile_resolution))
        .route(
            "/api/profile-resolution/override",
            put(set_profile_override).delete(clear_profile_override),
        )
        .route("/api/telemetry", get(list_telemetry))
        .route("/api/logs", get(list_logs))
        .route("/api/diagnostics", get(get_diagnostics))
        .route("/api/diagnostics/support-bundle", get(get_support_bundle))
        .route("/api/support-bundle", get(get_support_bundle))
        .route("/api/ws", get(ws_handler))
        .layer(middleware::from_fn(reject_cross_origin_mutations))
        .fallback_service(static_assets)
        .with_state(state)
}

/// SPA fallback for paths that match neither an API route nor a static
/// asset: unknown `/api` paths stay 404, while app routes receive the
/// resolved `index.html` with HTTP 200 so deep links are not reported as
/// errors by status-sensitive clients.
async fn spa_fallback_response(index: &FsPath, path: &str) -> Response {
    if path == "/api" || path.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(index).await {
        Ok(contents) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            contents,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(crate) fn web_dist_dir() -> PathBuf {
    let current_exe = std::env::current_exe().ok();
    let current_dir = std::env::current_dir().ok();
    web_dist_dir_from_parts(
        configured_web_dist_dir(),
        current_exe.as_deref(),
        current_dir.as_deref(),
    )
}

pub(crate) fn configured_web_dist_dir() -> Option<PathBuf> {
    std::env::var_os("DSCC_WEB_DIST")
        .or_else(|| std::env::var_os("DSCC_WEB_DIST_DIR"))
        .map(PathBuf::from)
}

pub(crate) fn web_dist_dir_from_parts(
    configured: Option<PathBuf>,
    executable: Option<&FsPath>,
    current_dir: Option<&FsPath>,
) -> PathBuf {
    if let Some(path) = configured {
        return path;
    }

    let candidates = web_dist_candidates(executable, current_dir);
    candidates
        .iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .cloned()
        .unwrap_or_else(|| {
            candidates
                .into_iter()
                .next()
                .unwrap_or_else(|| PathBuf::from("web/dist"))
        })
}

pub(crate) fn web_dist_candidates(
    executable: Option<&FsPath>,
    current_dir: Option<&FsPath>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(current_dir) = current_dir {
        candidates.push(current_dir.join("web").join("dist"));
    }
    if let Some(executable_dir) = executable.and_then(FsPath::parent) {
        candidates.push(executable_dir.join("web").join("dist"));
        candidates.push(executable_dir.join("dist"));
        if let Some(install_parent) = executable_dir.parent() {
            candidates.push(install_parent.join("web").join("dist"));
        }
    }
    candidates.push(PathBuf::from("web/dist"));
    candidates
}
