use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct SteamGameCatalog {
    pub(crate) supported_games: Vec<SupportedGameSummary>,
    pub(crate) artwork_paths: BTreeMap<(String, String), PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionResponse {
    pub active_game_id: Option<String>,
    pub active_game_name: Option<String>,
    pub source: String,
    pub confidence: u8,
    pub process_name: Option<String>,
    pub module_id: Option<String>,
    pub adapter_id: Option<String>,
    pub profile_id: Option<String>,
    pub candidates: Vec<GameDetectionCandidate>,
    #[serde(default)]
    pub supported_games: Vec<SupportedGameSummary>,
    #[serde(default)]
    pub selected_game: Option<SupportedGameSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionCandidate {
    pub game_id: String,
    pub name: String,
    pub process_name: String,
    pub module_id: String,
    pub adapter_id: String,
    pub profile_id: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameArtwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamGameStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub achievements: Option<SteamAchievementStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamAchievementStats {
    pub unlocked: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportedGameSummary {
    pub game_id: String,
    pub name: String,
    #[serde(default = "default_game_source")]
    pub source: String,
    #[serde(default = "default_game_input_provider")]
    pub input_provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_name: Option<String>,
    pub installed: bool,
    pub running: bool,
    pub support_level: String,
    #[serde(default)]
    pub artwork: GameArtwork,
    #[serde(default)]
    pub stats: SteamGameStats,
}

pub(crate) fn default_game_source() -> String {
    "built_in".to_string()
}

pub(crate) fn default_game_input_provider() -> String {
    "native_dualsense".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserGameConfig {
    pub game_id: String,
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryEntry {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    pub artwork: GameArtwork,
    pub stats: SteamGameStats,
    pub already_in_catalog: bool,
    pub suggested_game_id: String,
    pub process_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryListResponse {
    pub games: Vec<SteamLibraryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameRequest {
    pub app_id: String,
    /// Optional override for the .exe process names DSCC will watch for to
    /// auto-load this game's profile. When omitted/empty, the agent uses the
    /// candidates it discovered by scanning the install directory. Useful for
    /// games whose .exe lives in a subfolder or isn't named obviously.
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLocalGameRequest {
    pub name: Option<String>,
    pub executable_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLocalGameResponse {
    pub valid: bool,
    pub name: String,
    pub executable_name: String,
    pub process_names: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLocalGameRequest {
    pub name: String,
    pub executable_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalGameValidation {
    response: ValidateLocalGameResponse,
    canonical_executable: PathBuf,
    install_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameResponse {
    pub game: SupportedGameSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseSteamLibraryParams {
    pub app_id: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseEntry {
    pub name: String,
    /// `"dir"` or `"exe"`. Kept as a string so the wire shape is forward
    /// compatible if we ever surface other kinds (data files, configs, ...).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseResponse {
    pub app_id: String,
    pub install_path: String,
    /// Path relative to `install_path`, using forward slashes and never
    /// containing `..` segments. Empty string means the install root.
    pub relative_path: String,
    pub entries: Vec<SteamLibraryBrowseEntry>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SteamAppManifest {
    pub(crate) app_id: String,
    pub(crate) name: String,
    pub(crate) install_dir: String,
    pub(crate) install_path: PathBuf,
}

pub(crate) fn discover_steam_game_catalog() -> SteamGameCatalog {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return unsupported_steam_game_catalog();
    };

    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    build_supported_steam_game_catalog(&steam_root, &libraries, &manifests)
}

pub(crate) fn unsupported_steam_game_catalog() -> SteamGameCatalog {
    SteamGameCatalog {
        supported_games: built_in_game_modules()
            .iter()
            .filter(|game| game.steam_catalog)
            .map(|game| {
                supported_game_summary(
                    game,
                    None,
                    None,
                    GameArtwork::default(),
                    SteamGameStats::default(),
                )
            })
            .collect(),
        artwork_paths: BTreeMap::new(),
    }
}

pub(crate) fn build_supported_steam_game_catalog(
    steam_root: &FsPath,
    libraries: &[PathBuf],
    manifests: &[SteamAppManifest],
) -> SteamGameCatalog {
    let mut supported_games = Vec::new();
    let mut artwork_paths = BTreeMap::new();
    let steam_stats = discover_steam_game_stats(steam_root);

    for game in built_in_game_modules()
        .iter()
        .filter(|game| game.steam_catalog)
    {
        let manifest = manifests
            .iter()
            .find(|manifest| steam_manifest_matches_game(manifest, game));
        let install_path = manifest
            .map(|manifest| manifest.install_path.clone())
            .or_else(|| find_steam_common_install_dir(libraries, game));
        let app_id = manifest
            .map(|manifest| manifest.app_id.clone())
            .or_else(|| {
                game.steam_app_ids
                    .first()
                    .map(|app_id| (*app_id).to_string())
            });
        let mut artwork = GameArtwork::default();

        if let Some(app_id) = app_id.as_deref() {
            for (kind, path) in discover_steam_artwork_paths(steam_root, app_id) {
                let key = (game.id.to_string(), kind);
                artwork_paths.insert(key.clone(), path);
                match key.1.as_str() {
                    "icon" => artwork.icon_url = Some(game_art_url(game.id, "icon")),
                    "banner" => artwork.banner_url = Some(game_art_url(game.id, "banner")),
                    "hero" => artwork.hero_url = Some(game_art_url(game.id, "hero")),
                    "capsule" => artwork.capsule_url = Some(game_art_url(game.id, "capsule")),
                    _ => {}
                }
            }
        }

        if let Some(app_id) = app_id.as_deref() {
            apply_steam_cdn_artwork_fallback(&mut artwork, app_id);
        }
        let stats = app_id
            .as_deref()
            .and_then(|app_id| steam_stats.get(app_id))
            .cloned()
            .unwrap_or_default();

        supported_games.push(supported_game_summary(
            game,
            app_id,
            install_path,
            artwork,
            stats,
        ));
    }

    SteamGameCatalog {
        supported_games,
        artwork_paths,
    }
}

pub(crate) fn discover_steam_game_stats(steam_root: &FsPath) -> BTreeMap<String, SteamGameStats> {
    let mut stats = BTreeMap::new();
    for user_dir in numeric_child_dirs(&steam_root.join("userdata"), 8) {
        let local_config = user_dir.join("config").join("localconfig.vdf");
        if let Ok(contents) = fs::read_to_string(&local_config) {
            merge_steam_game_stats_map(&mut stats, parse_steam_localconfig_stats(&contents));
        }
        merge_steam_game_achievement_cache(
            &mut stats,
            &user_dir.join("config").join("librarycache"),
        );
    }
    stats
}

pub(crate) fn merge_steam_game_stats_map(
    target: &mut BTreeMap<String, SteamGameStats>,
    updates: BTreeMap<String, SteamGameStats>,
) {
    for (app_id, update) in updates {
        merge_steam_game_stats(target.entry(app_id).or_default(), update);
    }
}

pub(crate) fn merge_steam_game_stats(target: &mut SteamGameStats, update: SteamGameStats) {
    if let Some(minutes) = update.playtime_minutes {
        target.playtime_minutes = Some(target.playtime_minutes.unwrap_or(0).max(minutes));
    }
    if let Some(last_played) = update.last_played_unix {
        target.last_played_unix = Some(target.last_played_unix.unwrap_or(0).max(last_played));
    }
    if let Some(achievements) = update.achievements {
        let replace = match target.achievements.as_ref() {
            Some(current) => {
                achievements.total > current.total
                    || (achievements.total == current.total
                        && achievements.unlocked > current.unlocked)
            }
            None => true,
        };
        if replace {
            target.achievements = Some(achievements);
        }
    }
}

pub(crate) fn parse_steam_localconfig_stats(contents: &str) -> BTreeMap<String, SteamGameStats> {
    let mut stats: BTreeMap<String, SteamGameStats> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
            continue;
        }
        if line == "}" {
            pending_block = None;
            stack.pop();
            continue;
        }

        let tokens = quoted_tokens(line);
        match tokens.as_slice() {
            [block] => pending_block = Some(block.to_string()),
            [key, value] => {
                pending_block = None;
                let Some(app_id) = steam_app_id_from_vdf_stack(&stack) else {
                    continue;
                };
                let entry = stats.entry(app_id.to_string()).or_default();
                match key.as_str() {
                    "Playtime" => entry.playtime_minutes = value.parse::<u64>().ok(),
                    "LastPlayed" => entry.last_played_unix = value.parse::<u64>().ok(),
                    _ => {}
                }
            }
            _ => pending_block = None,
        }
    }

    stats
}

pub(crate) fn steam_app_id_from_vdf_stack(stack: &[String]) -> Option<&str> {
    stack
        .windows(2)
        .rev()
        .find(|window| window[0] == "apps" && window[1].chars().all(|ch| ch.is_ascii_digit()))
        .map(|window| window[1].as_str())
}

pub(crate) fn merge_steam_game_achievement_cache(
    stats: &mut BTreeMap<String, SteamGameStats>,
    library_cache: &FsPath,
) {
    let progress = library_cache.join("achievement_progress.json");
    if let Ok(contents) = fs::read_to_string(progress) {
        for (app_id, achievements) in parse_steam_achievement_progress_cache(&contents) {
            merge_steam_game_stats(
                stats.entry(app_id).or_default(),
                SteamGameStats {
                    achievements: Some(achievements),
                    ..SteamGameStats::default()
                },
            );
        }
    }

    for app_id in built_in_game_modules()
        .iter()
        .filter(|game| game.steam_catalog)
        .flat_map(|game| game.steam_app_ids)
    {
        let app_cache = library_cache.join(format!("{app_id}.json"));
        if !fs::metadata(&app_cache)
            .map(|metadata| metadata.is_file() && metadata.len() <= 8 * 1024 * 1024)
            .unwrap_or(false)
        {
            continue;
        }
        let Ok(contents) = fs::read_to_string(app_cache) else {
            continue;
        };
        if let Some(achievements) = parse_steam_librarycache_achievements(&contents) {
            merge_steam_game_stats(
                stats.entry((*app_id).to_string()).or_default(),
                SteamGameStats {
                    achievements: Some(achievements),
                    ..SteamGameStats::default()
                },
            );
        }
    }
}

pub(crate) fn parse_steam_achievement_progress_cache(
    contents: &str,
) -> BTreeMap<String, SteamAchievementStats> {
    let mut stats = BTreeMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(contents) else {
        return stats;
    };
    let Some(entries) = value.get("mapCache").and_then(|value| value.as_array()) else {
        return stats;
    };

    for entry in entries {
        let Some(pair) = entry.as_array() else {
            continue;
        };
        let [app_id_value, stats_value] = pair.as_slice() else {
            continue;
        };
        let Some(app_id) = app_id_value.as_u64().map(|id| id.to_string()) else {
            continue;
        };
        if let Some(achievements) = achievement_stats_from_json(stats_value) {
            stats.insert(app_id, achievements);
        }
    }

    stats
}

pub(crate) fn parse_steam_librarycache_achievements(
    contents: &str,
) -> Option<SteamAchievementStats> {
    let value = serde_json::from_str::<serde_json::Value>(contents).ok()?;
    let entries = value.as_array()?;
    for entry in entries {
        let pair = entry.as_array()?;
        let [key, payload] = pair.as_slice() else {
            continue;
        };
        if key.as_str() != Some("achievements") {
            continue;
        }
        if let Some(stats) = payload.get("data").and_then(achievement_stats_from_json) {
            return Some(stats);
        }
    }
    None
}

pub(crate) fn achievement_stats_from_json(
    value: &serde_json::Value,
) -> Option<SteamAchievementStats> {
    let unlocked = value
        .get("unlocked")
        .or_else(|| value.get("nAchieved"))?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())?;
    let total = value
        .get("total")
        .or_else(|| value.get("nTotal"))?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())?;
    if total == 0 || unlocked > total {
        return None;
    }
    Some(SteamAchievementStats { unlocked, total })
}

pub(crate) fn steam_library_dirs(steam_root: &FsPath) -> Vec<PathBuf> {
    let mut libraries = vec![steam_root.to_path_buf()];
    let libraryfolders = steam_root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(contents) = fs::read_to_string(libraryfolders) {
        libraries.extend(parse_steam_library_folders(&contents));
    }
    libraries.retain(|path| path.join("steamapps").is_dir());
    libraries.sort();
    libraries.dedup();
    libraries
}

pub(crate) fn parse_steam_library_folders(contents: &str) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut pending_block: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "{" {
            if let Some(block) = pending_block.take() {
                stack.push(block);
            }
            continue;
        }
        if line == "}" {
            stack.pop();
            continue;
        }

        let tokens = quoted_tokens(line);
        match tokens.as_slice() {
            [key] => pending_block = Some(key.to_string()),
            [key, value] => {
                pending_block = None;
                if key == "path"
                    || key.chars().all(|ch| ch.is_ascii_digit()) && looks_like_path(value)
                {
                    folders.push(PathBuf::from(value));
                }
            }
            _ => {}
        }
    }

    folders
}

pub(crate) fn looks_like_path(value: &str) -> bool {
    value.contains(":\\") || value.starts_with('/') || value.starts_with("\\\\")
}

pub(crate) fn collect_steam_app_manifests(libraries: &[PathBuf]) -> Vec<SteamAppManifest> {
    let mut manifests = Vec::new();
    for library in libraries.iter().take(16) {
        let steamapps = library.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten().take(2048) {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.starts_with("appmanifest_") || !file_name.ends_with(".acf") {
                continue;
            }
            if !fs::metadata(&path)
                .map(|metadata| metadata.is_file() && metadata.len() <= 256 * 1024)
                .unwrap_or(false)
            {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(manifest) = parse_steam_app_manifest(library, &contents) {
                manifests.push(manifest);
            }
        }
    }
    manifests
}

pub(crate) fn parse_steam_app_manifest(
    library: &FsPath,
    contents: &str,
) -> Option<SteamAppManifest> {
    let mut app_id = None;
    let mut name = None;
    let mut install_dir = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let tokens = quoted_tokens(line);
        if let [key, value] = tokens.as_slice() {
            match key.as_str() {
                "appid" => app_id = Some(value.to_string()),
                "name" => name = Some(value.to_string()),
                "installdir" => install_dir = Some(value.to_string()),
                _ => {}
            }
        }
    }

    let app_id = app_id?;
    let name = name.unwrap_or_else(|| format!("Steam app {app_id}"));
    let install_dir = install_dir?;
    let install_path = library.join("steamapps").join("common").join(&install_dir);
    Some(SteamAppManifest {
        app_id,
        name,
        install_dir,
        install_path,
    })
}

pub(crate) fn steam_manifest_matches_game(manifest: &SteamAppManifest, game: &GameModule) -> bool {
    game.steam_app_ids
        .iter()
        .any(|app_id| manifest.app_id == *app_id)
        || manifest.name.eq_ignore_ascii_case(game.display_name)
        || game
            .steam_install_dirs
            .iter()
            .any(|dir| manifest.install_dir.eq_ignore_ascii_case(dir))
}

pub(crate) fn find_steam_common_install_dir(
    libraries: &[PathBuf],
    game: &GameModule,
) -> Option<PathBuf> {
    for library in libraries {
        for install_dir in game.steam_install_dirs {
            let candidate = library.join("steamapps").join("common").join(install_dir);
            if candidate.is_dir() || game_executable_exists(&candidate, game) {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn discover_steam_artwork_paths(
    steam_root: &FsPath,
    app_id: &str,
) -> BTreeMap<String, PathBuf> {
    let mut paths = BTreeMap::new();
    for kind in ["icon", "banner", "hero", "capsule"] {
        if let Some(path) = steam_artwork_candidates(steam_root, app_id, kind)
            .into_iter()
            .find(|path| steam_artwork_file_usable(path))
        {
            paths.insert(kind.to_string(), path);
        }
    }
    paths
}

pub(crate) fn steam_artwork_candidates(
    steam_root: &FsPath,
    app_id: &str,
    kind: &str,
) -> Vec<PathBuf> {
    let cache = steam_root.join("appcache").join("librarycache");
    let app_cache = cache.join(app_id);
    let mut candidates = Vec::new();

    match kind {
        "icon" => {
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &["logo.png", "icon.jpg", "icon.png", "icon.ico"],
                true,
            ));
            candidates.extend([
                app_cache.join("icon.jpg"),
                app_cache.join("icon.png"),
                app_cache.join("icon.ico"),
                app_cache.join("logo.png"),
                cache.join(format!("{app_id}_icon.jpg")),
                cache.join(format!("{app_id}_icon.png")),
                steam_root
                    .join("steam")
                    .join("games")
                    .join(format!("{app_id}_icon.ico")),
            ]);
        }
        "banner" => {
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_header.jpg",
                    "library_header.png",
                    "header.jpg",
                    "header.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("header.jpg"),
                app_cache.join("header.png"),
                app_cache.join("library_header.jpg"),
                cache.join(format!("{app_id}_header.jpg")),
                cache.join(format!("{app_id}_header.png")),
            ]);
        }
        "hero" => {
            candidates.extend(custom_grid_candidates(steam_root, app_id, "hero"));
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_hero.jpg",
                    "library_hero.png",
                    "hero.jpg",
                    "hero.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("library_hero.jpg"),
                app_cache.join("library_hero.png"),
                app_cache.join("hero.jpg"),
                app_cache.join("hero.png"),
                cache.join(format!("{app_id}_library_hero.jpg")),
                cache.join(format!("{app_id}_library_hero.png")),
            ]);
        }
        "capsule" => {
            candidates.extend(custom_grid_candidates(steam_root, app_id, "capsule"));
            candidates.extend(steam_nested_artwork_candidates(
                &app_cache,
                &[
                    "library_capsule.jpg",
                    "library_capsule.png",
                    "library_600x900.jpg",
                    "library_600x900.png",
                ],
                false,
            ));
            candidates.extend([
                app_cache.join("library_600x900.jpg"),
                app_cache.join("library_600x900.png"),
                cache.join(format!("{app_id}_library_600x900.jpg")),
                cache.join(format!("{app_id}_library_600x900.png")),
            ]);
        }
        _ => {}
    }

    candidates
}

pub(crate) fn steam_nested_artwork_candidates(
    app_cache: &FsPath,
    file_names: &[&str],
    include_root_images: bool,
) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(app_cache) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    let mut root_images = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.push(path);
        } else if include_root_images && file_type.is_file() && steam_artwork_extension(&path) {
            root_images.push(path);
        }
    }

    dirs.sort();
    root_images.sort();

    let mut candidates = Vec::new();
    for dir in dirs {
        for file_name in file_names {
            candidates.push(dir.join(file_name));
        }
    }
    candidates.extend(root_images);
    candidates
}

pub(crate) fn steam_artwork_extension(path: &FsPath) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "ico"
    )
}

pub(crate) fn custom_grid_candidates(
    steam_root: &FsPath,
    app_id: &str,
    kind: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for user_dir in numeric_child_dirs(&steam_root.join("userdata"), 8) {
        let grid = user_dir.join("config").join("grid");
        match kind {
            "hero" => candidates.extend([
                grid.join(format!("{app_id}_hero.jpg")),
                grid.join(format!("{app_id}_hero.png")),
            ]),
            "capsule" => candidates.extend([
                grid.join(format!("{app_id}p.jpg")),
                grid.join(format!("{app_id}p.png")),
                grid.join(format!("{app_id}.jpg")),
                grid.join(format!("{app_id}.png")),
            ]),
            _ => {}
        }
    }
    candidates
}

pub(crate) fn steam_artwork_file_usable(path: &FsPath) -> bool {
    fs::metadata(path)
        .map(|metadata| {
            metadata.is_file() && metadata.len() > 0 && metadata.len() <= 10 * 1024 * 1024
        })
        .unwrap_or(false)
}

pub(crate) fn game_art_url(game_id: &str, kind: &str) -> String {
    format!("/api/games/art/{game_id}/{kind}")
}

pub(crate) fn steam_art_url_by_app(app_id: &str, kind: &str) -> String {
    format!("/api/games/steam-art/{app_id}/{kind}")
}

pub(crate) fn apply_steam_cdn_artwork_fallback(artwork: &mut GameArtwork, app_id: &str) {
    let base = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}");
    if artwork.banner_url.is_none() {
        artwork.banner_url = Some(format!("{base}/header.jpg"));
    }
    if artwork.hero_url.is_none() {
        artwork.hero_url = Some(format!("{base}/library_hero.jpg"));
    }
    if artwork.capsule_url.is_none() {
        artwork.capsule_url = Some(format!("{base}/library_600x900.jpg"));
    }
    if artwork.icon_url.is_none() {
        artwork.icon_url = artwork
            .capsule_url
            .clone()
            .or_else(|| artwork.banner_url.clone())
            .or_else(|| Some(format!("{base}/capsule_231x87.jpg")));
    }
}

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
    let candidate = GameDetectionCandidate {
        game_id: game.id.to_string(),
        name: game.display_name.to_string(),
        process_name: format!("{adapter_id}:telemetry"),
        module_id: game.id.to_string(),
        adapter_id: game.adapter_id.to_string(),
        profile_id: game.default_profile_id.to_string(),
        confidence: 70,
    };

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
        .filter(|game| game.adapter_id == adapter_id)
        .collect();
    if modules.is_empty() {
        return None;
    }

    if let Some(game_id) = inner.telemetry.text("game.id") {
        if let Some(game) = modules.iter().find(|game| game.id == game_id) {
            return Some(*game);
        }
    }

    let installed: Vec<&SupportedGameSummary> = catalog
        .supported_games
        .iter()
        .filter(|summary| {
            summary.installed
                && modules
                    .iter()
                    .any(|game| game.id == summary.game_id.as_str())
        })
        .collect();
    if installed.len() == 1 {
        let game_id = installed[0].game_id.as_str();
        if let Some(game) = modules.iter().find(|game| game.id == game_id) {
            return Some(*game);
        }
    }

    modules.first().copied()
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

pub(crate) fn user_game_artwork_for_app(steam_root: &FsPath, app_id: &str) -> GameArtwork {
    let mut artwork = GameArtwork::default();
    // Prefer the local Steam librarycache (what Steam actually shows in-client).
    // Routes back through /api/games/steam-art/:app_id/:kind which streams the
    // file from disk on demand. Many Steam apps lack public CDN capsules, so the
    // local cache is the most reliable source for the user's actual library.
    for kind in discover_steam_artwork_paths(steam_root, app_id).keys() {
        let url = steam_art_url_by_app(app_id, kind);
        match kind.as_str() {
            "icon" => artwork.icon_url = Some(url),
            "banner" => artwork.banner_url = Some(url),
            "hero" => artwork.hero_url = Some(url),
            "capsule" => artwork.capsule_url = Some(url),
            _ => {}
        }
    }
    // Fill remaining slots from Steam's CDN. apply_steam_cdn_artwork_fallback
    // only writes is_none() fields, so this preserves the local-cache choices.
    apply_steam_cdn_artwork_fallback(&mut artwork, app_id);
    artwork
}

/// Locate the configured Steam root (if any) and read per-app stats. Designed
/// to be run inside `spawn_blocking` so it does not block the async runtime.
pub(crate) fn steam_root_and_stats_for_user_games(
) -> (Option<PathBuf>, BTreeMap<String, SteamGameStats>) {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return (None, BTreeMap::new());
    };
    let stats = discover_steam_game_stats(&steam_root);
    (Some(steam_root), stats)
}

