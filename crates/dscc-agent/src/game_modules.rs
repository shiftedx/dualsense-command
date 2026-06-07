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
    pub(crate) source: &'static str,
    pub(crate) input_provider: &'static str,
    pub(crate) support_level: &'static str,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleIdentity {
    pub(crate) id: &'static str,
    pub(crate) display_name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleTelemetryLink {
    pub(crate) adapter_id: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleProfileMetadata {
    pub(crate) default_profile_id: &'static str,
    pub(crate) templates: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleDetectionMetadata {
    pub(crate) process_names: &'static [&'static str],
    pub(crate) steam_app_ids: &'static [&'static str],
    pub(crate) steam_install_dirs: &'static [&'static str],
    pub(crate) steam_catalog: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleLightbarCue {
    pub(crate) color_hex: &'static str,
    pub(crate) brightness_percent: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GameModulePresentationMetadata {
    pub(crate) source: &'static str,
    pub(crate) input_provider: &'static str,
    pub(crate) support_level: &'static str,
    pub(crate) detection_lightbar: GameModuleLightbarCue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GameModuleSetupMetadata {
    pub(crate) protocol: String,
    pub(crate) setup_hint: String,
}

impl GameModule {
    pub(crate) const fn from_supported_game_capabilities(
        identity: GameModuleIdentity,
        telemetry: GameModuleTelemetryLink,
        profiles: GameModuleProfileMetadata,
        detection: GameModuleDetectionMetadata,
        presentation: GameModulePresentationMetadata,
    ) -> Self {
        Self {
            id: identity.id,
            display_name: identity.display_name,
            source: presentation.source,
            input_provider: presentation.input_provider,
            support_level: presentation.support_level,
            adapter_id: telemetry.adapter_id,
            default_profile_id: profiles.default_profile_id,
            process_names: detection.process_names,
            steam_app_ids: detection.steam_app_ids,
            steam_install_dirs: detection.steam_install_dirs,
            steam_catalog: detection.steam_catalog,
            profile_templates: profiles.templates,
            detection_lightbar_color: presentation.detection_lightbar.color_hex,
            detection_lightbar_brightness: presentation.detection_lightbar.brightness_percent,
        }
    }

    pub(crate) fn identity(self) -> GameModuleIdentity {
        GameModuleIdentity {
            id: self.id,
            display_name: self.display_name,
        }
    }

    pub(crate) fn telemetry_link(self) -> GameModuleTelemetryLink {
        GameModuleTelemetryLink {
            adapter_id: self.adapter_id,
        }
    }

    pub(crate) fn profile_metadata(self) -> GameModuleProfileMetadata {
        GameModuleProfileMetadata {
            default_profile_id: self.default_profile_id,
            templates: self.profile_templates,
        }
    }

    pub(crate) fn detection_metadata(self) -> GameModuleDetectionMetadata {
        GameModuleDetectionMetadata {
            process_names: self.process_names,
            steam_app_ids: self.steam_app_ids,
            steam_install_dirs: self.steam_install_dirs,
            steam_catalog: self.steam_catalog,
        }
    }

    pub(crate) fn presentation_metadata(self) -> GameModulePresentationMetadata {
        GameModulePresentationMetadata {
            source: self.source,
            input_provider: self.input_provider,
            support_level: self.support_level,
            detection_lightbar: self.detection_lightbar(),
        }
    }

    pub(crate) fn detection_lightbar(self) -> GameModuleLightbarCue {
        GameModuleLightbarCue {
            color_hex: self.detection_lightbar_color,
            brightness_percent: self.detection_lightbar_brightness,
        }
    }

    pub(crate) fn setup_metadata(self) -> GameModuleSetupMetadata {
        GameModuleSetupMetadata {
            protocol: format!("game:{}", self.adapter_id),
            setup_hint: format!(
                "Uses the {} adapter with game-specific detection and profile metadata.",
                self.adapter_id
            ),
        }
    }

    pub(crate) fn profile_template_names(self) -> Vec<String> {
        self.profile_metadata()
            .templates
            .iter()
            .map(|template| (*template).to_string())
            .collect()
    }

    pub(crate) fn module_summary(self) -> ModuleSummary {
        let identity = self.identity();
        let setup = self.setup_metadata();
        ModuleSummary {
            id: identity.id.to_string(),
            name: identity.display_name.to_string(),
            kind: "game".to_string(),
            version: "builtin".to_string(),
            source: "built_in_game".to_string(),
            trusted: true,
            protocol: setup.protocol,
            setup_hint: setup.setup_hint,
            setup_url: None,
            profile_templates: self.profile_template_names(),
        }
    }

    pub(crate) fn matches_process(self, process_name: &str) -> bool {
        self.detection_metadata()
            .process_names
            .iter()
            .any(|known| known.eq_ignore_ascii_case(process_name))
    }

    pub(crate) fn process_detection_candidate(
        self,
        process_name: &str,
        confidence: u8,
    ) -> Option<GameDetectionCandidate> {
        self.matches_process(process_name)
            .then(|| self.detection_candidate(process_name.to_string(), confidence))
    }

    pub(crate) fn telemetry_detection_candidate(
        self,
        adapter_id: &str,
        confidence: u8,
    ) -> GameDetectionCandidate {
        debug_assert_eq!(self.adapter_id, adapter_id);
        self.detection_candidate(format!("{adapter_id}:telemetry"), confidence)
    }

    fn detection_candidate(self, process_name: String, confidence: u8) -> GameDetectionCandidate {
        let identity = self.identity();
        let telemetry = self.telemetry_link();
        let profiles = self.profile_metadata();
        GameDetectionCandidate {
            game_id: identity.id.to_string(),
            name: identity.display_name.to_string(),
            process_name,
            module_id: identity.id.to_string(),
            adapter_id: telemetry.adapter_id.to_string(),
            profile_id: profiles.default_profile_id.to_string(),
            confidence,
        }
    }

    pub(crate) fn supported_summary(
        self,
        app_id: Option<String>,
        install_path: Option<PathBuf>,
        artwork: GameArtwork,
        stats: SteamGameStats,
    ) -> SupportedGameSummary {
        let identity = self.identity();
        let detection = self.detection_metadata();
        let presentation = self.presentation_metadata();
        let installed = install_path
            .as_ref()
            .is_some_and(|path| path.is_dir() || self.executable_exists(path));
        SupportedGameSummary {
            game_id: identity.id.to_string(),
            name: identity.display_name.to_string(),
            source: presentation.source.to_string(),
            input_provider: presentation.input_provider.to_string(),
            app_id,
            install_path: install_path.map(|path| path.display().to_string()),
            process_names: detection
                .process_names
                .iter()
                .map(|process| (*process).to_string())
                .collect(),
            executable_name: detection
                .process_names
                .first()
                .map(|process| (*process).to_string()),
            installed,
            running: false,
            support_level: presentation.support_level.to_string(),
            artwork,
            stats,
        }
    }

    pub(crate) fn executable_exists(self, root: &FsPath) -> bool {
        self.detection_metadata()
            .process_names
            .iter()
            .any(|process| root.join(process).is_file())
    }
}

const BUILT_IN_GAME_MODULES: &[GameModule] = &[
    GameModule::from_supported_game_capabilities(
        GameModuleIdentity {
            id: "forza-horizon-6",
            display_name: "Forza Horizon 6",
        },
        GameModuleTelemetryLink {
            adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        },
        GameModuleProfileMetadata {
            default_profile_id: FORZA_HORIZON_IMMERSIVE_PROFILE_ID,
            templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        },
        GameModuleDetectionMetadata {
            process_names: &[
                "ForzaHorizon6.exe",
                "ForzaHorizon6-WinGDK-Shipping.exe",
                "ForzaHorizon6_Steam.exe",
            ],
            steam_app_ids: &[FORZA_HORIZON6_STEAM_APP_ID],
            steam_install_dirs: &["ForzaHorizon6"],
            steam_catalog: true,
        },
        GameModulePresentationMetadata {
            source: "built_in",
            input_provider: "native_dualsense",
            support_level: "telemetry",
            detection_lightbar: GameModuleLightbarCue {
                color_hex: "#00a8ff",
                brightness_percent: 58,
            },
        },
    ),
    GameModule::from_supported_game_capabilities(
        GameModuleIdentity {
            id: "forza-horizon-5",
            display_name: "Forza Horizon 5",
        },
        GameModuleTelemetryLink {
            adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        },
        GameModuleProfileMetadata {
            default_profile_id: FORZA_HORIZON_IMMERSIVE_PROFILE_ID,
            templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        },
        GameModuleDetectionMetadata {
            process_names: &[
                "ForzaHorizon5.exe",
                "ForzaHorizon5-Win64-Shipping.exe",
                "ForzaHorizon5_Steam.exe",
            ],
            steam_app_ids: &[FORZA_HORIZON5_STEAM_APP_ID],
            steam_install_dirs: &["ForzaHorizon5"],
            steam_catalog: true,
        },
        GameModulePresentationMetadata {
            source: "built_in",
            input_provider: "native_dualsense",
            support_level: "telemetry",
            detection_lightbar: GameModuleLightbarCue {
                color_hex: "#00a8ff",
                brightness_percent: 58,
            },
        },
    ),
    GameModule::from_supported_game_capabilities(
        GameModuleIdentity {
            id: "forza-motorsport",
            display_name: "Forza Motorsport",
        },
        GameModuleTelemetryLink {
            adapter_id: FORZA_DATA_OUT_ADAPTER_ID,
        },
        GameModuleProfileMetadata {
            default_profile_id: FORZA_HORIZON_PROFILE_ID,
            templates: FORZA_HORIZON_PROFILE_TEMPLATES,
        },
        GameModuleDetectionMetadata {
            process_names: &["ForzaMotorsport.exe", "ForzaMotorsport-WinGDK-Shipping.exe"],
            steam_app_ids: &["2483190"],
            steam_install_dirs: &[],
            steam_catalog: false,
        },
        GameModulePresentationMetadata {
            source: "built_in",
            input_provider: "native_dualsense",
            support_level: "telemetry",
            detection_lightbar: GameModuleLightbarCue {
                color_hex: "#00a8ff",
                brightness_percent: 58,
            },
        },
    ),
    GameModule::from_supported_game_capabilities(
        GameModuleIdentity {
            id: "assetto-corsa-rally",
            display_name: "Assetto Corsa Rally",
        },
        GameModuleTelemetryLink {
            adapter_id: ASSETTO_SHARED_MEMORY_ADAPTER_ID,
        },
        GameModuleProfileMetadata {
            default_profile_id: ASSETTO_CORSA_RALLY_PROFILE_ID,
            templates: ASSETTO_CORSA_RALLY_PROFILE_TEMPLATES,
        },
        GameModuleDetectionMetadata {
            process_names: &["acr.exe"],
            steam_app_ids: &[ASSETTO_CORSA_RALLY_STEAM_APP_ID],
            steam_install_dirs: &["Assetto Corsa Rally"],
            steam_catalog: true,
        },
        GameModulePresentationMetadata {
            source: "built_in",
            input_provider: "native_dualsense",
            support_level: "telemetry",
            detection_lightbar: GameModuleLightbarCue {
                color_hex: "#ff3b30",
                brightness_percent: 62,
            },
        },
    ),
];

pub(crate) fn built_in_game_modules() -> &'static [GameModule] {
    BUILT_IN_GAME_MODULES
}

pub(crate) fn game_module_summaries() -> Vec<ModuleSummary> {
    BUILT_IN_GAME_MODULES
        .iter()
        .map(|game| (*game).module_summary())
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
            if let Some(candidate) = (*game).process_detection_candidate(process, 82) {
                candidates.push(candidate);
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
    (*game).supported_summary(app_id, install_path, artwork, stats)
}

pub(crate) fn game_executable_exists(root: &FsPath, game: &GameModule) -> bool {
    (*game).executable_exists(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn built_in_game_modules_have_unique_ids_and_required_metadata() {
        let mut ids = BTreeSet::new();
        for game in built_in_game_modules() {
            let identity = (*game).identity();
            let telemetry = (*game).telemetry_link();
            let profiles = (*game).profile_metadata();
            let detection = (*game).detection_metadata();
            let presentation = (*game).presentation_metadata();

            assert!(
                ids.insert(identity.id),
                "duplicate game module id {}",
                identity.id
            );
            assert!(!identity.display_name.is_empty());
            assert!(!telemetry.adapter_id.is_empty());
            assert!(!profiles.default_profile_id.is_empty());
            assert!(
                !detection.process_names.is_empty(),
                "{} must have process detection metadata",
                identity.id
            );
            assert!(
                !profiles.templates.is_empty(),
                "{} must expose at least one profile template",
                identity.id
            );
            assert!(!presentation.source.is_empty());
            assert!(!presentation.input_provider.is_empty());
            assert!(!presentation.support_level.is_empty());
            assert!(!presentation.detection_lightbar.color_hex.is_empty());
            assert!(presentation.detection_lightbar.brightness_percent <= 100);
        }
    }

    #[test]
    fn supported_game_interface_builds_module_summary_and_supported_summary() {
        let game = built_in_game_modules()
            .iter()
            .copied()
            .find(|game| game.identity().id == "assetto-corsa-rally")
            .expect("Assetto Corsa Rally module exists");

        let setup = game.setup_metadata();
        assert_eq!(setup.protocol, "game:assetto-shared-memory");
        assert!(setup.setup_hint.contains("assetto-shared-memory adapter"));

        let module = game.module_summary();
        assert_eq!(module.id, "assetto-corsa-rally");
        assert_eq!(module.name, "Assetto Corsa Rally");
        assert_eq!(module.protocol, setup.protocol);
        assert_eq!(module.profile_templates, vec!["Rally".to_string()]);

        let summary = game.supported_summary(
            Some(ASSETTO_CORSA_RALLY_STEAM_APP_ID.to_string()),
            None,
            GameArtwork::default(),
            SteamGameStats::default(),
        );
        assert_eq!(summary.source, "built_in");
        assert_eq!(summary.input_provider, "native_dualsense");
        assert_eq!(summary.support_level, "telemetry");
        assert_eq!(summary.executable_name.as_deref(), Some("acr.exe"));
        assert_eq!(summary.process_names, vec!["acr.exe".to_string()]);
    }

    #[test]
    fn forza_games_are_distinct_modules_with_a_shared_adapter() {
        let game_module_by_id = |id: &str| {
            built_in_game_modules()
                .iter()
                .find(|game| game.identity().id == id)
        };
        let forza_games = [
            game_module_by_id("forza-horizon-6").unwrap(),
            game_module_by_id("forza-horizon-5").unwrap(),
            game_module_by_id("forza-motorsport").unwrap(),
        ];
        let ids = forza_games
            .iter()
            .map(|game| game.identity().id)
            .collect::<BTreeSet<_>>();

        assert_eq!(ids.len(), forza_games.len());
        assert!(forza_games
            .iter()
            .all(|game| game.telemetry_link().adapter_id == FORZA_DATA_OUT_ADAPTER_ID));
    }

    #[test]
    fn assetto_corsa_rally_is_a_distinct_shared_memory_module() {
        let game = built_in_game_modules()
            .iter()
            .find(|game| game.identity().id == "assetto-corsa-rally")
            .expect("Assetto Corsa Rally module exists");
        let telemetry = (*game).telemetry_link();
        let profiles = (*game).profile_metadata();
        let detection = (*game).detection_metadata();

        assert_eq!(telemetry.adapter_id, ASSETTO_SHARED_MEMORY_ADAPTER_ID);
        assert_eq!(profiles.default_profile_id, ASSETTO_CORSA_RALLY_PROFILE_ID);
        assert_eq!(detection.steam_app_ids, &[ASSETTO_CORSA_RALLY_STEAM_APP_ID]);
        assert!(detection
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
