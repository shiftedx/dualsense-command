use super::*;

/// TTL-cached discovery state shared by snapshot queries and the hardware
/// output gate: detected Supported Game, Steam game catalog, Steam Input
/// status, and update check.
#[derive(Debug)]
pub(crate) struct DiscoveryCache {
    pub(crate) game_detection: AsyncMutex<CachedValue<GameDetectionResponse>>,
    hardware_game_detection: Mutex<CachedValue<GameDetectionResponse>>,
    hardware_detection_refreshing: AtomicBool,
    pub(crate) steam_input: AsyncMutex<CachedValue<SteamInputStatus>>,
    pub(crate) steam_game_catalog: AsyncMutex<CachedValue<Arc<SteamGameCatalog>>>,
    user_game_catalog: AsyncMutex<Option<CachedUserGameCatalog>>,
    pub(crate) update_check: AsyncMutex<CachedValue<UpdateCheckResponse>>,
    pub(crate) steam_input_refreshing: AtomicBool,
}

impl Default for DiscoveryCache {
    fn default() -> Self {
        Self {
            game_detection: AsyncMutex::new(CachedValue::default()),
            hardware_game_detection: Mutex::new(CachedValue::default()),
            hardware_detection_refreshing: AtomicBool::new(false),
            steam_input: AsyncMutex::new(CachedValue::default()),
            steam_game_catalog: AsyncMutex::new(CachedValue::default()),
            user_game_catalog: AsyncMutex::new(None),
            update_check: AsyncMutex::new(CachedValue::default()),
            steam_input_refreshing: AtomicBool::new(false),
        }
    }
}

#[derive(Debug)]
struct CachedUserGameCatalog {
    registrations: BTreeMap<String, UserGameConfig>,
    // Catalog replacement follows its five-minute TTL; registration equality
    // also invalidates presentation metadata after add/change/remove.
    steam_catalog: Arc<SteamGameCatalog>,
    games: Arc<Vec<SupportedGameSummary>>,
}

#[derive(Debug)]
pub(crate) struct CachedValue<T> {
    pub(crate) value: Option<T>,
    pub(crate) refreshed_at: Option<Instant>,
}

impl<T> Default for CachedValue<T> {
    fn default() -> Self {
        Self {
            value: None,
            refreshed_at: None,
        }
    }
}

impl<T: Clone> CachedValue<T> {
    pub(crate) fn fresh(&self, ttl: Duration, now: Instant) -> Option<T> {
        match (self.value.as_ref(), self.refreshed_at) {
            (Some(value), Some(refreshed_at)) if now.duration_since(refreshed_at) < ttl => {
                Some(value.clone())
            }
            _ => None,
        }
    }

    pub(crate) fn store(&mut self, value: T, now: Instant) -> T {
        self.value = Some(value.clone());
        self.refreshed_at = Some(now);
        value
    }
}

