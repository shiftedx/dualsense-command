use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};

use crate::{
    GameArtwork, GameDetectionCandidate, GameDetectionResponse, ModuleSummary, SteamGameStats,
    SupportedGameSummary, UserGameConfig,
};

pub(crate) const FORZA_DATA_OUT_ADAPTER_ID: &str = "forza-data-out";
pub(crate) const ASSETTO_SHARED_MEMORY_ADAPTER_ID: &str = "assetto-shared-memory";
pub(crate) const FORZA_HORIZON_PROFILE_ID: &str = "forza-horizon";
pub(crate) const FORZA_HORIZON_IMMERSIVE_PROFILE_ID: &str = "forza-horizon-immersive";
pub(crate) const ASSETTO_CORSA_RALLY_PROFILE_ID: &str = "assetto-corsa-rally";
pub(crate) const ASSETTO_CORSA_RALLY_STEAM_APP_ID: &str = "3917090";
pub(crate) const FORZA_HORIZON5_STEAM_APP_ID: &str = "1551360";
pub(crate) const FORZA_HORIZON6_STEAM_APP_ID: &str = "2483190";

const FORZA_HORIZON_PROFILE_TEMPLATES: &[&str] = &["Base", "Immersive"];
const ASSETTO_CORSA_RALLY_PROFILE_TEMPLATES: &[&str] = &["Rally"];

#[derive(Clone, Copy, Debug)]
pub(crate) struct GameModule {
    pub(crate) id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) adapter_id: &'static str,
    pub(crate) default_profile_id: &'static str,
    pub(crate) process_names: &'static [&'static str],
    pub(crate) steam_app_ids: &'static [&'static str],
    pub(crate) steam_install_dirs: &'static [&'static str],
    pub(crate) steam_catalog: bool,
    pub(crate) profile_templates: &'static [&'static str],
    pub(crate) detection_lightbar_color: &'static str,
    pub(crate) detection_lightbar_brightness: u8,
}

impl GameModule {
    pub(crate) fn profile_template_names(self) -> Vec<String> {
        self.profile_templates
            .iter()
            .map(|template| (*template).to_string())
            .collect()
    }
}

const BUILT_IN_GAME_MODULES: &[GameModule] = &[
    GameModule {
        id: "forza-horizon-6",
        display_name: "Forza Horizon 6",
        adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        default_profile_id: FORZA_HORIZON_IMMERSIVE_PROFILE_ID,
        process_names: &[
            "ForzaHorizon6.exe",
            "ForzaHorizon6-WinGDK-Shipping.exe",
            "ForzaHorizon6_Steam.exe",
        ],
        steam_app_ids: &[FORZA_HORIZON6_STEAM_APP_ID],
        steam_install_dirs: &["ForzaHorizon6"],
        steam_catalog: true,
        profile_templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        detection_lightbar_color: "#00a8ff",
        detection_lightbar_brightness: 58,
    },
    GameModule {
        id: "forza-horizon-5",
        display_name: "Forza Horizon 5",
        adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        default_profile_id: FORZA_HORIZON_IMMERSIVE_PROFILE_ID,
        process_names: &[
            "ForzaHorizon5.exe",
            "ForzaHorizon5-Win64-Shipping.exe",
            "ForzaHorizon5_Steam.exe",
        ],
        steam_app_ids: &[FORZA_HORIZON5_STEAM_APP_ID],
        steam_install_dirs: &["ForzaHorizon5"],
        steam_catalog: true,
        profile_templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        detection_lightbar_color: "#00a8ff",
        detection_lightbar_brightness: 58,
    },
    GameModule {
        id: "forza-motorsport",
        display_name: "Forza Motorsport",
        adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        default_profile_id: FORZA_HORIZON_PROFILE_ID,
        process_names: &["ForzaMotorsport.exe", "ForzaMotorsport-WinGDK-Shipping.exe"],
        steam_app_ids: &["2483190"],
        steam_install_dirs: &[],
        steam_catalog: false,
        profile_templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        detection_lightbar_color: "#00a8ff",
        detection_lightbar_brightness: 58,
    },
    GameModule {
        id: "assetto-corsa-rally",
        display_name: "Assetto Corsa Rally",
        adapter_id: ASSETTO_SHARED_MEMORY_ADAPTER_ID,
        default_profile_id: ASSETTO_CORSA_RALLY_PROFILE_ID,
        process_names: &["acr.exe"],
        steam_app_ids: &[ASSETTO_CORSA_RALLY_STEAM_APP_ID],
        steam_install_dirs: &["Assetto Corsa Rally"],
        steam_catalog: true,
        profile_templates: ASSETTO_CORSA_RALLY_PROFILE_TEMPLATES,
        detection_lightbar_color: "#ff3b30",
        detection_lightbar_brightness: 62,
    },
];

