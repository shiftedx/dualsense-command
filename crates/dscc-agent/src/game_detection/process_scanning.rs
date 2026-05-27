use super::*;

#[cfg(target_os = "windows")]
pub(crate) fn windows_process_names() -> io::Result<Vec<String>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut names = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let process_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if !process_name.is_empty() {
                    names.push(process_name);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        Ok(names)
    }
}

#[cfg(all(target_os = "windows", not(test)))]
pub(crate) fn windows_process_image_paths_matching(targets: &[String]) -> io::Result<Vec<PathBuf>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut paths = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let process_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if targets
                    .iter()
                    .any(|target| target.eq_ignore_ascii_case(&process_name))
                {
                    let process =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    if !process.is_null() {
                        let mut buffer = [0_u16; 32768];
                        let mut size = buffer.len() as u32;
                        if QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size)
                            != 0
                            && size > 0
                        {
                            paths.push(PathBuf::from(String::from_utf16_lossy(
                                &buffer[..size as usize],
                            )));
                        }
                        CloseHandle(process);
                    }
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        Ok(paths)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_process_running(target: &str) -> bool {
    windows_process_names()
        .map(|names| {
            names
                .iter()
                .any(|process_name| process_name.eq_ignore_ascii_case(target))
        })
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) async fn detect_running_game(
    _user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    no_game_detection("none")
}

#[cfg(not(test))]
pub(crate) async fn detect_running_game(
    user_games: &BTreeMap<String, UserGameConfig>,
) -> GameDetectionResponse {
    if std::env::var_os("DSCC_DISABLE_PROCESS_SCAN").is_some() {
        return no_game_detection("process_scan_disabled");
    }

    match current_process_names().await {
        Ok(processes) => detect_running_game_from_processes_with_user_games(
            processes.iter().map(String::as_str),
            user_games,
        ),
        Err(error) => GameDetectionResponse {
            active_game_id: None,
            active_game_name: None,
            source: "process_scan_unavailable".to_string(),
            confidence: 0,
            process_name: None,
            module_id: None,
            adapter_id: None,
            profile_id: None,
            candidates: Vec::new(),
            supported_games: Vec::new(),
            selected_game: None,
        }
        .with_source_detail(error.to_string()),
    }
}

#[cfg(not(test))]
pub(crate) trait GameDetectionSourceDetail {
    fn with_source_detail(self, detail: String) -> Self;
}

#[cfg(not(test))]
impl GameDetectionSourceDetail for GameDetectionResponse {
    fn with_source_detail(mut self, detail: String) -> Self {
        self.source = format!("{}:{detail}", self.source);
        self
    }
}

#[cfg(not(test))]
pub(crate) async fn current_process_names() -> io::Result<Vec<String>> {
    #[cfg(target_os = "windows")]
    {
        windows_process_names()
    }

    #[cfg(not(target_os = "windows"))]
    {
        let output = tokio::process::Command::new("ps")
            .args(["-eo", "comm=", "-eo", "args="])
            .output()
            .await?;
        if !output.status.success() {
            return Err(io::Error::other("ps did not complete successfully"));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_unix_process_names(&text))
    }
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn parse_unix_process_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        for token in line.split_whitespace() {
            push_process_name_candidates(&mut names, &mut seen, token);
        }
    }
    names
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn push_process_name_candidates(
    names: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    raw: &str,
) {
    for candidate in process_name_candidates(raw) {
        let key = candidate.to_ascii_lowercase();
        if seen.insert(key) {
            names.push(candidate);
        }
    }
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn process_name_candidates(raw: &str) -> Vec<String> {
    let trimmed = raw.trim_matches(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';'
            )
    });
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    push_process_name_candidate(&mut candidates, trimmed);

    let normalized = trimmed.replace('\\', "/");
    if let Some(base) = normalized.rsplit('/').next() {
        push_process_name_candidate(&mut candidates, base);
    }

    let lower = normalized.to_ascii_lowercase();
    if let Some(exe_end) = lower.find(".exe").map(|index| index + 4) {
        let exe_path = &normalized[..exe_end];
        if let Some(base) = exe_path.rsplit('/').next() {
            push_process_name_candidate(&mut candidates, base);
        }
    }

    candidates
}

#[cfg(any(test, not(target_os = "windows")))]
pub(crate) fn push_process_name_candidate(candidates: &mut Vec<String>, value: &str) {
    let candidate = value.trim();
    if candidate.is_empty() {
        return;
    }
    if !candidates
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(candidate))
    {
        candidates.push(candidate.to_string());
    }
}