/// Look up a Steam app manifest by app_id across the entire library set.
/// Returns `None` if Steam isn't installed or the manifest can't be found.
pub(crate) fn locate_steam_manifest(app_id: &str) -> Option<SteamAppManifest> {
    let steam_root = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())?;
    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    manifests
        .into_iter()
        .find(|manifest| manifest.app_id == app_id)
}

/// Returns every Steam library entry that the agent can see on disk, with a
/// flag marking entries that are already represented by a built-in module or
/// a previously-added user game.
pub(crate) fn discover_steam_library_entries(
    user_games: &BTreeMap<String, UserGameConfig>,
) -> Vec<SteamLibraryEntry> {
    let Some(steam_root) = steam_root_candidates()
        .into_iter()
        .find(|path| path.join("steamapps").is_dir() || path.join("steam.exe").is_file())
    else {
        return Vec::new();
    };

    let libraries = steam_library_dirs(&steam_root);
    let manifests = collect_steam_app_manifests(&libraries);
    let steam_stats = discover_steam_game_stats(&steam_root);
    let built_in_app_ids: std::collections::BTreeSet<&str> = built_in_game_modules()
        .iter()
        .flat_map(|game| game.steam_app_ids.iter().copied())
        .collect();

    let mut entries = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let suggested_game_id = user_game_id_for_app_id(&manifest.app_id);
        let already_in_catalog = built_in_app_ids.contains(manifest.app_id.as_str())
            || user_games.contains_key(&suggested_game_id);
        let artwork = user_game_artwork_for_app(&steam_root, &manifest.app_id);
        let stats = steam_stats
            .get(manifest.app_id.as_str())
            .cloned()
            .unwrap_or_default();
        let process_candidates = discover_user_game_process_candidates(&manifest.install_path);
        entries.push(SteamLibraryEntry {
            app_id: manifest.app_id.clone(),
            name: manifest.name.clone(),
            install_dir: manifest.install_dir.clone(),
            install_path: manifest.install_path.display().to_string(),
            artwork,
            stats,
            already_in_catalog,
            suggested_game_id,
            process_candidates,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    entries
}

/// Build a `SupportedGameSummary` entry for a user-registered game, suitable
/// for appending to the snapshot's `supported_games` list.
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

#[cfg(target_os = "windows")]
pub(crate) fn windows_process_names() -> io::Result<Vec<String>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut names = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let process_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if !process_name.is_empty() {
                    names.push(process_name);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        Ok(names)
    }
}

#[cfg(all(target_os = "windows", not(test)))]
pub(crate) fn windows_process_image_paths_matching(targets: &[String]) -> io::Result<Vec<PathBuf>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut paths = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let process_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if targets
                    .iter()
                    .any(|target| target.eq_ignore_ascii_case(&process_name))
                {
                    let process =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    if !process.is_null() {
                        let mut buffer = [0_u16; 32768];
                        let mut size = buffer.len() as u32;
                        if QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size)
                            != 0
                            && size > 0
                        {
                            paths.push(PathBuf::from(String::from_utf16_lossy(
                                &buffer[..size as usize],
                            )));
                        }
                        CloseHandle(process);
                    }
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        Ok(paths)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_process_running(target: &str) -> bool {
    windows_process_names()
        .map(|names| {
            names
                .iter()
                .any(|process_name| process_name.eq_ignore_ascii_case(target))
        })
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) async fn detect_running_game(
    _user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    no_game_detection("none")
}

#[cfg(not(test))]
pub(crate) async fn detect_running_game(
    user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    if std::env::var_os("DSCC_DISABLE_PROCESS_SCAN").is_some() {
        return no_game_detection("process_scan_disabled");
    }

    match current_process_names().await {
        Ok(processes) => detect_running_game_from_processes_with_user_games(
            processes.iter().map(String::as_str),
            user_games,
        ),
        Err(error) => GameDetectionResponse {
            active_game_id: None,
            active_game_name: None,
            source: "process_scan_unavailable".to_string(),
            confidence: 0,
            process_name: None,
            module_id: None,
            adapter_id: None,
            profile_id: None,
            candidates: Vec::new(),
            supported_games: Vec::new(),
            selected_game: None,
        }
        .with_source_detail(error.to_string()),
    }
}

#[cfg(not(test))]
pub(crate) trait GameDetectionSourceDetail {
    fn with_source_detail(self, detail: String) -> Self;
}

#[cfg(not(test))]
impl GameDetectionSourceDetail for GameDetectionResponse {
    fn with_source_detail(mut self, detail: String) -> Self {
        self.source = format!("{}:{detail}", self.source);
        self
    }
}

#[cfg(not(test))]
pub(crate) async fn current_process_names() -> io::Result<Vec<String>> {
    #[cfg(target_os = "windows")]
    {
        windows_process_names()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = tokio::process::Command::new("ps")
            .args(["-eo", "comm=", "-eo", "args="])
            .output()
            .await?;
        if !output.status.success() {
            return Err(io::Error::other("ps did not complete successfully"));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_unix_process_names(&text))
    }
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn parse_unix_process_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        for token in line.split_whitespace() {
            push_process_name_candidates(&mut names, &mut seen, token);
        }
    }
    names
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn push_process_name_candidates(
    names: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    raw: &str,
) {
    for candidate in process_name_candidates(raw) {
        let key = candidate.to_ascii_lowercase();
        if seen.insert(key) {
            names.push(candidate);
        }
    }
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn process_name_candidates(raw: &str) -> Vec<String> {
    let trimmed = raw.trim_matches(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';'
            )
    });
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    push_process_name_candidate(&mut candidates, trimmed);

    let normalized = trimmed.replace('\\', "/");
    if let Some(base) = normalized.rsplit('/').next() {
        push_process_name_candidate(&mut candidates, base);
    }

    let lower = normalized.to_ascii_lowercase();
    if let Some(exe_end) = lower.find(".exe").map(|index| index + 4) {
        let exe_path = &normalized[..exe_end];
        if let Some(base) = exe_path.rsplit('/').next() {
            push_process_name_candidate(&mut candidates, base);
        }
    }

    candidates
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn push_process_name_candidate(candidates: &mut Vec<String>, value: &str) {
    let candidate = value.trim();
    if candidate.is_empty() {
        return;
    }
    if !candidates
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(candidate))
    {
        candidates.push(candidate.to_string());
    }
}