pub(crate) fn built_in_game_modules() -> &'static [GameModule] {
    BUILT_IN_GAME_MODULES
}

pub(crate) fn game_module_summaries() -> Vec<ModuleSummary> {
    BUILT_IN_GAME_MODULES
        .iter()
        .map(|game| ModuleSummary {
            id: game.id.to_string(),
            name: game.display_name.to_string(),
            kind: "game".to_string(),
            version: "builtin".to_string(),
            source: "built_in_game".to_string(),
            trusted: true,
            protocol: format!("game:{}", game.adapter_id),
            setup_hint: format!(
                "Uses the {} adapter with game-specific detection and profile metadata.",
                game.adapter_id
            ),
            setup_url: None,
            profile_templates: (*game).profile_template_names(),
        })
        .collect()
}

pub(crate) fn no_game_detection(source: &str) -> GameDetectionResponse {
    GameDetectionResponse {
        active_game_id: None,
        active_game_name: None,
        source: source.to_string(),
        confidence: 0,
        process_name: None,
        module_id: None,
        adapter_id: None,
        profile_id: None,
        candidates: Vec::new(),
        supported_games: Vec::new(),
        selected_game: None,
    }
}

#[cfg(test)]
pub(crate) fn detect_running_game_from_processes<'a, I>(processes: I) -> GameDetectionResponse
where
    I: IntoIterator<Item = &'a str>,
{
    detect_running_game_from_processes_with_user_games(processes, &BTreeMap::new())
}

pub(crate) fn detect_running_game_from_processes_with_user_games<'a, I>(
    processes: I,
    user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse
where
    I: IntoIterator<Item = &'a str>,
{
    let mut candidates = Vec::new();
    for process in processes {
        for game in BUILT_IN_GAME_MODULES {
            if game
                .process_names
                .iter()
                .any(|known| known.eq_ignore_ascii_case(process))
            {
                candidates.push(GameDetectionCandidate {
                    game_id: game.id.to_string(),
                    name: game.display_name.to_string(),
                    process_name: process.to_string(),
                    module_id: game.id.to_string(),
                    adapter_id: game.adapter_id.to_string(),
                    profile_id: game.default_profile_id.to_string(),
                    confidence: 82,
                });
            }
        }
        for user_game in user_games.values() {
            if user_game
                .process_names
                .iter()
                .any(|known| known.eq_ignore_ascii_case(process))
            {
                candidates.push(GameDetectionCandidate {
                    game_id: user_game.game_id.clone(),
                    name: user_game.name.clone(),
                    process_name: process.to_string(),
                    module_id: user_game.game_id.clone(),
                    adapter_id: String::new(),
                    profile_id: String::new(),
                    confidence: 78,
                });
            }
        }
    }

    let active = candidates.first().cloned();
    GameDetectionResponse {
        active_game_id: active.as_ref().map(|candidate| candidate.game_id.clone()),
        active_game_name: active.as_ref().map(|candidate| candidate.name.clone()),
        source: if active.is_some() {
            "process_scan".to_string()
        } else {
            "none".to_string()
        },
        confidence: active.as_ref().map_or(0, |candidate| candidate.confidence),
        process_name: active
            .as_ref()
            .map(|candidate| candidate.process_name.clone()),
        module_id: active.as_ref().map(|candidate| candidate.module_id.clone()),
        adapter_id: active
            .as_ref()
            .map(|candidate| candidate.adapter_id.clone())
            .filter(|adapter| !adapter.is_empty()),
        profile_id: active
            .as_ref()
            .map(|candidate| candidate.profile_id.clone())
            .filter(|profile| !profile.is_empty()),
        candidates,
        supported_games: Vec::new(),
        selected_game: None,
    }
}

