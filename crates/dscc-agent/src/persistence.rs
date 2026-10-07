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
        let contents = match fs::read(&self.state_file) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(PersistedAgentState::default()),
            Err(error) => return Err(io::Error::new(error.kind(), "Saved state could not be read; replacement saves are blocked until it can be read.")),
        };
        parse_persisted_state(&contents)
    }

    pub(crate) fn save_snapshot(&self, snapshot: &PersistedAgentState) -> io::Result<()> {
        // A load failure must never turn the only recovery copy into writable defaults.
        // Recheck here as files can become unreadable or corrupt after initialization.
        match fs::read(&self.state_file) {
            Ok(contents) if parse_persisted_state(&contents).is_err() => {
                let (mut backup, _) = create_exclusive_state_backup(&self.state_file)?;
                use std::io::Write;
                backup.write_all(&contents)?;
                backup.sync_all()?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    "Saved state could not be read; refusing replacement.",
                ))
            }
        }
        if let Some(parent) = self.state_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut snapshot = snapshot.clone();
        snapshot.version = PERSISTED_STATE_VERSION;
        let contents = serde_json::to_string_pretty(&snapshot)
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

fn parse_persisted_state(contents: &[u8]) -> io::Result<PersistedAgentState> {
    let state: PersistedAgentState = serde_json::from_slice(contents).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved state is malformed; the original will be quarantined before replacement.",
        )
    })?;
    if state.version != PERSISTED_STATE_VERSION {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Saved state version is unsupported; the original will be quarantined before replacement."));
    }
    Ok(state)
}