impl AgentState {
    /// Hardware ticks never wait for filesystem/process discovery. Missing or
    /// expired detection permits only the controller-only Global Profile path.
    pub(crate) fn hardware_game_detection_snapshot(&self) -> Option<GameDetectionResponse> {
        let now = Instant::now();
        let (snapshot, refresh_due) = {
            let cache = self
                .discovery_cache
                .hardware_game_detection
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                cache.fresh(HARDWARE_GAME_DETECTION_INTERVAL, now),
                cache.refreshed_at.is_none_or(|last| {
                    now.duration_since(last)
                        >= HARDWARE_GAME_DETECTION_INTERVAL.saturating_sub(HARDWARE_OUTPUT_INTERVAL)
                }),
            )
        };
        if refresh_due
            && !self
                .discovery_cache
                .hardware_detection_refreshing
                .swap(true, Ordering::AcqRel)
        {
            let state = self.clone();
            tokio::spawn(async move {
                struct RefreshGuard(Arc<DiscoveryCache>);
                impl Drop for RefreshGuard {
                    fn drop(&mut self) {
                        self.0
                            .hardware_detection_refreshing
                            .store(false, Ordering::Release);
                    }
                }
                let _guard = RefreshGuard(Arc::clone(&state.discovery_cache));
                // A long catalog scan must not make an old process observation
                // look fresh when the worker eventually completes.
                let observed_at = Instant::now();
                let detection = state.cached_game_detection_with_ttl(Duration::ZERO).await;
                state
                    .discovery_cache
                    .hardware_game_detection
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .store(detection, observed_at);
            });
        }
        snapshot
    }

    pub(crate) async fn cached_game_detection_with_ttl(
        &self,
        ttl: Duration,
    ) -> GameDetectionResponse {
        let mut cache = self.discovery_cache.game_detection.lock().await;
        let now = Instant::now();
        if let Some(value) = cache.fresh(ttl, now) {
            return value;
        }

        let user_games = {
            let inner = self.inner.read().await;
            inner.user_games.clone()
        };
        let detection = detect_running_game(&user_games).await;
        let catalog = self.cached_steam_game_catalog().await;
        let user_summaries = self.cached_user_game_summaries(&user_games, &catalog).await;
        let mut detection = enrich_game_detection(detection, &catalog);
        append_user_games_to_detection(&mut detection, &user_summaries);
        if detection.active_game_id.is_none() {
            let inner = self.inner.read().await;
            if let Some(telemetry_detection) = telemetry_game_detection(&inner, &catalog) {
                detection = enrich_game_detection(telemetry_detection, &catalog);
                append_user_games_to_detection(&mut detection, &user_summaries);
            }
        }
        {
            let mut inner = self.inner.write().await;
            sync_auto_loaded_profile_for_detection(&mut inner, &detection);
        }
        cache.store(detection, Instant::now())
    }

    pub(crate) async fn cached_game_detection(&self) -> GameDetectionResponse {
        self.cached_game_detection_with_ttl(GAME_DETECTION_CACHE_TTL)
            .await
    }

    async fn cached_user_game_summaries(
        &self,
        user_games: &BTreeMap<String, UserGameConfig>,
        catalog: &Arc<SteamGameCatalog>,
    ) -> Arc<Vec<SupportedGameSummary>> {
        let mut cache = self.discovery_cache.user_game_catalog.lock().await;
        if let Some(cached) = cache.as_ref().filter(|cached| {
            cached.registrations == *user_games && Arc::ptr_eq(&cached.steam_catalog, catalog)
        }) {
            return Arc::clone(&cached.games);
        }
        let registrations = user_games.clone();
        let steam_catalog = Arc::clone(catalog);
        let games = tokio::task::spawn_blocking(move || {
            let mut games: Vec<_> = registrations
                .values()
                .map(|game| {
                    let stats = steam_catalog
                        .steam_stats
                        .get(&game.app_id)
                        .cloned()
                        .unwrap_or_default();
                    crate::game_detection::user_game_to_supported_summary(
                        game,
                        steam_catalog.steam_root.as_deref(),
                        stats,
                    )
                })
                .collect();
            games.sort_by_key(|game| game.name.to_ascii_lowercase());
            games
        })
        .await;
        match games {
            Ok(games) => {
                let games = Arc::new(games);
                *cache = Some(CachedUserGameCatalog {
                    registrations: user_games.clone(),
                    steam_catalog: Arc::clone(catalog),
                    games: Arc::clone(&games),
                });
                games
            }
            Err(error) => {
                tracing::warn!(%error, "User game presentation discovery task failed");
                Arc::new(Vec::new())
            }
        }
    }

    pub(crate) async fn cached_steam_game_catalog(&self) -> Arc<SteamGameCatalog> {
        let now = Instant::now();
        let mut cache = self.discovery_cache.steam_game_catalog.lock().await;
        if let Some(value) = cache.fresh(STEAM_GAME_CATALOG_CACHE_TTL, now) {
            return value;
        }

        let catalog = tokio::task::spawn_blocking(discover_steam_game_catalog)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Steam game catalog discovery task failed");
                unsupported_steam_game_catalog()
            });
        cache.store(Arc::new(catalog), Instant::now())
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    use crate::game_detection::user_game_to_supported_summary;

    #[tokio::test]
    async fn expired_hardware_detection_fails_closed_while_discovery_is_busy() {
        let state = AgentState::from_controller_events([]);
        let _busy = state.discovery_cache.game_detection.lock().await;
        let detection = detect_running_game_from_processes(["ForzaHorizon6.exe"]);
        state
            .discovery_cache
            .hardware_game_detection
            .lock()
            .unwrap()
            .store(detection.clone(), Instant::now());
        assert_eq!(
            state
                .hardware_game_detection_snapshot()
                .unwrap()
                .active_game_id,
            detection.active_game_id
        );
        state
            .discovery_cache
            .hardware_game_detection
            .lock()
            .unwrap()
            .store(detection, Instant::now() - HARDWARE_GAME_DETECTION_INTERVAL);
        assert!(state.hardware_game_detection_snapshot().is_none());
        assert!(state
            .discovery_cache
            .hardware_detection_refreshing
            .load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn user_install_metadata_survives_fast_detection_refresh() {
        let state = AgentState::from_controller_events([]);
        let install = std::env::temp_dir().join(format!(
            "dscc-presentation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&install).unwrap();
        {
            let mut cache = state.discovery_cache.steam_game_catalog.lock().await;
            cache.store(Arc::new(SteamGameCatalog::default()), Instant::now());
        }
        {
            let mut inner = state.inner.write().await;
            inner.user_games.insert(
                "custom-99999".into(),
                UserGameConfig {
                    game_id: "custom-99999".into(),
                    app_id: "99999".into(),
                    name: "Cached Racer".into(),
                    install_path: install.display().to_string(),
                    ..UserGameConfig::default()
                },
            );
        }
        let first = state.cached_game_detection_with_ttl(Duration::ZERO).await;
        assert!(
            first
                .supported_games
                .iter()
                .find(|game| game.game_id == "custom-99999")
                .unwrap()
                .installed
        );
        fs::remove_dir(&install).unwrap();
        let repeated = state.cached_game_detection_with_ttl(Duration::ZERO).await;
        assert!(
            repeated
                .supported_games
                .iter()
                .find(|game| game.game_id == "custom-99999")
                .unwrap()
                .installed
        );
        state
            .discovery_cache
            .steam_game_catalog
            .lock()
            .await
            .store(Arc::new(SteamGameCatalog::default()), Instant::now());
        let refreshed = state.cached_game_detection_with_ttl(Duration::ZERO).await;
        assert!(
            !refreshed
                .supported_games
                .iter()
                .find(|game| game.game_id == "custom-99999")
                .unwrap()
                .installed
        );
    }

    #[tokio::test]
    async fn presentation_cache_tracks_registration_changes_and_running_state() {
        let state = AgentState::from_controller_events([]);
        state
            .discovery_cache
            .steam_game_catalog
            .lock()
            .await
            .store(Arc::new(SteamGameCatalog::default()), Instant::now());
        let catalog = state.cached_steam_game_catalog().await;
        assert!(Arc::ptr_eq(
            &catalog,
            &state.cached_steam_game_catalog().await
        ));
        let mut registrations = BTreeMap::new();
        let empty = state
            .cached_user_game_summaries(&registrations, &catalog)
            .await;
        assert!(empty.is_empty());
        registrations.insert(
            "custom-99999".into(),
            UserGameConfig {
                game_id: "custom-99999".into(),
                app_id: "99999".into(),
                name: "Racer".into(),
                ..UserGameConfig::default()
            },
        );
        let added = state
            .cached_user_game_summaries(&registrations, &catalog)
            .await;
        assert_eq!(added[0].name, "Racer");
        let repeated = state
            .cached_user_game_summaries(&registrations, &catalog)
            .await;
        assert!(Arc::ptr_eq(&added, &repeated));
        let mut detection = detect_running_game_from_processes([]);
        detection.active_game_id = Some("custom-99999".into());
        append_user_games_to_detection(&mut detection, &added);
        assert!(detection.selected_game.as_ref().unwrap().running);
        assert!(!added[0].running);
        registrations.get_mut("custom-99999").unwrap().name = "Renamed".into();
        let changed = state
            .cached_user_game_summaries(&registrations, &catalog)
            .await;
        assert_eq!(changed[0].name, "Renamed");
        assert!(!Arc::ptr_eq(&added, &changed));
        registrations.clear();
        assert!(state
            .cached_user_game_summaries(&registrations, &catalog)
            .await
            .is_empty());
    }

    #[test]
    fn local_apps_and_empty_app_ids_have_no_steam_artwork() {
        for (game_id, app_id) in [("local-racer", "local-executable-id"), ("custom-empty", "")] {
            let game = UserGameConfig {
                game_id: game_id.into(),
                app_id: app_id.into(),
                ..UserGameConfig::default()
            };
            let summary = user_game_to_supported_summary(&game, None, SteamGameStats::default());
            assert_eq!(summary.artwork, GameArtwork::default());
        }
    }
}
