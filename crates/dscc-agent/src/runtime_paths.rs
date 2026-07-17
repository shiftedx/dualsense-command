use std::path::PathBuf;

use directories::ProjectDirs;

use crate::AppPaths;

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dscc_agent=info,tower_http=info".into()),
        )
        .try_init();
}

/// The effective config directory override, shared by persistence and
/// `app_paths` so `dscc paths` always reports where state is actually stored.
pub(crate) fn config_dir_override() -> Option<PathBuf> {
    std::env::var_os("DSCC_CONFIG_DIR").map(PathBuf::from)
}

pub fn app_paths() -> Option<AppPaths> {
    let dirs = ProjectDirs::from("dev", "DualSenseCommand", "DualSenseCommandCenter")?;
    let config_dir = config_dir_override().unwrap_or_else(|| dirs.config_dir().to_path_buf());
    Some(AppPaths {
        config_dir: config_dir.display().to_string(),
        data_dir: dirs.data_dir().display().to_string(),
        log_dir: dirs.cache_dir().join("logs").display().to_string(),
    })
}
