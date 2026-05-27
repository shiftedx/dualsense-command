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

pub fn app_paths() -> Option<AppPaths> {
    ProjectDirs::from("dev", "DualSenseCommand", "DualSenseCommandCenter").map(|dirs| AppPaths {
        config_dir: dirs.config_dir().display().to_string(),
        data_dir: dirs.data_dir().display().to_string(),
        log_dir: dirs.cache_dir().join("logs").display().to_string(),
    })
}