pub(crate) fn supported_game_summary(
    game: &GameModule,
    app_id: Option<String>,
    install_path: Option<PathBuf>,
    artwork: GameArtwork,
    stats: SteamGameStats,
) -> SupportedGameSummary {
    let installed = install_path
        .as_ref()
        .is_some_and(|path| path.is_dir() || game_executable_exists(path, game));
    SupportedGameSummary {
        game_id: game.id.to_string(),
        name: game.display_name.to_string(),
        source: "built_in".to_string(),
        input_provider: "native_dualsense".to_string(),
        app_id,
        install_path: install_path.map(|path| path.display().to_string()),
        process_names: game
            .process_names
            .iter()
            .map(|process| (*process).to_string())
            .collect(),
        executable_name: game
            .process_names
            .first()
            .map(|process| (*process).to_string()),
        installed,
        running: false,
        support_level: "telemetry".to_string(),
        artwork,
        stats,
    }
}

pub(crate) fn game_executable_exists(root: &FsPath, game: &GameModule) -> bool {
    game.process_names
        .iter()
        .any(|process| root.join(process).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn built_in_game_modules_have_unique_ids_and_required_metadata() {
        let mut ids = BTreeSet::new();
        for game in built_in_game_modules() {
            assert!(ids.insert(game.id), "duplicate game module id {}", game.id);
            assert!(!game.display_name.is_empty());
            assert!(!game.adapter_id.is_empty());
            assert!(!game.default_profile_id.is_empty());
            assert!(
                !game.process_names.is_empty(),
                "{} must have process detection metadata",
                game.id
            );
            assert!(
                !game.profile_templates.is_empty(),
                "{} must expose at least one profile template",
                game.id
            );
        }
    }

    #[test]
    fn forza_games_are_distinct_modules_with_a_shared_adapter() {
        let game_module_by_id =
            |id: &str| built_in_game_modules().iter().find(|game| game.id == id);
        let forza_games = [
            game_module_by_id("forza-horizon-6").unwrap(),
            game_module_by_id("forza-horizon-5").unwrap(),
            game_module_by_id("forza-motorsport").unwrap(),
        ];
        let ids = forza_games
            .iter()
            .map(|game| game.id)
            .collect::<BTreeSet<_>>();

        assert_eq!(ids.len(), forza_games.len());
        assert!(forza_games
            .iter()
            .all(|game| game.adapter_id == FORZA_DATA_OUT_ADAPTER_ID));
    }

    #[test]
    fn assetto_corsa_rally_is_a_distinct_shared_memory_module() {
        let game = built_in_game_modules()
            .iter()
            .find(|game| game.id == "assetto-corsa-rally")
            .expect("Assetto Corsa Rally module exists");

        assert_eq!(game.adapter_id, ASSETTO_SHARED_MEMORY_ADAPTER_ID);
        assert_eq!(game.default_profile_id, ASSETTO_CORSA_RALLY_PROFILE_ID);
        assert_eq!(game.steam_app_ids, &[ASSETTO_CORSA_RALLY_STEAM_APP_ID]);
        assert!(game
            .process_names
            .iter()
            .any(|process| process.eq_ignore_ascii_case("acr.exe")));
    }

    #[test]
    fn process_detection_uses_game_module_metadata() {
        let detection = detect_running_game_from_processes(["ForzaHorizon5.exe"]);

        assert_eq!(detection.active_game_id.as_deref(), Some("forza-horizon-5"));
        assert_eq!(detection.module_id.as_deref(), Some("forza-horizon-5"));
        assert_eq!(
            detection.adapter_id.as_deref(),
            Some(FORZA_DATA_OUT_ADAPTER_ID)
        );
        assert_eq!(
            detection.profile_id.as_deref(),
            Some(FORZA_HORIZON_IMMERSIVE_PROFILE_ID)
        );
    }

    #[test]
    fn process_detection_matches_assetto_corsa_rally() {
        let detection = detect_running_game_from_processes(["acr.exe"]);

        assert_eq!(
            detection.active_game_id.as_deref(),
            Some("assetto-corsa-rally")
        );
        assert_eq!(
            detection.adapter_id.as_deref(),
            Some(ASSETTO_SHARED_MEMORY_ADAPTER_ID)
        );
        assert_eq!(
            detection.profile_id.as_deref(),
            Some(ASSETTO_CORSA_RALLY_PROFILE_ID)
        );
    }
}
