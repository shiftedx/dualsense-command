use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct SteamGameCatalog {
    pub(crate) supported_games: Vec<SupportedGameSummary>,
    pub(crate) artwork_paths: BTreeMap<(String, String), PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionResponse {
    pub active_game_id: Option<String>,
    pub active_game_name: Option<String>,
    pub source: String,
    pub confidence: u8,
    pub process_name: Option<String>,
    pub module_id: Option<String>,
    pub adapter_id: Option<String>,
    pub profile_id: Option<String>,
    pub candidates: Vec<GameDetectionCandidate>,
    #[serde(default)]
    pub supported_games: Vec<SupportedGameSummary>,
    #[serde(default)]
    pub selected_game: Option<SupportedGameSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDetectionCandidate {
    pub game_id: String,
    pub name: String,
    pub process_name: String,
    pub module_id: String,
    pub adapter_id: String,
    pub profile_id: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameArtwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capsule_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamGameStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played_unix: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub achievements: Option<SteamAchievementStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SteamAchievementStats {
    pub unlocked: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportedGameSummary {
    pub game_id: String,
    pub name: String,
    #[serde(default = "default_game_source")]
    pub source: String,
    #[serde(default = "default_game_input_provider")]
    pub input_provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_name: Option<String>,
    pub installed: bool,
    pub running: bool,
    pub support_level: String,
    #[serde(default)]
    pub artwork: GameArtwork,
    #[serde(default)]
    pub stats: SteamGameStats,
}

pub(crate) fn default_game_source() -> String {
    "built_in".to_string()
}

pub(crate) fn default_game_input_provider() -> String {
    "native_dualsense".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserGameConfig {
    pub game_id: String,
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryEntry {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub install_path: String,
    pub artwork: GameArtwork,
    pub stats: SteamGameStats,
    pub already_in_catalog: bool,
    pub suggested_game_id: String,
    pub process_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryListResponse {
    pub games: Vec<SteamLibraryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameRequest {
    pub app_id: String,
    /// Optional override for the .exe process names DSCC will watch for to
    /// auto-load this game's profile. When omitted/empty, the agent uses the
    /// candidates it discovered by scanning the install directory. Useful for
    /// games whose .exe lives in a subfolder or isn't named obviously.
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLocalGameRequest {
    pub name: Option<String>,
    pub executable_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLocalGameResponse {
    pub valid: bool,
    pub name: String,
    pub executable_name: String,
    pub process_names: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLocalGameRequest {
    pub name: String,
    pub executable_path: String,
    #[serde(default)]
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserGameResponse {
    pub game: SupportedGameSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseSteamLibraryParams {
    pub app_id: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseEntry {
    pub name: String,
    /// `"dir"` or `"exe"`. Kept as a string so the wire shape is forward
    /// compatible if we ever surface other kinds (data files, configs, ...).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibraryBrowseResponse {
    pub app_id: String,
    pub install_path: String,
    /// Path relative to `install_path`, using forward slashes and never
    /// containing `..` segments. Empty string means the install root.
    pub relative_path: String,
    pub entries: Vec<SteamLibraryBrowseEntry>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SteamAppManifest {
    pub(crate) app_id: String,
    pub(crate) name: String,
    pub(crate) install_dir: String,
    pub(crate) install_path: PathBuf,
}