pub(crate) async fn get_detected_game(
    State(state): State<AgentState>,
) -> Json<GameDetectionResponse> {
    Json(state.cached_game_detection().await)
}

pub(crate) async fn get_game_art(
    Path((game_id, kind)): Path<(String, String)>,
    State(state): State<AgentState>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["icon", "banner", "hero", "capsule"].contains(&kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let catalog = state.cached_steam_game_catalog().await;
    let path = catalog
        .artwork_paths
        .get(&(game_id, kind))
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    if !steam_artwork_file_usable(&path) {
        return Err(StatusCode::NOT_FOUND);
    }

    let content_type = artwork_content_type(&path);
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes))
}

/// Serves a single artwork file out of Steam's local librarycache, keyed by the
/// numeric `app_id`. This is what `user_game_artwork_for_app` points its URLs
/// at — many Steam apps lack public CDN capsules but always have the local
/// renders Steam uses in-client.
pub(crate) async fn get_steam_app_art(
    Path((app_id, kind)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    if !["icon", "banner", "hero", "capsule"].contains(&kind.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    // app_id must be purely digits — guards against path traversal and tells us
    // the request is for a real Steam manifest, not an arbitrary string.
    if app_id.is_empty() || !app_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = tokio::task::spawn_blocking(move || {
        steam_root_candidates()
            .into_iter()
            .find(|root| root.join("steamapps").is_dir() || root.join("steam.exe").is_file())
            .and_then(|root| {
                discover_steam_artwork_paths(&root, &app_id)
                    .remove(&kind)
                    .filter(|path| steam_artwork_file_usable(path))
            })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let content_type = artwork_content_type(&path);
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(([(header::CONTENT_TYPE, content_type)], bytes))
}

pub(crate) async fn list_steam_library(
    State(state): State<AgentState>,
) -> Json<SteamLibraryListResponse> {
    let user_games = {
        let inner = state.inner.read().await;
        inner.user_games.clone()
    };
    let games = tokio::task::spawn_blocking(move || discover_steam_library_entries(&user_games))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "Steam library discovery task failed");
            Vec::new()
        });
    Json(SteamLibraryListResponse { games })
}

/// Maximum entries returned by a single browse request. Larger directories are
/// truncated and the response sets `truncated: true` so the UI can warn.
pub(crate) const STEAM_LIBRARY_BROWSE_LIMIT: usize = 400;

/// Sandboxed directory listing for a Steam-installed game. The endpoint never
/// resolves outside the game's install path; it confirms the resolved path is
/// a prefix-equal of the install root after canonicalisation, so symlinks and
/// `..` traversal can't escape the game folder.
pub(crate) async fn browse_steam_library(
    Query(params): Query<BrowseSteamLibraryParams>,
) -> Result<Json<SteamLibraryBrowseResponse>, (StatusCode, Json<serde_json::Value>)> {
    let app_id = params.app_id.trim().to_string();
    if app_id.is_empty() || !app_id.chars().all(|ch| ch.is_ascii_digit()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId must be a numeric Steam app id"})),
        ));
    }

    let requested_rel = params.path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let Some(manifest) = locate_steam_manifest(&app_id) else {
            return Err(BrowseError::NotFound);
        };
        let install_root = manifest
            .install_path
            .canonicalize()
            .map_err(|_| BrowseError::NotFound)?;
        let target = resolve_browse_target(&install_root, &requested_rel)?;
        if !target.is_dir() {
            return Err(BrowseError::NotADirectory);
        }
        let (entries, truncated) = read_browse_entries(&target);
        let relative_path = path_relative_to(&install_root, &target);
        Ok(SteamLibraryBrowseResponse {
            app_id: manifest.app_id,
            install_path: install_root.display().to_string(),
            relative_path,
            entries,
            truncated,
        })
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "directory scan task failed"})),
        )
    })?;

    match result {
        Ok(response) => Ok(Json(response)),
        Err(BrowseError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Steam app manifest or install folder not found"})),
        )),
        Err(BrowseError::OutsideRoot) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "path escapes the install folder (rejected)"})),
        )),
        Err(BrowseError::NotADirectory) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "requested path is not a directory"})),
        )),
    }
}

