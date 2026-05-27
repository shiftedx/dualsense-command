use super::*;

mod catalog;
mod local_apps;
mod process_scanning;
mod steam;
mod types;

pub(crate) use catalog::{
    append_user_games_to_detection, enrich_game_detection, get_detected_game,
    supported_game_install_path, telemetry_game_detection,
};
#[cfg(all(test, target_os = "windows"))]
pub(crate) use local_apps::local_app_process_path_allowed;
#[cfg(test)]
pub(crate) use local_apps::USER_GAME_PROCESS_CANDIDATE_LIMIT;
pub(crate) use local_apps::{
    add_custom_game, add_local_game, detection_allows_input_bridge,
    discover_user_game_process_candidates, local_app_execution_verified_for_input_bridge,
    remove_custom_game, user_game_id_for_app_id, user_game_to_supported_summary,
    validate_local_game,
};
pub(crate) use process_scanning::detect_running_game;
#[cfg(test)]
pub(crate) use process_scanning::parse_unix_process_names;
#[cfg(target_os = "windows")]
pub(crate) use process_scanning::windows_process_running;
pub(crate) use steam::{
    apply_steam_cdn_artwork_fallback, browse_steam_library, discover_steam_game_catalog,
    get_game_art, get_steam_app_art, list_steam_library, locate_steam_manifest,
    steam_root_and_stats_for_user_games, unsupported_steam_game_catalog, user_game_artwork_for_app,
};
#[cfg(test)]
pub(crate) use steam::{
    build_supported_steam_game_catalog, parse_steam_achievement_progress_cache,
    parse_steam_app_manifest, parse_steam_library_folders, parse_steam_localconfig_stats,
};
pub use types::{
    AddLocalGameRequest, AddUserGameRequest, AddUserGameResponse, BrowseSteamLibraryParams,
    GameArtwork, GameDetectionCandidate, GameDetectionResponse, SteamAchievementStats,
    SteamGameStats, SteamLibraryBrowseEntry, SteamLibraryBrowseResponse, SteamLibraryEntry,
    SteamLibraryListResponse, SupportedGameSummary, UserGameConfig, ValidateLocalGameRequest,
    ValidateLocalGameResponse,
};
pub(crate) use types::{SteamAppManifest, SteamGameCatalog};
