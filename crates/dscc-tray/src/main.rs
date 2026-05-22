#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::Result;

#[cfg(windows)]
fn main() -> Result<()> {
    windows_tray::run()
}

#[cfg(not(windows))]
fn main() -> Result<()> {
    println!("DualSense Command Center tray is currently implemented for Windows.");
    Ok(())
}

#[cfg(windows)]
mod windows_tray {
    use anyhow::{anyhow, Context, Result};
    use serde::Deserialize;
    use std::{
        env,
        ffi::OsStr,
        io::{Read, Write},
        net::{SocketAddr, TcpStream},
        os::windows::{ffi::OsStrExt, process::CommandExt},
        path::PathBuf,
        process::{Child, Command},
        ptr::{null, null_mut},
        sync::{
            atomic::{AtomicU32, Ordering},
            mpsc::{self, Receiver, SyncSender, TrySendError},
            Arc, Mutex, OnceLock,
        },
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject,
            DrawTextW, Ellipse, EndPaint, FillRect, InvalidateRect, RoundRect, SelectObject,
            SetBkMode, SetTextColor, SetWindowRgn, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
            DEFAULT_CHARSET, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
            FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID,
            TRANSPARENT,
        },
        System::{LibraryLoader::GetModuleHandleW, Threading::CREATE_NO_WINDOW},
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT},
            Shell::{
                ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP,
                NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW,
                NOTIFYICON_VERSION_4,
            },
            WindowsAndMessaging::{
                CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow,
                DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, GetSystemMetrics,
                GetWindowLongPtrW, LoadCursorW, LoadIconW, MessageBoxW, PostMessageW,
                PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetCursor,
                SetForegroundWindow, SetWindowLongPtrW, ShowWindow, CREATESTRUCTW, CS_DROPSHADOW,
                CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HICON, IDC_ARROW,
                IDI_APPLICATION, MB_ICONERROR, MB_OK, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
                SW_SHOWNORMAL, WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_COMMAND, WM_CONTEXTMENU,
                WM_DESTROY, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_NCCREATE, WM_NCDESTROY, WM_NULL, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR,
                WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            },
        },
    };

    const TRAY_ICON_ICO: &[u8] = include_bytes!("../assets/dscc-tray.ico");
    const DASHBOARD_URL: &str = "http://127.0.0.1:43473/#/games";
    const HAPTICS_URL: &str = "http://127.0.0.1:43473/#/adaptive-triggers-haptics";
    const BUTTON_MAPPING_URL: &str = "http://127.0.0.1:43473/#/button-mapping";
    const STATUS_PATH: &str = "/api/status";
    const SNAPSHOT_PATH: &str = "/api/snapshot";
    const API_HOST: &str = "127.0.0.1:43473";
    const RELEASES_URL: &str = "https://github.com/shiftedx/dualsense-command/releases/latest";
    const OPEN_UI_DEBOUNCE_MS: u64 = 650;
    const TRAY_HEALTH_STALE_AFTER: Duration = Duration::from_secs(4);
    const TRAY_HEALTH_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
    const TRAY_ICON_ID: u32 = 1;
    const WM_TRAYICON: u32 = WM_APP + 1;
    const CMD_OPEN_UI: usize = 1001;
    const CMD_START: usize = 1002;
    const CMD_STOP: usize = 1003;
    const CMD_RESTART: usize = 1004;
    const CMD_QUIT: usize = 1005;
    const CMD_OPEN_BUTTON_MAPPING: usize = 1006;
    const CMD_CHECK_UPDATES: usize = 1007;
    const CMD_OPEN_HAPTICS: usize = 1008;
    const NIN_KEYSELECT: u32 = NIN_SELECT + 1;
    const MENU_WIDTH: u32 = 248;
    const MENU_HEADER_HEIGHT: u32 = 36;
    const MENU_READOUT_HEIGHT: u32 = 32;
    const MENU_ITEM_HEIGHT: u32 = 30;
    const MENU_SEPARATOR_HEIGHT: u32 = 6;
    const MENU_CORNER_RADIUS: i32 = 12;
    const COLOR_OBSIDIAN: COLORREF = rgb(18, 19, 22);
    const COLOR_CARBON: COLORREF = rgb(30, 31, 35);
    const COLOR_SELECTED: COLORREF = rgb(48, 50, 57);
    const COLOR_ACTUATION: COLORREF = rgb(0, 112, 204);
    const COLOR_HAPTIC: COLORREF = rgb(242, 243, 245);
    const COLOR_TUNGSTEN: COLORREF = rgb(181, 186, 193);
    const COLOR_OVERDRIVE: COLORREF = rgb(240, 62, 62);
    const COLOR_READY: COLORREF = rgb(34, 197, 94);
    const COLOR_LINE: COLORREF = rgb(62, 64, 72);
    const COLOR_DISABLED: COLORREF = rgb(118, 122, 132);
    const COLOR_WHITE: COLORREF = rgb(255, 255, 255);

    static STATE: OnceLock<Mutex<TrayState>> = OnceLock::new();
    static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

    const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
        red as COLORREF | ((green as COLORREF) << 8) | ((blue as COLORREF) << 16)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TrayIconAction {
        OpenUi,
        ShowMenu,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LaunchMode {
        Interactive,
        Startup,
    }

    impl LaunchMode {
        fn from_args() -> Self {
            if env::args_os()
                .skip(1)
                .any(|arg| arg == OsStr::new("--startup"))
            {
                Self::Startup
            } else {
                Self::Interactive
            }
        }

        fn opens_ui(self) -> bool {
            self == Self::Interactive
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TrayMenuKind {
        Header,
        Readout,
        Action,
        Separator,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TrayMenuAccent {
        Brand,
        Ready,
        Danger,
        Neutral,
    }

    #[derive(Debug)]
    struct TrayMenuDescriptor {
        kind: TrayMenuKind,
        label: String,
        detail: String,
        accent: TrayMenuAccent,
    }

    impl TrayMenuDescriptor {
        fn new(
            kind: TrayMenuKind,
            label: impl Into<String>,
            detail: impl Into<String>,
            accent: TrayMenuAccent,
        ) -> Self {
            Self {
                kind,
                label: label.into(),
                detail: detail.into(),
                accent,
            }
        }
    }

    #[derive(Debug)]
    struct TrayMenuEntry {
        command: usize,
        descriptor: TrayMenuDescriptor,
        disabled: bool,
    }

    #[derive(Debug)]
    struct TrayPopupState {
        owner: HWND,
        entries: Vec<TrayMenuEntry>,
        hover_index: Option<usize>,
    }

    impl TrayPopupState {
        fn new(owner: HWND, entries: Vec<TrayMenuEntry>) -> Self {
            Self {
                owner,
                entries,
                hover_index: None,
            }
        }

        fn height(&self) -> i32 {
            tray_menu_height(&self.entries)
        }

        fn item_rect(&self, index: usize) -> Option<RECT> {
            if index >= self.entries.len() {
                return None;
            }

            let top = self
                .entries
                .iter()
                .take(index)
                .map(|entry| menu_item_height(entry.descriptor.kind) as i32)
                .sum::<i32>();
            let bottom = top + menu_item_height(self.entries[index].descriptor.kind) as i32;
            Some(RECT {
                left: 0,
                top,
                right: MENU_WIDTH as i32,
                bottom,
            })
        }

        fn item_at(&self, y: i32) -> Option<usize> {
            let mut top = 0;
            for (index, entry) in self.entries.iter().enumerate() {
                let bottom = top + menu_item_height(entry.descriptor.kind) as i32;
                if y >= top && y < bottom {
                    return Some(index);
                }
                top = bottom;
            }
            None
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TrayHealthSummary {
        agent_running: bool,
        agent_label: String,
        agent_detail: String,
        agent_accent: TrayMenuAccent,
        profile_label: String,
        profile_detail: String,
        profile_accent: TrayMenuAccent,
        controller_label: String,
        controller_detail: String,
        controller_accent: TrayMenuAccent,
        diagnostics_label: String,
        diagnostics_detail: String,
        diagnostics_accent: TrayMenuAccent,
    }

    #[derive(Debug, Clone)]
    struct TrayHealthCache {
        summary: TrayHealthSummary,
        refreshed_at: Instant,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotDto {
        status: TraySnapshotStatusDto,
        #[serde(default)]
        profiles: Vec<TraySnapshotProfileDto>,
        #[serde(default)]
        controllers: Vec<TraySnapshotControllerDto>,
        #[serde(default, alias = "profileResolution")]
        profile_resolution: TraySnapshotProfileResolutionDto,
        diagnostics: TraySnapshotDiagnosticsDto,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotStatusDto {
        #[serde(default)]
        version: String,
        #[serde(default)]
        active_profile_id: Option<String>,
        #[serde(default)]
        active_adapter_id: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotProfileDto {
        id: String,
        name: String,
        #[serde(default)]
        active: bool,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotControllerDto {
        id: String,
        name: String,
        #[serde(default)]
        model: String,
        #[serde(default)]
        transport: String,
        #[serde(default)]
        connected: bool,
    }

    #[derive(Debug, Default, Deserialize)]
    struct TraySnapshotProfileResolutionDto {
        #[serde(default, alias = "controllerId")]
        controller_id: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotDiagnosticsDto {
        #[serde(default)]
        checks: Vec<TraySnapshotHealthCheckDto>,
    }

    #[derive(Debug, Deserialize)]
    struct TraySnapshotHealthCheckDto {
        status: String,
    }

    fn tray_menu_entries(summary: &TrayHealthSummary, owned_agent: bool) -> Vec<TrayMenuEntry> {
        let mut entries = vec![
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Header,
                    "DualSense Command Center",
                    "Local haptics control",
                    TrayMenuAccent::Brand,
                ),
                disabled: true,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Readout,
                    summary.agent_label.clone(),
                    summary.agent_detail.clone(),
                    summary.agent_accent,
                ),
                disabled: true,
            },
        ];

        entries.extend([
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Readout,
                    summary.profile_label.clone(),
                    summary.profile_detail.clone(),
                    summary.profile_accent,
                ),
                disabled: true,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Readout,
                    summary.controller_label.clone(),
                    summary.controller_detail.clone(),
                    summary.controller_accent,
                ),
                disabled: true,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Readout,
                    summary.diagnostics_label.clone(),
                    summary.diagnostics_detail.clone(),
                    summary.diagnostics_accent,
                ),
                disabled: true,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: separator_descriptor(),
                disabled: true,
            },
            TrayMenuEntry {
                command: CMD_OPEN_UI,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Dashboard",
                    "Open controller and profile overview",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: CMD_OPEN_HAPTICS,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Triggers & Haptics",
                    "Open adaptive trigger tuning",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: CMD_OPEN_BUTTON_MAPPING,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Button Mapping",
                    "Open button layout editor",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: CMD_CHECK_UPDATES,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Check for Updates...",
                    "Open latest GitHub release",
                    TrayMenuAccent::Neutral,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: separator_descriptor(),
                disabled: true,
            },
        ]);

        if summary.agent_running && owned_agent {
            entries.extend([
                TrayMenuEntry {
                    command: CMD_STOP,
                    descriptor: TrayMenuDescriptor::new(
                        TrayMenuKind::Action,
                        "Stop Agent",
                        "Stop local runtime",
                        TrayMenuAccent::Danger,
                    ),
                    disabled: false,
                },
                TrayMenuEntry {
                    command: CMD_RESTART,
                    descriptor: TrayMenuDescriptor::new(
                        TrayMenuKind::Action,
                        "Restart Agent",
                        "Refresh local runtime",
                        TrayMenuAccent::Brand,
                    ),
                    disabled: false,
                },
                TrayMenuEntry {
                    command: 0,
                    descriptor: separator_descriptor(),
                    disabled: true,
                },
            ]);
        } else if !summary.agent_running {
            entries.extend([
                TrayMenuEntry {
                    command: CMD_START,
                    descriptor: TrayMenuDescriptor::new(
                        TrayMenuKind::Action,
                        "Start Agent",
                        "Launch local runtime",
                        TrayMenuAccent::Ready,
                    ),
                    disabled: false,
                },
                TrayMenuEntry {
                    command: 0,
                    descriptor: separator_descriptor(),
                    disabled: true,
                },
            ]);
        }

        entries.push(TrayMenuEntry {
            command: CMD_QUIT,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Action,
                "Quit DSCC",
                if owned_agent {
                    "Stop owned agent and tray"
                } else {
                    "Close tray"
                },
                TrayMenuAccent::Danger,
            ),
            disabled: false,
        });
        entries
    }

    fn separator_descriptor() -> TrayMenuDescriptor {
        TrayMenuDescriptor::new(TrayMenuKind::Separator, "", "", TrayMenuAccent::Neutral)
    }

    struct TrayState {
        agent: Option<Child>,
        install_dir: PathBuf,
        last_open_ui: Option<(Instant, String)>,
        health_cache: Arc<Mutex<TrayHealthCache>>,
        health_refresh_tx: SyncSender<()>,
    }

    impl TrayState {
        fn new() -> Result<Self> {
            let exe = env::current_exe().context("could not resolve tray executable path")?;
            let install_dir = exe
                .parent()
                .ok_or_else(|| anyhow!("tray executable has no parent directory"))?
                .to_path_buf();
            let health_cache = Arc::new(Mutex::new(TrayHealthCache {
                summary: refreshing_health_summary(),
                refreshed_at: Instant::now() - TRAY_HEALTH_STALE_AFTER,
            }));
            let (health_refresh_tx, health_refresh_rx) = mpsc::sync_channel(1);
            spawn_tray_health_worker(health_cache.clone(), health_refresh_rx);
            Ok(Self {
                agent: None,
                install_dir,
                last_open_ui: None,
                health_cache,
                health_refresh_tx,
            })
        }

        fn claim_open_ui(&mut self, url: &str) -> bool {
            let now = Instant::now();
            if self.last_open_ui.as_ref().is_some_and(|(last, last_url)| {
                last_url == url
                    && now.duration_since(*last) < Duration::from_millis(OPEN_UI_DEBOUNCE_MS)
            }) {
                return false;
            }
            self.last_open_ui = Some((now, url.to_string()));
            true
        }

        fn agent_path(&self) -> PathBuf {
            self.install_dir.join("dscc-agent.exe")
        }

        fn web_dist(&self) -> PathBuf {
            self.install_dir.join("web").join("dist")
        }

        fn prune_exited_child(&mut self) {
            if self
                .agent
                .as_mut()
                .and_then(|child| child.try_wait().ok())
                .flatten()
                .is_some()
            {
                self.agent = None;
            }
        }

        fn owns_agent(&mut self) -> bool {
            self.prune_exited_child();
            self.agent.is_some()
        }

        fn request_health_refresh(&self) {
            match self.health_refresh_tx.try_send(()) {
                Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
            }
        }

        fn cached_health_summary(&self) -> TrayHealthSummary {
            let cached = match self.health_cache.lock() {
                Ok(cache) => cache.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            if cached.refreshed_at.elapsed() >= TRAY_HEALTH_STALE_AFTER {
                self.request_health_refresh();
            }
            cached.summary
        }

        fn menu_health(&mut self) -> (TrayHealthSummary, bool) {
            let owned_agent = self.owns_agent();
            (self.cached_health_summary(), owned_agent)
        }

        fn ensure_agent(&mut self) -> Result<()> {
            self.prune_exited_child();
            if agent_is_healthy() {
                self.request_health_refresh();
                return Ok(());
            }

            let agent_path = self.agent_path();
            if !agent_path.exists() {
                return Err(anyhow!("{} was not found", agent_path.display()));
            }

            let mut command = Command::new(agent_path);
            command
                .current_dir(&self.install_dir)
                .env("DSCC_WEB_DIST", self.web_dist())
                .env("DSCC_AGENT_ADDR", agent_spawn_addr())
                .creation_flags(CREATE_NO_WINDOW);

            self.agent = Some(command.spawn().context("could not start dscc-agent.exe")?);
            wait_for_agent(Duration::from_secs(4));
            self.request_health_refresh();
            Ok(())
        }

        fn stop_agent(&mut self) {
            if let Some(mut child) = self.agent.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.request_health_refresh();
        }

        fn restart_agent(&mut self) -> Result<()> {
            self.stop_agent();
            thread::sleep(Duration::from_millis(350));
            self.ensure_agent()
        }
    }

    pub fn run() -> Result<()> {
        let launch_mode = LaunchMode::from_args();
        if activate_existing_instance(launch_mode) {
            return Ok(());
        }

        STATE
            .set(Mutex::new(TrayState::new()?))
            .map_err(|_| anyhow!("tray state was already initialized"))?;

        TASKBAR_CREATED_MESSAGE.store(register_taskbar_created_message(), Ordering::Relaxed);
        register_popup_window_class()?;
        let hwnd = create_hidden_window()?;
        add_tray_icon_with_retry(hwnd)?;

        match with_state(|state| state.ensure_agent()) {
            Ok(_) if launch_mode.opens_ui() => open_browser(hwnd),
            Ok(_) => {}
            Err(error) if launch_mode.opens_ui() => show_error(hwnd, &error.to_string()),
            Err(_) => {}
        }

        message_loop();
        Ok(())
    }

    fn create_hidden_window() -> Result<HWND> {
        unsafe {
            let instance = GetModuleHandleW(null());
            let class_name = wide_null("DSCCTrayWindow");
            let window_title = wide_null("DualSense Command Center");
            let wndclass = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                hIcon: load_tray_icon(32),
                hCursor: arrow_cursor(),
                ..Default::default()
            };

            if RegisterClassW(&wndclass) == 0 {
                return Err(anyhow!("could not register tray window class"));
            }

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                0,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                return Err(anyhow!("could not create tray window"));
            }

            Ok(hwnd)
        }
    }

    fn register_popup_window_class() -> Result<()> {
        unsafe {
            let instance = GetModuleHandleW(null());
            let class_name = wide_null("DSCCTrayPopupWindow");
            let wndclass = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
                lpfnWndProc: Some(popup_window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                hIcon: load_tray_icon(16),
                hCursor: arrow_cursor(),
                ..Default::default()
            };

            if RegisterClassW(&wndclass) == 0 {
                return Err(anyhow!("could not register tray popup window class"));
            }

            Ok(())
        }
    }

    fn activate_existing_instance(launch_mode: LaunchMode) -> bool {
        unsafe {
            let class_name = wide_null("DSCCTrayWindow");
            let hwnd = FindWindowW(class_name.as_ptr(), null());
            if hwnd.is_null() {
                return false;
            }

            if launch_mode.opens_ui() {
                PostMessageW(hwnd, WM_COMMAND, CMD_OPEN_UI, 0);
            }
            true
        }
    }

    fn register_taskbar_created_message() -> u32 {
        unsafe {
            let message = wide_null("TaskbarCreated");
            RegisterWindowMessageW(message.as_ptr())
        }
    }

    fn add_tray_icon_with_retry(hwnd: HWND) -> Result<()> {
        let started = std::time::Instant::now();
        let timeout = Duration::from_secs(10);

        loop {
            match add_tray_icon(hwnd) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    if started.elapsed() >= timeout {
                        return Err(anyhow!(
                            "could not add DSCC tray icon after waiting for the Windows notification area: {error}"
                        ));
                    }
                }
            }

            thread::sleep(Duration::from_millis(250));
        }
    }

    fn add_tray_icon(hwnd: HWND) -> Result<()> {
        unsafe {
            let mut data = notify_icon_data(hwnd);
            if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                return Err(anyhow!("could not add DSCC tray icon"));
            }
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
            Ok(())
        }
    }

    fn remove_tray_icon(hwnd: HWND) {
        unsafe {
            let data = notify_icon_data(hwnd);
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }

    fn notify_icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: load_tray_icon(32),
            ..Default::default()
        };
        copy_wide_fixed(&mut data.szTip, "DualSense Command Center");
        data
    }

    fn load_tray_icon(size: i32) -> HICON {
        icon_image_from_ico(TRAY_ICON_ICO, size)
            .map(|image| unsafe {
                CreateIconFromResourceEx(
                    image.as_ptr(),
                    image.len() as u32,
                    1,
                    0x0003_0000,
                    size,
                    size,
                    0,
                )
            })
            .filter(|icon| !icon.is_null())
            .unwrap_or_else(|| unsafe { LoadIconW(null_mut(), IDI_APPLICATION) })
    }

    unsafe fn arrow_cursor() -> windows_sys::Win32::UI::WindowsAndMessaging::HCURSOR {
        LoadCursorW(null_mut(), IDC_ARROW)
    }

    fn icon_image_from_ico(data: &'static [u8], desired_size: i32) -> Option<&'static [u8]> {
        if data.len() < 6 || u16::from_le_bytes([data[2], data[3]]) != 1 {
            return None;
        }
        let count = u16::from_le_bytes([data[4], data[5]]) as usize;
        let desired_size = desired_size.max(1) as usize;
        let mut best: Option<(usize, usize, usize, usize)> = None;
        for index in 0..count {
            let entry = 6 + index * 16;
            if entry + 16 > data.len() {
                break;
            }
            let width = if data[entry] == 0 {
                256
            } else {
                data[entry] as usize
            };
            let bytes = u32::from_le_bytes([
                data[entry + 8],
                data[entry + 9],
                data[entry + 10],
                data[entry + 11],
            ]) as usize;
            let offset = u32::from_le_bytes([
                data[entry + 12],
                data[entry + 13],
                data[entry + 14],
                data[entry + 15],
            ]) as usize;
            if bytes == 0 || offset.checked_add(bytes).is_none_or(|end| end > data.len()) {
                continue;
            }
            let score = width.abs_diff(desired_size);
            if best.as_ref().is_none_or(|(best_score, best_width, _, _)| {
                score < *best_score || (score == *best_score && width > *best_width)
            }) {
                best = Some((score, width, offset, bytes));
            }
        }
        let (_, _, offset, bytes) = best?;
        data.get(offset..offset + bytes)
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let taskbar_created_message = TASKBAR_CREATED_MESSAGE.load(Ordering::Relaxed);
        if taskbar_created_message != 0 && msg == taskbar_created_message {
            let _ = add_tray_icon(hwnd);
            return 0;
        }

        match msg {
            WM_COMMAND => {
                handle_command(hwnd, wparam & 0xffff);
                0
            }
            WM_TRAYICON => {
                if let Some(action) = tray_icon_action(wparam, lparam) {
                    handle_tray_icon_action(hwnd, action);
                }
                0
            }
            WM_DESTROY => {
                remove_tray_icon(hwnd);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe extern "system" fn popup_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let create = &*(lparam as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                1
            }
            WM_PAINT => {
                paint_tray_popup(hwnd);
                0
            }
            WM_MOUSEMOVE => {
                update_popup_hover(hwnd, mouse_y(lparam));
                let mut event = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut event);
                0
            }
            WM_MOUSELEAVE => {
                if let Some(state) = popup_state_mut(hwnd) {
                    if state.hover_index.take().is_some() {
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                0
            }
            WM_SETCURSOR => {
                SetCursor(arrow_cursor());
                1
            }
            WM_LBUTTONUP => {
                let command = popup_state_mut(hwnd)
                    .and_then(|state| state.item_at(mouse_y(lparam)).map(|index| (state, index)))
                    .and_then(|(state, index)| {
                        let entry = &state.entries[index];
                        (!entry.disabled && entry.command != 0)
                            .then_some((state.owner, entry.command))
                    });
                DestroyWindow(hwnd);
                if let Some((owner, command)) = command {
                    PostMessageW(owner, WM_COMMAND, command, 0);
                }
                0
            }
            WM_ACTIVATE => {
                if loword(wparam) == WA_INACTIVE {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_KILLFOCUS => {
                DestroyWindow(hwnd);
                0
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayPopupState;
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn popup_state_mut(hwnd: HWND) -> Option<&'static mut TrayPopupState> {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayPopupState;
        if ptr.is_null() {
            None
        } else {
            Some(&mut *ptr)
        }
    }

    unsafe fn paint_tray_popup(hwnd: HWND) {
        let mut paint = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut paint);
        if !hdc.is_null() {
            if let Some(state) = popup_state_mut(hwnd) {
                draw_tray_popup(hdc, state);
            }
        }
        EndPaint(hwnd, &paint);
    }

    unsafe fn update_popup_hover(hwnd: HWND, y: i32) {
        let Some(state) = popup_state_mut(hwnd) else {
            return;
        };
        let next = state.item_at(y).filter(|index| {
            let entry = &state.entries[*index];
            entry.command != 0 && !entry.disabled
        });
        if state.hover_index != next {
            state.hover_index = next;
            InvalidateRect(hwnd, null(), 0);
        }
    }

    unsafe fn draw_tray_popup(hdc: windows_sys::Win32::Graphics::Gdi::HDC, state: &TrayPopupState) {
        let panel_rect = RECT {
            left: 0,
            top: 0,
            right: MENU_WIDTH as i32,
            bottom: state.height(),
        };
        draw_round_panel(hdc, panel_rect, COLOR_CARBON, COLOR_LINE);
        for (index, entry) in state.entries.iter().enumerate() {
            let Some(rect) = state.item_rect(index) else {
                continue;
            };
            match entry.descriptor.kind {
                TrayMenuKind::Header => draw_menu_header(hdc, rect, &entry.descriptor),
                TrayMenuKind::Readout => draw_menu_readout(hdc, rect, &entry.descriptor),
                TrayMenuKind::Action => draw_menu_action(
                    hdc,
                    rect,
                    &entry.descriptor,
                    state.hover_index == Some(index),
                    entry.disabled,
                ),
                TrayMenuKind::Separator => draw_menu_separator(hdc, rect),
            }
        }
        draw_round_panel_outline(hdc, panel_rect, COLOR_LINE);
    }

    fn mouse_y(lparam: LPARAM) -> i32 {
        (((lparam as u32) >> 16) as i16) as i32
    }

    fn tray_icon_action(wparam: WPARAM, lparam: LPARAM) -> Option<TrayIconAction> {
        let packed_lparam = lparam as usize;
        if hiword(packed_lparam) == TRAY_ICON_ID {
            if let Some(action) = action_for_tray_event(loword(packed_lparam)) {
                return Some(action);
            }
        }

        if wparam as u32 == TRAY_ICON_ID {
            return action_for_tray_event(lparam as u32);
        }

        None
    }

    fn action_for_tray_event(event: u32) -> Option<TrayIconAction> {
        match event {
            WM_CONTEXTMENU | WM_RBUTTONUP => Some(TrayIconAction::ShowMenu),
            WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | NIN_KEYSELECT => {
                Some(TrayIconAction::OpenUi)
            }
            _ => None,
        }
    }

    fn loword(value: usize) -> u32 {
        (value & 0xffff) as u32
    }

    fn hiword(value: usize) -> u32 {
        ((value >> 16) & 0xffff) as u32
    }

    fn handle_tray_icon_action(hwnd: HWND, action: TrayIconAction) {
        match action {
            TrayIconAction::OpenUi => open_ui(hwnd),
            TrayIconAction::ShowMenu => show_menu(hwnd),
        }
    }

    fn show_menu(hwnd: HWND) {
        unsafe {
            let (summary, owned_agent) = with_state(|state| Ok(state.menu_health()))
                .unwrap_or_else(|_| (refreshing_health_summary(), false));
            let entries = tray_menu_entries(&summary, owned_agent);
            let popup_state = Box::new(TrayPopupState::new(hwnd, entries));
            let height = popup_state.height();
            let width = MENU_WIDTH as i32;

            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 {
                let screen_width = GetSystemMetrics(SM_CXSCREEN);
                let screen_height = GetSystemMetrics(SM_CYSCREEN);
                let x = (point.x - width + 12).clamp(6, (screen_width - width - 6).max(6));
                let y = (point.y - height - 8).clamp(6, (screen_height - height - 6).max(6));
                let popup_ptr = Box::into_raw(popup_state);
                let class_name = wide_null("DSCCTrayPopupWindow");
                let window_title = wide_null("DualSense Command Center");
                let popup = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    class_name.as_ptr(),
                    window_title.as_ptr(),
                    WS_POPUP,
                    x,
                    y,
                    width,
                    height,
                    hwnd,
                    null_mut(),
                    GetModuleHandleW(null()),
                    popup_ptr.cast(),
                );
                if popup.is_null() {
                    drop(Box::from_raw(popup_ptr));
                    return;
                }
                apply_popup_shape(popup, width, height);
                SetForegroundWindow(hwnd);
                ShowWindow(popup, SW_SHOW);
                SetForegroundWindow(popup);
                PostMessageW(hwnd, WM_NULL, 0, 0);
            }
        }
    }

    unsafe fn apply_popup_shape(hwnd: HWND, width: i32, height: i32) {
        let region = CreateRoundRectRgn(
            0,
            0,
            width + 1,
            height + 1,
            MENU_CORNER_RADIUS,
            MENU_CORNER_RADIUS,
        );
        if region.is_null() {
            return;
        }

        if SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
        }
    }

    fn menu_item_height(kind: TrayMenuKind) -> u32 {
        match kind {
            TrayMenuKind::Header => MENU_HEADER_HEIGHT,
            TrayMenuKind::Readout => MENU_READOUT_HEIGHT,
            TrayMenuKind::Action => MENU_ITEM_HEIGHT,
            TrayMenuKind::Separator => MENU_SEPARATOR_HEIGHT,
        }
    }

    fn tray_menu_height(entries: &[TrayMenuEntry]) -> i32 {
        entries
            .iter()
            .map(|entry| menu_item_height(entry.descriptor.kind) as i32)
            .sum()
    }

    unsafe fn draw_menu_header(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        descriptor: &TrayMenuDescriptor,
    ) {
        fill_rect(hdc, rect, COLOR_OBSIDIAN);
        fill_rect(
            hdc,
            RECT {
                left: rect.left,
                top: rect.bottom - 1,
                right: rect.right,
                bottom: rect.bottom,
            },
            COLOR_LINE,
        );

        let title_rect = RECT {
            left: rect.left + 28,
            top: rect.top + 4,
            right: rect.right - 14,
            bottom: rect.top + 20,
        };
        draw_text_line(
            hdc,
            &descriptor.label,
            title_rect,
            COLOR_WHITE,
            13,
            FW_SEMIBOLD,
        );

        draw_dot(hdc, rect.left + 14, rect.top + 12, 9, COLOR_ACTUATION);
        let status_rect = RECT {
            left: rect.left + 28,
            top: rect.top + 19,
            right: rect.right - 14,
            bottom: rect.bottom - 3,
        };
        draw_text_line(
            hdc,
            &descriptor.detail,
            status_rect,
            COLOR_TUNGSTEN,
            9,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_readout(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        descriptor: &TrayMenuDescriptor,
    ) {
        fill_rect(hdc, rect, COLOR_CARBON);
        draw_dot(
            hdc,
            rect.left + 16,
            rect.top + 12,
            8,
            menu_accent_color(descriptor.accent),
        );

        let label_rect = RECT {
            left: rect.left + 32,
            top: rect.top + 2,
            right: rect.right - 14,
            bottom: rect.top + 17,
        };
        draw_text_line(
            hdc,
            &descriptor.label,
            label_rect,
            COLOR_HAPTIC,
            11,
            FW_SEMIBOLD,
        );

        let detail_rect = RECT {
            left: rect.left + 32,
            top: rect.top + 16,
            right: rect.right - 14,
            bottom: rect.bottom - 3,
        };
        draw_text_line(
            hdc,
            &descriptor.detail,
            detail_rect,
            COLOR_TUNGSTEN,
            9,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_action(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        descriptor: &TrayMenuDescriptor,
        selected: bool,
        disabled: bool,
    ) {
        fill_rect(
            hdc,
            rect,
            if selected {
                COLOR_SELECTED
            } else {
                COLOR_CARBON
            },
        );

        let label_color = if disabled {
            COLOR_DISABLED
        } else if descriptor.accent == TrayMenuAccent::Danger {
            COLOR_OVERDRIVE
        } else {
            COLOR_HAPTIC
        };
        let detail_color = if disabled {
            COLOR_DISABLED
        } else {
            COLOR_TUNGSTEN
        };
        let label_rect = RECT {
            left: rect.left + 18,
            top: rect.top + 1,
            right: rect.right - 14,
            bottom: rect.top + 16,
        };
        draw_text_line(
            hdc,
            &descriptor.label,
            label_rect,
            label_color,
            11,
            FW_SEMIBOLD,
        );

        let detail_rect = RECT {
            left: rect.left + 18,
            top: rect.top + 15,
            right: rect.right - 14,
            bottom: rect.bottom - 1,
        };
        draw_text_line(
            hdc,
            &descriptor.detail,
            detail_rect,
            detail_color,
            9,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_separator(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT) {
        fill_rect(hdc, rect, COLOR_CARBON);
        let top = rect.top + ((rect.bottom - rect.top) / 2);
        fill_rect(
            hdc,
            RECT {
                left: rect.left + 12,
                top,
                right: rect.right - 12,
                bottom: top + 1,
            },
            COLOR_LINE,
        );
    }

    unsafe fn draw_round_panel(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        fill: COLORREF,
        stroke: COLORREF,
    ) {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, stroke);
        if brush.is_null() || pen.is_null() {
            if !brush.is_null() {
                DeleteObject(brush);
            }
            if !pen.is_null() {
                DeleteObject(pen);
            }
            fill_rect(hdc, rect, fill);
            return;
        }

        let previous_brush = SelectObject(hdc, brush);
        let previous_pen = SelectObject(hdc, pen);
        RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            MENU_CORNER_RADIUS,
            MENU_CORNER_RADIUS,
        );
        if !previous_pen.is_null() {
            SelectObject(hdc, previous_pen);
        }
        if !previous_brush.is_null() {
            SelectObject(hdc, previous_brush);
        }
        DeleteObject(pen);
        DeleteObject(brush);
    }

    unsafe fn draw_round_panel_outline(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        color: COLORREF,
    ) {
        fill_rect(
            hdc,
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.top + 1,
            },
            color,
        );
        fill_rect(
            hdc,
            RECT {
                left: rect.left,
                top: rect.bottom - 1,
                right: rect.right,
                bottom: rect.bottom,
            },
            color,
        );
        fill_rect(
            hdc,
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.left + 1,
                bottom: rect.bottom,
            },
            color,
        );
        fill_rect(
            hdc,
            RECT {
                left: rect.right - 1,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            },
            color,
        );
    }

    unsafe fn fill_rect(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT, color: COLORREF) {
        let brush = CreateSolidBrush(color);
        if !brush.is_null() {
            FillRect(hdc, &rect, brush);
            DeleteObject(brush);
        }
    }

    unsafe fn draw_dot(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        left: i32,
        top: i32,
        size: i32,
        color: COLORREF,
    ) {
        let brush = CreateSolidBrush(color);
        if brush.is_null() {
            return;
        }
        let previous = SelectObject(hdc, brush);
        Ellipse(hdc, left, top, left + size, top + size);
        if !previous.is_null() {
            SelectObject(hdc, previous);
        }
        DeleteObject(brush);
    }

    unsafe fn draw_text_line(
        hdc: windows_sys::Win32::Graphics::Gdi::HDC,
        text: &str,
        mut rect: RECT,
        color: COLORREF,
        height: i32,
        weight: u32,
    ) {
        let face = wide_null("Segoe UI");
        let font = CreateFontW(
            -height,
            0,
            0,
            0,
            weight as i32,
            0,
            0,
            0,
            u32::from(DEFAULT_CHARSET),
            u32::from(OUT_DEFAULT_PRECIS),
            u32::from(CLIP_DEFAULT_PRECIS),
            u32::from(CLEARTYPE_QUALITY),
            u32::from(FF_DONTCARE),
            face.as_ptr(),
        );
        let previous = if font.is_null() {
            null_mut()
        } else {
            SelectObject(hdc, font)
        };
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, color);
        let text = wide_text(text);
        DrawTextW(
            hdc,
            text.as_ptr(),
            text.len() as i32,
            &mut rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        if !previous.is_null() {
            SelectObject(hdc, previous);
        }
        if !font.is_null() {
            DeleteObject(font);
        }
    }

    fn menu_accent_color(accent: TrayMenuAccent) -> COLORREF {
        match accent {
            TrayMenuAccent::Brand => COLOR_ACTUATION,
            TrayMenuAccent::Ready => COLOR_READY,
            TrayMenuAccent::Danger => COLOR_OVERDRIVE,
            TrayMenuAccent::Neutral => COLOR_TUNGSTEN,
        }
    }

    fn refreshing_health_summary() -> TrayHealthSummary {
        TrayHealthSummary {
            agent_running: false,
            agent_label: "Agent Status".to_string(),
            agent_detail: "Refreshing local runtime state".to_string(),
            agent_accent: TrayMenuAccent::Neutral,
            profile_label: "Profile Pending".to_string(),
            profile_detail: "Waiting for snapshot".to_string(),
            profile_accent: TrayMenuAccent::Neutral,
            controller_label: "Controller Pending".to_string(),
            controller_detail: "Waiting for snapshot".to_string(),
            controller_accent: TrayMenuAccent::Neutral,
            diagnostics_label: "Diagnostics Pending".to_string(),
            diagnostics_detail: "Waiting for health checks".to_string(),
            diagnostics_accent: TrayMenuAccent::Neutral,
        }
    }

    fn offline_health_summary() -> TrayHealthSummary {
        TrayHealthSummary {
            agent_running: false,
            agent_label: "Agent Offline".to_string(),
            agent_detail: "Start the agent to enable controller control".to_string(),
            agent_accent: TrayMenuAccent::Danger,
            profile_label: "Profile Unavailable".to_string(),
            profile_detail: "Start the agent to read profile state".to_string(),
            profile_accent: TrayMenuAccent::Neutral,
            controller_label: "Controller Unavailable".to_string(),
            controller_detail: "Start the agent to read controller state".to_string(),
            controller_accent: TrayMenuAccent::Neutral,
            diagnostics_label: "Diagnostics Unavailable".to_string(),
            diagnostics_detail: "Waiting for the local runtime".to_string(),
            diagnostics_accent: TrayMenuAccent::Neutral,
        }
    }

    fn spawn_tray_health_worker(cache: Arc<Mutex<TrayHealthCache>>, refresh_rx: Receiver<()>) {
        thread::spawn(move || {
            refresh_tray_health_cache(&cache);
            while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) =
                refresh_rx.recv_timeout(TRAY_HEALTH_REFRESH_INTERVAL)
            {
                while refresh_rx.try_recv().is_ok() {}
                refresh_tray_health_cache(&cache);
            }
        });
    }

    fn refresh_tray_health_cache(cache: &Arc<Mutex<TrayHealthCache>>) {
        let summary = fetch_tray_health_summary().unwrap_or_else(offline_health_summary);
        let mut cache = match cache.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache.summary = summary;
        cache.refreshed_at = Instant::now();
    }

    fn fetch_tray_health_summary() -> Option<TrayHealthSummary> {
        let body = http_get_body(SNAPSHOT_PATH, Duration::from_millis(900))?;
        let snapshot = serde_json::from_str::<TraySnapshotDto>(&body).ok()?;
        Some(tray_health_summary_from_snapshot(&snapshot))
    }

    fn tray_health_summary_from_snapshot(snapshot: &TraySnapshotDto) -> TrayHealthSummary {
        let version = if snapshot.status.version.trim().is_empty() {
            "unknown"
        } else {
            snapshot.status.version.trim()
        };
        let active_profile_id = snapshot.status.active_profile_id.as_deref().or_else(|| {
            snapshot
                .profiles
                .iter()
                .find(|profile| profile.active)
                .map(|profile| profile.id.as_str())
        });
        let active_adapter = snapshot.status.active_adapter_id.as_deref();
        let (profile_label, profile_detail, profile_accent) =
            active_profile_summary(active_profile_id, &snapshot.profiles);
        let (controller_label, controller_detail, controller_accent) = active_controller_summary(
            snapshot.profile_resolution.controller_id.as_deref(),
            &snapshot.controllers,
        );
        let agent_detail = match (active_profile_id, active_adapter) {
            (_, Some(adapter)) => format!("v{version} - telemetry via {adapter}"),
            (Some(_), None) => format!("v{version} - profile ready"),
            _ => format!("v{version} - local runtime ready"),
        };

        let statuses = snapshot
            .diagnostics
            .checks
            .iter()
            .map(|check| check.status.as_str())
            .collect::<Vec<_>>();
        let (diagnostics_label, diagnostics_detail, diagnostics_accent) =
            diagnostics_summary_from_statuses(&statuses);

        TrayHealthSummary {
            agent_running: true,
            agent_label: "Agent Online".to_string(),
            agent_detail,
            agent_accent: TrayMenuAccent::Ready,
            profile_label,
            profile_detail,
            profile_accent,
            controller_label,
            controller_detail,
            controller_accent,
            diagnostics_label,
            diagnostics_detail,
            diagnostics_accent,
        }
    }

    fn diagnostics_summary_from_statuses(statuses: &[&str]) -> (String, String, TrayMenuAccent) {
        if statuses.is_empty() {
            return (
                "Diagnostics Warming Up".to_string(),
                "No checks reported yet".to_string(),
                TrayMenuAccent::Neutral,
            );
        }

        let pending = statuses
            .iter()
            .filter(|status| **status == "pending")
            .count();
        let attention = statuses
            .iter()
            .filter(|status| {
                !matches!(
                    **status,
                    "ok" | "hidapi" | "pending" | "ready" | "connected"
                )
            })
            .count();

        if attention > 0 {
            (
                "Diagnostics Need Attention".to_string(),
                format!("{attention} of {} checks need review", statuses.len()),
                TrayMenuAccent::Danger,
            )
        } else if pending > 0 {
            (
                "Diagnostics Warming Up".to_string(),
                format!(
                    "{pending} check warming up, {} checks healthy",
                    statuses.len() - pending
                ),
                TrayMenuAccent::Neutral,
            )
        } else {
            (
                "Diagnostics Clear".to_string(),
                format!("{} checks healthy", statuses.len()),
                TrayMenuAccent::Ready,
            )
        }
    }

    fn active_profile_summary(
        active_profile_id: Option<&str>,
        profiles: &[TraySnapshotProfileDto],
    ) -> (String, String, TrayMenuAccent) {
        let Some(profile_id) = active_profile_id else {
            return (
                "Profile: None".to_string(),
                "No active profile selected".to_string(),
                TrayMenuAccent::Neutral,
            );
        };
        let profile_name = profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| fallback_profile_name(profile_id));
        (
            format!("Profile: {profile_name}"),
            profile_id.to_string(),
            TrayMenuAccent::Ready,
        )
    }

    fn active_controller_summary(
        active_controller_id: Option<&str>,
        controllers: &[TraySnapshotControllerDto],
    ) -> (String, String, TrayMenuAccent) {
        let controller = active_controller_id
            .and_then(|id| controllers.iter().find(|controller| controller.id == id))
            .or_else(|| controllers.iter().find(|controller| controller.connected))
            .or_else(|| controllers.first());

        let Some(controller) = controller else {
            return (
                "Controller: None".to_string(),
                "No controller detected".to_string(),
                TrayMenuAccent::Neutral,
            );
        };

        let label = if controller.name.trim().is_empty() {
            fallback_controller_name(&controller.model)
        } else {
            controller.name.trim().to_string()
        };
        let detail = [
            fallback_controller_name(&controller.model),
            transport_label(&controller.transport),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");

        (
            format!("Controller: {label}"),
            if detail.is_empty() {
                controller.id.clone()
            } else {
                detail
            },
            if controller.connected {
                TrayMenuAccent::Ready
            } else {
                TrayMenuAccent::Neutral
            },
        )
    }

    fn fallback_controller_name(model: &str) -> String {
        let normalized = model.trim();
        if normalized.is_empty() {
            "DualSense".to_string()
        } else if normalized.eq_ignore_ascii_case("dualsense_edge") {
            "DualSense Edge".to_string()
        } else if normalized.eq_ignore_ascii_case("dualsense") {
            "DualSense".to_string()
        } else {
            normalized.replace('_', " ")
        }
    }

    fn transport_label(transport: &str) -> String {
        match transport.trim().to_ascii_lowercase().as_str() {
            "usb" => "USB".to_string(),
            "bluetooth" => "Bluetooth".to_string(),
            "" | "unknown" => String::new(),
            value => value.to_string(),
        }
    }

    fn fallback_profile_name(profile_id: &str) -> String {
        match profile_id {
            "global" => "Global".to_string(),
            "forza-horizon" => "Base".to_string(),
            "forza-horizon-immersive" => "Immersive".to_string(),
            "assetto-corsa-rally" => "Rally".to_string(),
            _ => profile_id.to_string(),
        }
    }

    fn handle_command(hwnd: HWND, command: usize) {
        match command {
            CMD_OPEN_UI => open_ui(hwnd),
            CMD_OPEN_HAPTICS => open_ui_url(hwnd, HAPTICS_URL),
            CMD_OPEN_BUTTON_MAPPING => open_ui_url(hwnd, BUTTON_MAPPING_URL),
            CMD_CHECK_UPDATES => open_url(hwnd, RELEASES_URL),
            CMD_START => {
                if let Err(error) = with_state(|state| state.ensure_agent()) {
                    show_error(hwnd, &error.to_string());
                }
            }
            CMD_STOP => with_state(|state| {
                state.stop_agent();
                Ok(())
            })
            .unwrap_or_else(|error| show_error(hwnd, &error.to_string())),
            CMD_RESTART => {
                if let Err(error) = with_state(|state| state.restart_agent()) {
                    show_error(hwnd, &error.to_string());
                }
            }
            CMD_QUIT => {
                let _ = with_state(|state| {
                    state.stop_agent();
                    Ok(())
                });
                remove_tray_icon(hwnd);
                unsafe {
                    PostQuitMessage(0);
                }
            }
            _ => {}
        }
    }

    fn open_ui(hwnd: HWND) {
        open_ui_url(hwnd, DASHBOARD_URL);
    }

    fn open_ui_url(hwnd: HWND, url: &str) {
        match with_state(|state| {
            state.ensure_agent()?;
            Ok(state.claim_open_ui(url))
        }) {
            Ok(true) => open_url(hwnd, url),
            Ok(false) => {}
            Err(error) => show_error(hwnd, &error.to_string()),
        }
    }

    fn open_browser(hwnd: HWND) {
        open_ui_url(hwnd, DASHBOARD_URL);
    }

    fn open_url(hwnd: HWND, url: &str) {
        unsafe {
            let operation = wide_null("open");
            let url = wide_null(url);
            let result = ShellExecuteW(
                hwnd,
                operation.as_ptr(),
                url.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            );
            if (result as isize) <= 32 {
                show_error(hwnd, "Windows could not open the requested DSCC URL.");
            }
        }
    }

    fn with_state<T>(action: impl FnOnce(&mut TrayState) -> Result<T>) -> Result<T> {
        let state = STATE
            .get()
            .ok_or_else(|| anyhow!("tray state is not initialized"))?;
        let mut guard = state
            .lock()
            .map_err(|_| anyhow!("tray state lock was poisoned"))?;
        action(&mut guard)
    }

    fn message_loop() {
        unsafe {
            let mut message = MSG::default();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&message);
            }
        }
    }

    fn agent_is_healthy() -> bool {
        http_get_body(STATUS_PATH, Duration::from_millis(450)).is_some_and(|body| {
            body.contains("DualSense Command Center Agent")
                && (body.contains("\"healthy\":true") || body.contains("\"healthy\": true"))
        })
    }

    fn http_get_body(path: &str, timeout: Duration) -> Option<String> {
        let Ok(addr) = API_HOST.parse::<SocketAddr>() else {
            return None;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout.min(Duration::from_secs(2)))
        else {
            return None;
        };
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            return None;
        }
        let mut response = String::new();
        if stream.read_to_string(&mut response).is_err() || !response.contains("200 OK") {
            return None;
        }
        response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .or(Some(response))
    }

    fn agent_spawn_addr() -> String {
        if persisted_listen_on_all_interfaces() {
            "0.0.0.0:43473".to_string()
        } else {
            API_HOST.to_string()
        }
    }

    fn persisted_listen_on_all_interfaces() -> bool {
        let Some(state_file) = config_dir().map(|dir| dir.join("state.json")) else {
            return false;
        };
        let Ok(contents) = std::fs::read_to_string(state_file) else {
            return false;
        };
        let compact = contents
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        compact.contains("\"listenOnAllInterfaces\":true")
    }

    fn config_dir() -> Option<PathBuf> {
        if let Some(config_dir) = env::var_os("DSCC_CONFIG_DIR") {
            return Some(PathBuf::from(config_dir));
        }
        env::var_os("APPDATA").map(PathBuf::from).map(|appdata| {
            appdata
                .join("DualSenseCommand")
                .join("DualSenseCommandCenter")
                .join("config")
        })
    }

    fn wait_for_agent(timeout: Duration) {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if agent_is_healthy() {
                return;
            }
            thread::sleep(Duration::from_millis(150));
        }
    }

    fn show_error(hwnd: HWND, message: &str) {
        unsafe {
            let title = wide_null("DualSense Command Center");
            let message = wide_null(message);
            MessageBoxW(hwnd, message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    fn wide_text(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().collect()
    }

    fn copy_wide_fixed(target: &mut [u16], value: &str) {
        let encoded = wide_null(value);
        let len = encoded.len().min(target.len());
        target[..len].copy_from_slice(&encoded[..len]);
        if len == target.len() {
            target[target.len() - 1] = 0;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn decodes_classic_tray_messages() {
            assert_eq!(
                tray_icon_action(TRAY_ICON_ID as WPARAM, WM_RBUTTONUP as LPARAM),
                Some(TrayIconAction::ShowMenu)
            );
            assert_eq!(
                tray_icon_action(TRAY_ICON_ID as WPARAM, WM_LBUTTONUP as LPARAM),
                Some(TrayIconAction::OpenUi)
            );
        }

        #[test]
        fn decodes_notifyicon_version_4_tray_messages() {
            let context_menu = ((TRAY_ICON_ID as usize) << 16) | WM_CONTEXTMENU as usize;
            let keyboard_select = ((TRAY_ICON_ID as usize) << 16) | NIN_KEYSELECT as usize;

            assert_eq!(
                tray_icon_action(0, context_menu as LPARAM),
                Some(TrayIconAction::ShowMenu)
            );
            assert_eq!(
                tray_icon_action(0, keyboard_select as LPARAM),
                Some(TrayIconAction::OpenUi)
            );
        }

        #[test]
        fn ignores_messages_for_other_icons() {
            let other_icon = (((TRAY_ICON_ID + 1) as usize) << 16) | WM_CONTEXTMENU as usize;

            assert_eq!(tray_icon_action(0, other_icon as LPARAM), None);
            assert_eq!(tray_icon_action(999, WM_RBUTTONUP as LPARAM), None);
        }

        #[test]
        fn tray_state_debounces_duplicate_open_ui_requests() {
            let (health_refresh_tx, _health_refresh_rx) = mpsc::sync_channel(1);
            let mut state = TrayState {
                agent: None,
                install_dir: PathBuf::new(),
                last_open_ui: None,
                health_cache: Arc::new(Mutex::new(TrayHealthCache {
                    summary: refreshing_health_summary(),
                    refreshed_at: Instant::now(),
                })),
                health_refresh_tx,
            };

            assert!(state.claim_open_ui(DASHBOARD_URL));
            assert!(!state.claim_open_ui(DASHBOARD_URL));
            assert!(state.claim_open_ui(HAPTICS_URL));
            assert!(state.claim_open_ui(BUTTON_MAPPING_URL));

            state.last_open_ui = Some((
                Instant::now() - Duration::from_millis(OPEN_UI_DEBOUNCE_MS + 1),
                BUTTON_MAPPING_URL.to_string(),
            ));
            assert!(state.claim_open_ui(BUTTON_MAPPING_URL));
        }

        #[test]
        fn tray_menu_exposes_useful_actions_and_agent_state() {
            assert!(DASHBOARD_URL.ends_with("#/games"));
            assert!(HAPTICS_URL.ends_with("#/adaptive-triggers-haptics"));
            assert!(BUTTON_MAPPING_URL.ends_with("#/button-mapping"));

            let running_summary = TrayHealthSummary {
                agent_running: true,
                agent_label: "Agent Online".to_string(),
                agent_detail: "v0.1.9 - local runtime ready".to_string(),
                agent_accent: TrayMenuAccent::Ready,
                profile_label: "Profile: Base".to_string(),
                profile_detail: "forza-horizon".to_string(),
                profile_accent: TrayMenuAccent::Ready,
                controller_label: "Controller: Edge".to_string(),
                controller_detail: "DualSense Edge / Bluetooth".to_string(),
                controller_accent: TrayMenuAccent::Ready,
                diagnostics_label: "Diagnostics Clear".to_string(),
                diagnostics_detail: "7 checks healthy".to_string(),
                diagnostics_accent: TrayMenuAccent::Ready,
            };
            let running = tray_menu_entries(&running_summary, true);
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_OPEN_BUTTON_MAPPING && !entry.disabled));
            assert!(running
                .iter()
                .any(|entry| entry.descriptor.label == "Agent Online"
                    && entry.descriptor.kind == TrayMenuKind::Readout));
            assert!(running
                .iter()
                .any(|entry| entry.descriptor.label == "Profile: Base"
                    && entry.descriptor.detail == "forza-horizon"));
            assert!(running.iter().any(|entry| {
                entry.descriptor.label == "Controller: Edge"
                    && entry.descriptor.detail == "DualSense Edge / Bluetooth"
            }));
            assert!(running
                .iter()
                .any(|entry| entry.descriptor.label == "Diagnostics Clear"
                    && entry.descriptor.kind == TrayMenuKind::Readout));
            assert!(running.iter().any(|entry| {
                entry.command == CMD_OPEN_UI && entry.descriptor.label == "Dashboard"
            }));
            assert!(running.iter().any(|entry| {
                entry.command == CMD_OPEN_HAPTICS && entry.descriptor.label == "Triggers & Haptics"
            }));
            assert!(running
                .iter()
                .all(|entry| entry.descriptor.label != "Diagnostics Waiting"));
            assert!(running
                .iter()
                .all(|entry| !entry.descriptor.label.contains("JSON")));
            assert!(running.iter().all(|entry| {
                !matches!(
                    entry.descriptor.label.as_str(),
                    "Open Install Folder" | "Open Config Folder"
                )
            }));
            assert!(running.iter().all(|entry| entry.command != CMD_START));
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_STOP && !entry.disabled));
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_CHECK_UPDATES && !entry.disabled));
            assert!(tray_menu_height(&running) < 400);

            let offline_summary = TrayHealthSummary {
                agent_running: false,
                agent_label: "Agent Offline".to_string(),
                agent_detail: "Start the agent to enable controller control".to_string(),
                agent_accent: TrayMenuAccent::Danger,
                profile_label: "Profile Unavailable".to_string(),
                profile_detail: "Start the agent to read profile state".to_string(),
                profile_accent: TrayMenuAccent::Neutral,
                controller_label: "Controller Unavailable".to_string(),
                controller_detail: "Start the agent to read controller state".to_string(),
                controller_accent: TrayMenuAccent::Neutral,
                diagnostics_label: "Diagnostics Unavailable".to_string(),
                diagnostics_detail: "Waiting for the local runtime".to_string(),
                diagnostics_accent: TrayMenuAccent::Neutral,
            };
            let offline = tray_menu_entries(&offline_summary, false);
            assert!(offline
                .iter()
                .any(|entry| entry.command == CMD_START && !entry.disabled));
            assert!(offline.iter().all(|entry| entry.command != CMD_STOP));
            assert!(tray_menu_height(&offline) < 370);

            let external = tray_menu_entries(&running_summary, false);
            assert!(external.iter().all(|entry| entry.command != CMD_STOP));
            assert!(external.iter().all(|entry| entry.command != CMD_RESTART));
            assert!(external.iter().all(|entry| entry.command != CMD_START));
            assert!(external.iter().any(|entry| {
                entry.command == CMD_QUIT && entry.descriptor.detail == "Close tray"
            }));
        }

        #[test]
        fn tray_snapshot_summary_reads_active_profile_and_diagnostics() {
            let snapshot = serde_json::from_str::<TraySnapshotDto>(
                r#"{
                    "status":{
                        "version":"0.2.6",
                        "healthy":true,
                        "active_profile_id":"forza-horizon",
                        "active_adapter_id":null
                    },
                    "profiles":[
                        {"id":"forza-horizon","name":"Base","built_in":true,"active":true},
                        {"id":"forza-horizon-immersive","name":"Immersive","built_in":true,"active":false}
                    ],
                    "controllers":[
                        {"id":"controller-0001","name":"Edge","model":"dualsense_edge","transport":"bluetooth","connected":true}
                    ],
                    "profileResolution":{"controllerId":"controller-0001"},
                    "diagnostics":{
                        "loopback_only":true,
                        "hardware_required":false,
                        "checks":[
                            {"name":"agent","status":"ok","detail":"ready"},
                            {"name":"hid","status":"connected","detail":"ready"}
                        ]
                    }
                }"#,
            )
            .expect("snapshot subset parses");
            let summary = tray_health_summary_from_snapshot(&snapshot);

            assert_eq!(summary.agent_label, "Agent Online");
            assert_eq!(summary.agent_detail, "v0.2.6 - profile ready");
            assert_eq!(summary.profile_label, "Profile: Base");
            assert_eq!(summary.profile_detail, "forza-horizon");
            assert_eq!(summary.controller_label, "Controller: Edge");
            assert_eq!(summary.controller_detail, "DualSense Edge / Bluetooth");
            assert_eq!(summary.diagnostics_label, "Diagnostics Clear");
            assert_eq!(
                fallback_profile_name("forza-horizon-immersive"),
                "Immersive"
            );
        }

        #[test]
        fn bundled_tray_icon_contains_usable_images() {
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 16).is_some());
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 32).is_some());
        }
    }
}
