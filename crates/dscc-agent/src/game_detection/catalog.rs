use super::*;

pub(crate) fn enrich_game_detection(
    mut detection: GameDetectionResponse,
    catalog: &SteamGameCatalog,
) -> GameDetectionResponse {
    let active_game_id = detection.active_game_id.as_deref();
    let mut supported_games = catalog.supported_games.clone();
    for game in &mut supported_games {
        game.running = active_game_id == Some(game.game_id.as_str());
    }

    detection.selected_game = active_game_id.and_then(|id| {
        supported_games
            .iter()
            .find(|game| game.game_id == id)
            .cloned()
    });
    detection.supported_games = supported_games;
    detection
}

/// Append every registered user game to the detection's `supported_games`
/// list. Built-in modules sort first; user games sort alphabetically after.
pub(crate) fn append_user_games_to_detection(
    detection: &mut GameDetectionResponse,
    user_games: &BTreeMap<String, UserGameConfig>,
    steam_root: Option<&FsPath>,
    steam_stats: &BTreeMap<String, SteamGameStats>,
) {
    if user_games.is_empty() {
        return;
    }

    let active_game_id = detection.active_game_id.clone();
    let mut user_entries: Vec<SupportedGameSummary> = user_games
        .values()
        .map(|game| {
            let stats = steam_stats.get(&game.app_id).cloned().unwrap_or_default();
            let mut summary = user_game_to_supported_summary(game, steam_root, stats);
            summary.running = active_game_id.as_deref() == Some(summary.game_id.as_str());
            summary
        })
        .collect();
    user_entries.sort_by_key(|game| game.name.to_ascii_lowercase());

    detection.supported_games.extend(user_entries);

    if let Some(active_id) = active_game_id.as_deref() {
        if detection
            .selected_game
            .as_ref()
            .is_none_or(|game| game.game_id != active_id)
        {
            detection.selected_game = detection
                .supported_games
                .iter()
                .find(|game| game.game_id == active_id)
                .cloned();
        }
    }
}

pub(crate) fn telemetry_game_detection(
    inner: &AgentStateInner,
    catalog: &SteamGameCatalog,
) -> Option<GameDetectionResponse> {
    let adapter_id = inner
        .telemetry
        .text("source.id")
        .or(inner.active_adapter_id.as_deref())?;
    let runtime = inner.adapter_runtime(adapter_id)?;
    if !runtime.has_recent_packet(Instant::now()) {
        return None;
    }

    let game = telemetry_game_module_for_adapter(inner, catalog, adapter_id)?;
    let candidate = game.telemetry_detection_candidate(adapter_id, 70);

    Some(GameDetectionResponse {
        active_game_id: Some(candidate.game_id.clone()),
        active_game_name: Some(candidate.name.clone()),
        source: "telemetry_source".to_string(),
        confidence: candidate.confidence,
        process_name: None,
        module_id: Some(candidate.module_id.clone()),
        adapter_id: Some(candidate.adapter_id.clone()),
        profile_id: Some(candidate.profile_id.clone()),
        candidates: vec![candidate],
        supported_games: Vec::new(),
        selected_game: None,
    })
}

pub(crate) fn telemetry_game_module_for_adapter(
    inner: &AgentStateInner,
    catalog: &SteamGameCatalog,
    adapter_id: &str,
) -> Option<GameModule> {
    let modules: Vec<GameModule> = built_in_game_modules()
        .iter()
        .copied()
        .filter(|game| game.telemetry_link().adapter_id == adapter_id)
        .collect();
    if modules.is_empty() {
        return None;
    }

    if let Some(game_id) = inner.telemetry.text("game.id") {
        if let Some(game) = modules
            .iter()
            .copied()
            .find(|game| game.identity().id == game_id)
        {
            return Some(game);
        }
    }

    let installed: Vec<&SupportedGameSummary> = catalog
        .supported_games
        .iter()
        .filter(|summary| {
            summary.installed
                && modules
                    .iter()
                    .any(|game| game.identity().id == summary.game_id.as_str())
        })
        .collect();
    if installed.len() == 1 {
        let game_id = installed[0].game_id.as_str();
        if let Some(game) = modules
            .iter()
            .copied()
            .find(|game| game.identity().id == game_id)
        {
            return Some(game);
        }
    }

    modules.first().copied()
}

pub(crate) fn supported_game_install_path(
    catalog: &SteamGameCatalog,
    game_id: &str,
) -> Option<PathBuf> {
    catalog
        .supported_games
        .iter()
        .find(|game| game.game_id == game_id && game.installed)
        .and_then(|game| game.install_path.as_deref())
        .map(PathBuf::from)
}

pub(crate) async fn get_detected_game(
    State(state): State<AgentState>,
) -> Json<GameDetectionResponse> {
    Json(state.cached_game_detection().await)
}
