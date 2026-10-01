use super::*;
use directories::ProjectDirs;
use std::sync::{atomic::AtomicU64, Mutex};

static NEXT_SAVE: AtomicU64 = AtomicU64::new(1);
// ponytail: one serialized writer is sufficient for this desktop app's small settings file.
// Allocate sequence numbers under the state lock; discard saves overtaken after that lock is released.
static COMPLETED_SAVES: Mutex<BTreeMap<PathBuf, u64>> = Mutex::new(BTreeMap::new());

pub(crate) struct PendingSave {
    store: PersistenceStore,
    snapshot: PersistedAgentState,
    sequence: u64,
}

impl PendingSave {
    fn save(self) -> io::Result<()> {
        let mut completed = COMPLETED_SAVES
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if completed
            .get(&self.store.state_file)
            .is_some_and(|sequence| *sequence > self.sequence)
        {
            return Ok(());
        }
        self.store.save_snapshot(&self.snapshot)?;
        completed.insert(self.store.state_file, self.sequence);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PersistenceStore {
    pub(crate) state_file: PathBuf,
}

pub(crate) const PERSISTED_STATE_VERSION: u32 = 9;

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
    #[serde(default)]
    pub(crate) adapters: BTreeMap<String, PersistedAdapterState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedAdapterState {
    pub(crate) enabled: bool,
}

impl PersistenceStore {
    pub(crate) fn default() -> Option<Self> {
        if let Some(config_dir) = crate::runtime_paths::config_dir_override() {
            return Some(Self {
                state_file: config_dir.join("state.json"),
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
        use std::io::Write;
        let mut file = fs::File::create(&temp_file)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        replace_state_file(&temp_file, &self.state_file)
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
        self.adapters.retain(|id, _| {
            built_in_adapters()
                .iter()
                .any(|adapter| adapter.id == id.as_str())
        });
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
            adapters: inner
                .adapters
                .iter()
                .map(|adapter| {
                    (
                        adapter.id.clone(),
                        PersistedAdapterState {
                            enabled: adapter.enabled,
                        },
                    )
                })
                .collect(),
        }
    }
}

pub(crate) fn temp_path_for(path: &FsPath) -> PathBuf {
    let mut temp = path.to_path_buf();
    temp.set_extension("json.tmp");
    temp
}

#[cfg(not(windows))]
fn replace_state_file(source: &FsPath, destination: &FsPath) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_state_file(source: &FsPath, destination: &FsPath) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Both paths are owned, NUL-terminated buffers that live through the call.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn build_persist_snapshot(inner: &AgentStateInner) -> Option<PendingSave> {
    inner.storage.clone().map(|store| PendingSave {
        store,
        snapshot: PersistedAgentState::from_inner(inner),
        sequence: NEXT_SAVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    })
}

pub(crate) async fn persist_snapshot(state: &AgentState, to_save: Option<PendingSave>) {
    let Some(save) = to_save else {
        return;
    };
    let result = tokio::task::spawn_blocking(move || save.save()).await;
    let save_error = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error.to_string(),
        Err(join_error) => format!("persistence task panicked: {join_error}"),
    };
    state
        .log_warn(format!("Could not persist DSCC state: {save_error}"))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> PersistenceStore {
        PersistenceStore {
            state_file: std::env::temp_dir()
                .join(format!(
                    "dscc-persistence-{}-{}",
                    std::process::id(),
                    NEXT_SAVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ))
                .join("state.json"),
        }
    }

    #[test]
    fn delayed_older_save_cannot_replace_newer_state() {
        let store = test_store();
        let older = PendingSave {
            store: store.clone(),
            snapshot: PersistedAgentState {
                active_profile_id: Some("older".into()),
                ..Default::default()
            },
            sequence: NEXT_SAVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        let newer = PendingSave {
            store: store.clone(),
            snapshot: PersistedAgentState {
                active_profile_id: Some("newer".into()),
                ..Default::default()
            },
            sequence: NEXT_SAVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        newer.save().unwrap();
        older.save().unwrap();
        assert_eq!(
            store.load().unwrap().active_profile_id.as_deref(),
            Some("newer")
        );
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

    #[test]
    fn failed_temp_write_preserves_previous_state() {
        let store = test_store();
        let original = PersistedAgentState {
            active_profile_id: Some("original".into()),
            ..Default::default()
        };
        store.save_snapshot(&original).unwrap();
        fs::create_dir(temp_path_for(&store.state_file)).unwrap();
        assert!(store
            .save_snapshot(&PersistedAgentState::default())
            .is_err());
        assert_eq!(
            store.load().unwrap().active_profile_id,
            original.active_profile_id
        );
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }
}
