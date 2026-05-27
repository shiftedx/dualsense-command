use super::*;
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub(crate) struct PersistenceStore {
    pub(crate) state_file: PathBuf,
}

pub(crate) const PERSISTED_STATE_VERSION: u32 = 8;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedAgentState {
    pub(crate) version: u32,
    pub(crate) profiles: Vec<ProfileSummary>,
    #[serde(default)]
    pub(crate) controller_names: BTreeMap<String, String>,
    pub(crate) controller_configs: BTreeMap<String, ControllerConfig>,
    #[serde(default)]
    pub(crate) profile_configs: BTreeMap<String, ProfileConfig>,
    pub(crate) profile_overrides: BTreeMap<String, ProfileOverride>,
    pub(crate) edge_profiles: BTreeMap<String, EdgeProfileStore>,
    pub(crate) app_settings: AppSettings,
    pub(crate) active_profile_id: Option<String>,
    #[serde(default)]
    pub(crate) user_games: BTreeMap<String, UserGameConfig>,
}

impl PersistenceStore {
    pub(crate) fn default() -> Option<Self> {
        if let Some(config_dir) = std::env::var_os("DSCC_CONFIG_DIR") {
            return Some(Self {
                state_file: PathBuf::from(config_dir).join("state.json"),
            });
        }

        ProjectDirs::from("dev", "DualSenseCommand", "DualSenseCommandCenter").map(|dirs| Self {
            state_file: dirs.config_dir().join("state.json"),
        })
    }

    pub(crate) fn load(&self) -> io::Result<PersistedAgentState> {
        if !self.state_file.exists() {
            return Ok(PersistedAgentState::default());
        }

        let contents = fs::read_to_string(&self.state_file)?;
        serde_json::from_str(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub(crate) fn save_snapshot(&self, snapshot: &PersistedAgentState) -> io::Result<()> {
        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let temp_file = temp_path_for(&self.state_file);
        fs::write(&temp_file, contents)?;
        if self.state_file.exists() {
            fs::remove_file(&self.state_file)?;
        }
        fs::rename(temp_file, &self.state_file)
    }
}

impl PersistedAgentState {
    pub(crate) fn normalized(mut self) -> Self {
        self.profiles = self
            .profiles
            .into_iter()
            .filter_map(|mut profile| {
                let id = profile.id.trim().to_string();
                if id.is_empty() || is_default_profile_id(&id) {
                    return None;
                }
                profile.id = id;
                profile.built_in = false;
                profile.active = false;
                Some(profile)
            })
            .collect();
        let persisted_profiles = self.profiles.clone();
        self.controller_names = self
            .controller_names
            .into_iter()
            .filter_map(|(id, name)| {
                let id = id.trim().chars().take(160).collect::<String>();
                let name = normalize_controller_display_name(&name)?;
                (!id.is_empty()).then_some((id, name))
            })
            .collect();
        self.controller_configs = self
            .controller_configs
            .into_iter()
            .map(|(id, config)| {
                let mut config = config.normalized();
                config.profile_assignments = normalize_existing_profile_assignments(
                    config.profile_assignments,
                    &persisted_profiles,
                );
                (id, config)
            })
            .collect();
        self.profile_configs = self
            .profile_configs
            .into_iter()
            .filter(|(id, _)| {
                let id = id.trim();
                !id.is_empty()
                    && !is_default_profile_id(id)
                    && profile_exists_in_defaults_or_persisted(id, &persisted_profiles)
            })
            .map(|(id, config)| {
                let config = config.normalized_for_model("DualSense");
                (id, config)
            })
            .collect();
        self.edge_profiles = self
            .edge_profiles
            .into_iter()
            .map(|(id, store)| (id, store.normalized()))
            .collect();
        self.profile_overrides = self
            .profile_overrides
            .into_iter()
            .filter_map(|(key, mut profile)| {
                let profile_id = profile.profile_id.trim().to_string();
                if profile_id.is_empty()
                    || !profile_exists_in_defaults_or_persisted(&profile_id, &persisted_profiles)
                {
                    return None;
                }
                profile.profile_id = profile_id;
                Some((key, profile))
            })
            .collect();
        self.active_profile_id = self.active_profile_id.and_then(|id| {
            let id = id.trim().to_string();
            (!id.is_empty() && profile_exists_in_defaults_or_persisted(&id, &persisted_profiles))
                .then_some(id)
        });
        self.app_settings.forza_playstation_glyphs.install_path = self
            .app_settings
            .forza_playstation_glyphs
            .install_path
            .and_then(|path| (!path.trim().is_empty()).then_some(path));
        self.user_games = self
            .user_games
            .into_iter()
            .filter_map(|(id, config)| {
                let game_id = config.game_id.trim().to_string();
                if game_id.is_empty()
                    || game_id != id.trim()
                    || built_in_game_modules()
                        .iter()
                        .any(|module| module.id == game_id)
                {
                    return None;
                }
                let mut config = config;
                config.game_id = game_id.clone();
                config.app_id = config.app_id.trim().to_string();
                config.name = config.name.trim().to_string();
                config.install_dir = config.install_dir.trim().to_string();
                config.install_path = config.install_path.trim().to_string();
                config.process_names = config
                    .process_names
                    .into_iter()
                    .filter_map(|name| {
                        let trimmed = name.trim();
                        (!trimmed.is_empty()).then(|| trimmed.to_string())
                    })
                    .collect();
                if config.app_id.is_empty() || config.name.is_empty() {
                    return None;
                }
                Some((game_id, config))
            })
            .collect();
        self.version = PERSISTED_STATE_VERSION;
        self
    }

    pub(crate) fn from_inner(inner: &AgentStateInner) -> Self {
        Self {
            version: PERSISTED_STATE_VERSION,
            profiles: inner
                .profiles
                .iter()
                .filter(|profile| !profile.built_in)
                .cloned()
                .collect(),
            controller_names: inner.controller_names.clone(),
            controller_configs: inner.controller_configs.clone(),
            profile_configs: inner
                .profile_configs
                .iter()
                .filter(|(id, _)| !is_default_profile_id(id))
                .map(|(id, config)| (id.clone(), config.clone()))
                .collect(),
            profile_overrides: inner.profile_overrides.clone(),
            edge_profiles: inner.edge_profiles.clone(),
            app_settings: inner.app_settings.clone(),
            active_profile_id: inner.active_profile_id.clone(),
            user_games: inner.user_games.clone(),
        }
    }
}

pub(crate) fn temp_path_for(path: &FsPath) -> PathBuf {
    let mut temp = path.to_path_buf();
    temp.set_extension("json.tmp");
    temp
}

pub(crate) fn build_persist_snapshot(
    inner: &AgentStateInner,
) -> Option<(PersistenceStore, PersistedAgentState)> {
    inner
        .storage
        .clone()
        .map(|store| (store, PersistedAgentState::from_inner(inner)))
}

pub(crate) async fn persist_snapshot(
    state: &AgentState,
    to_save: Option<(PersistenceStore, PersistedAgentState)>,
) {
    let Some((store, snapshot)) = to_save else {
        return;
    };
    let result = tokio::task::spawn_blocking(move || store.save_snapshot(&snapshot)).await;
    let save_error = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error.to_string(),
        Err(join_error) => format!("persistence task panicked: {join_error}"),
    };
    state
        .log_warn(format!("Could not persist DSCC state: {save_error}"))
        .await;
}
