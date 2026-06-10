use super::*;

/// TTL-cached discovery state shared by snapshot queries and the hardware
/// output gate: detected Supported Game, Steam game catalog, Steam Input
/// status, and update check.
#[derive(Debug)]
pub(crate) struct DiscoveryCache {
    pub(crate) game_detection: AsyncMutex<CachedValue<GameDetectionResponse>>,
    pub(crate) steam_input: AsyncMutex<CachedValue<SteamInputStatus>>,
    pub(crate) steam_game_catalog: AsyncMutex<CachedValue<SteamGameCatalog>>,
    pub(crate) update_check: AsyncMutex<CachedValue<UpdateCheckResponse>>,
    pub(crate) steam_input_refreshing: AtomicBool,
}

impl Default for DiscoveryCache {
    fn default() -> Self {
        Self {
            game_detection: AsyncMutex::new(CachedValue::default()),
            steam_input: AsyncMutex::new(CachedValue::default()),
            steam_game_catalog: AsyncMutex::new(CachedValue::default()),
            update_check: AsyncMutex::new(CachedValue::default()),
            steam_input_refreshing: AtomicBool::new(false),
        }
    }
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
        let mut detection = enrich_game_detection(detection, &catalog);
        let (steam_root, steam_stats) =
            tokio::task::spawn_blocking(steam_root_and_stats_for_user_games)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "Steam root/stats lookup task failed");
                    (None, BTreeMap::new())
                });
        append_user_games_to_detection(
            &mut detection,
            &user_games,
            steam_root.as_deref(),
            &steam_stats,
        );
        if detection.active_game_id.is_none() {
            let inner = self.inner.read().await;
            if let Some(telemetry_detection) = telemetry_game_detection(&inner, &catalog) {
                detection = enrich_game_detection(telemetry_detection, &catalog);
                append_user_games_to_detection(
                    &mut detection,
                    &user_games,
                    steam_root.as_deref(),
                    &steam_stats,
                );
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

    pub(crate) async fn cached_hardware_game_detection(&self) -> GameDetectionResponse {
        self.cached_game_detection_with_ttl(HARDWARE_GAME_DETECTION_INTERVAL)
            .await
    }

    pub(crate) async fn cached_steam_game_catalog(&self) -> SteamGameCatalog {
        let now = Instant::now();
        {
            let cache = self.discovery_cache.steam_game_catalog.lock().await;
            if let Some(value) = cache.fresh(STEAM_GAME_CATALOG_CACHE_TTL, now) {
                return value;
            }
        }

        let catalog = tokio::task::spawn_blocking(discover_steam_game_catalog)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Steam game catalog discovery task failed");
                unsupported_steam_game_catalog()
            });
        let mut cache = self.discovery_cache.steam_game_catalog.lock().await;
        cache.store(catalog, Instant::now())
    }
}