pub(crate) enum BrowseError {
    NotFound,
    OutsideRoot,
    NotADirectory,
}

/// Resolves `relative` against `root` and asserts the canonical result is
/// under `root`. Empty/whitespace relative paths return `root` itself. Forward
/// or backward slashes are accepted; `..` segments are blocked.
pub(crate) fn resolve_browse_target(root: &FsPath, relative: &str) -> Result<PathBuf, BrowseError> {
    let trimmed = relative.trim().trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(root.to_path_buf());
    }
    let mut combined = root.to_path_buf();
    for segment in trimmed.split(['/', '\\']) {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." || segment.contains('\0') {
            return Err(BrowseError::OutsideRoot);
        }
        combined.push(segment);
    }
    let canonical = combined.canonicalize().map_err(|_| BrowseError::NotFound)?;
    if !canonical.starts_with(root) {
        return Err(BrowseError::OutsideRoot);
    }
    Ok(canonical)
}

/// Returns the forward-slashed path of `target` relative to `root`, or an
/// empty string if they are equal. Caller has already confirmed `target` is
/// inside `root`.
pub(crate) fn path_relative_to(root: &FsPath, target: &FsPath) -> String {
    target
        .strip_prefix(root)
        .map(|rel| {
            rel.components()
                .filter_map(|component| component.as_os_str().to_str().map(String::from))
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

pub(crate) fn read_browse_entries(dir: &FsPath) -> (Vec<SteamLibraryBrowseEntry>, bool) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return (Vec::new(), false);
    };

    let mut dirs: Vec<SteamLibraryBrowseEntry> = Vec::new();
    let mut exes: Vec<SteamLibraryBrowseEntry> = Vec::new();
    let mut truncated = false;

    for entry in read_dir.flatten() {
        if dirs.len() + exes.len() >= STEAM_LIBRARY_BROWSE_LIMIT {
            truncated = true;
            break;
        }
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        // Hide dotfiles and the Steam-managed sentinel files; the user is
        // never going to pick those as a launch executable.
        if name.starts_with('.') || name.eq_ignore_ascii_case("steam_appid.txt") {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.push(SteamLibraryBrowseEntry {
                name,
                kind: "dir".to_string(),
                size_bytes: None,
            });
        } else if file_type.is_file() && name.to_ascii_lowercase().ends_with(".exe") {
            let size_bytes = entry.metadata().ok().map(|metadata| metadata.len());
            exes.push(SteamLibraryBrowseEntry {
                name,
                kind: "exe".to_string(),
                size_bytes,
            });
        }
    }

    dirs.sort_by_key(|entry| entry.name.to_ascii_lowercase());
    exes.sort_by_key(|entry| entry.name.to_ascii_lowercase());

    let mut combined = dirs;
    combined.extend(exes);
    (combined, truncated)
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
        inner.logs.push(LogEntry {
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
    if built_in_game_modules()
        .iter()
        .any(|module| module.id == game_id)
    {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "A built-in module already covers this gameId",
                "gameId": game_id,
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
        inner.logs.push(LogEntry {
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
        inner.logs.push(LogEntry {
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

pub(crate) fn artwork_content_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
