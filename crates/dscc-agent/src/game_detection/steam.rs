use super::*;
use crate::steam_input::{numeric_child_dirs, quoted_tokens};

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

    let content_type = self::artwork_content_type(&path);
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

    let content_type = self::artwork_content_type(&path);
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