fn create_exclusive_state_backup(path: &FsPath) -> io::Result<(fs::File, PathBuf)> {
    loop {
        let sequence = NEXT_SAVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let backup = path.with_extension(format!(
            "json.dscc-recovery-{}-{sequence}",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(file) => return Ok((file, backup)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
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
pub(crate) fn replace_state_file(source: &FsPath, destination: &FsPath) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
pub(crate) fn replace_state_file(source: &FsPath, destination: &FsPath) -> io::Result<()> {
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
        Ok(Ok(())) => {
            state.inner.write().await.persistence_load_error = None;
            return;
        }
        Ok(Err(error)) => error.to_string(),
        Err(join_error) => format!("persistence task panicked: {join_error}"),
    };
    state
        .log_warn(format!("Could not persist DSCC state: {save_error}"))
        .await;
}

/// Attach to persisted mutation owners, leaving runtime stop/test routes available.
pub(crate) async fn require_persistence_available(
    axum::extract::State(state): axum::extract::State<AgentState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        let inner = state.inner.read().await;
        if inner.storage.is_none() {
            if let Some(error) = &inner.persistence_load_error {
                return (axum::http::StatusCode::SERVICE_UNAVAILABLE, error.clone())
                    .into_response();
            }
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_state_is_preserved_before_a_subsequent_save() {
        let store = test_store();
        fs::create_dir_all(store.state_file.parent().unwrap()).unwrap();
        let original = b"{ malformed private recovery bytes";
        fs::write(&store.state_file, original).unwrap();
        assert!(store.load().is_err());
        store
            .save_snapshot(&PersistedAgentState::default())
            .unwrap();
        let preserved = fs::read_dir(store.state_file.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                entry.path() != store.state_file
                    && fs::read(entry.path()).ok().as_deref() == Some(original.as_slice())
            });
        assert!(preserved, "original must be quarantined before replacement");
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

    #[test]
    fn unsupported_state_version_is_not_silently_loaded() {
        let store = test_store();
        fs::create_dir_all(store.state_file.parent().unwrap()).unwrap();
        let state = PersistedAgentState {
            version: PERSISTED_STATE_VERSION + 1,
            ..Default::default()
        };
        fs::write(&store.state_file, serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(store.load().unwrap_err().kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn failed_load_is_visible_and_mutation_preserves_recovery_bytes() {
        let store = test_store();
        fs::create_dir_all(store.state_file.parent().unwrap()).unwrap();
        fs::write(&store.state_file, b"invalid private-fixture").unwrap();
        let state = AgentState::from_controller_registry_with_backend_and_storage(
            ControllerRegistry::default(),
            AgentState::mock().inner.read().await.device_backend.clone(),
            Some(store.clone()),
        );
        let pending = {
            let mut inner = state.inner.write().await;
            assert!(inner
                .logs
                .iter()
                .any(|log| log.level == "warn" && log.message.contains("load")));
            assert!(inner
                .logs
                .iter()
                .all(|log| !log.message.contains("private-fixture")));
            inner.active_profile_id = Some("changed".into());
            build_persist_snapshot(&inner)
        };
        persist_snapshot(&state, pending).await;
        assert!(fs::read_dir(store.state_file.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(
                |entry| fs::read(entry.path()).ok().as_deref() == Some(b"invalid private-fixture")
            ));
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn unreadable_state_cannot_be_overwritten() {
        use std::os::windows::fs::OpenOptionsExt;
        let store = test_store();
        fs::create_dir_all(store.state_file.parent().unwrap()).unwrap();
        fs::write(&store.state_file, b"private original").unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&store.state_file)
            .unwrap();
        assert!(store.load().is_err());
        assert!(store
            .save_snapshot(&PersistedAgentState::default())
            .is_err());
        drop(locked);
        assert_eq!(fs::read(&store.state_file).unwrap(), b"private original");
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn unreadable_startup_cannot_later_overwrite_recovered_state() {
        use std::os::windows::fs::OpenOptionsExt;
        use tower::ServiceExt;
        let store = test_store();
        store
            .save_snapshot(&PersistedAgentState {
                active_profile_id: Some("original".into()),
                ..Default::default()
            })
            .unwrap();
        let original = fs::read(&store.state_file).unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&store.state_file)
            .unwrap();
        let state = AgentState::from_controller_registry_with_backend_and_storage(
            ControllerRegistry::default(),
            AgentState::mock().inner.read().await.device_backend.clone(),
            Some(store.clone()),
        );
        drop(locked);
        assert!(!state.status_with_detection(None).await.healthy);
        assert!(state
            .diagnostics()
            .await
            .checks
            .iter()
            .any(|check| check.name == "persistence" && check.status == "error"));
        let response = app(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri("/api/app-settings")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(r#"{"listenOnAllInterfaces":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        // Every route whose owner captures a persistence snapshot is guarded before
        // extraction/side effects, including requests that would otherwise be invalid.
        for (method, uri) in [
            ("PUT", "/api/controllers/missing"),
            ("PUT", "/api/controllers/missing/config"),
            ("PUT", "/api/controllers/missing/edge-profiles/1"),
            ("POST", "/api/profiles"),
            ("POST", "/api/profiles/import"),
            ("PUT", "/api/profiles/missing"),
            ("DELETE", "/api/profiles/missing"),
            ("PUT", "/api/profiles/missing/config"),
            ("POST", "/api/profiles/missing/activate"),
            ("PUT", "/api/adapters/missing"),
            ("POST", "/api/input-bridge/bindings"),
            ("POST", "/api/games/local"),
            ("POST", "/api/games/custom"),
            ("DELETE", "/api/games/custom/missing"),
            ("PUT", "/api/profile-resolution/override"),
            ("DELETE", "/api/profile-resolution/override"),
        ] {
            let response = app(state.clone())
                .oneshot(
                    axum::http::Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "{method} {uri}"
            );
        }
        for uri in [
            "/api/status",
            "/api/diagnostics",
            "/api/app-settings",
            "/api/profiles",
        ] {
            let response = app(state.clone())
                .oneshot(
                    axum::http::Request::builder()
                        .uri(uri)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), axum::http::StatusCode::OK, "{uri}");
        }
        let response = app(state.clone())
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/input-bridge/sessions/missing/stop")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let pending = {
            let mut inner = state.inner.write().await;
            inner.active_profile_id = Some("mutation-after-unlock".into());
            build_persist_snapshot(&inner)
        };
        persist_snapshot(&state, pending).await;
        assert_eq!(fs::read(&store.state_file).unwrap(), original);
        fs::remove_dir_all(store.state_file.parent().unwrap()).unwrap();
    }

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
