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
            Mutex, OnceLock,
        },
        thread,
        time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, FillRect,
            SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
            DEFAULT_CHARSET, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
            FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, OUT_DEFAULT_PRECIS, TRANSPARENT,
        },
        System::{LibraryLoader::GetModuleHandleW, Threading::CREATE_NO_WINDOW},
        UI::{
            Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_GRAYED, ODS_SELECTED, ODT_MENU},
            Shell::{
                ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP,
                NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW,
                NOTIFYICON_VERSION_4,
            },
            WindowsAndMessaging::{
                AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, DispatchMessageW, FindWindowW, GetCursorPos,
                GetMessageW, LoadIconW, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW,
                RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu, CS_HREDRAW,
                CS_VREDRAW, CW_USEDEFAULT, HICON, HMENU, IDI_APPLICATION, MB_ICONERROR, MB_OK,
                MF_DISABLED, MF_GRAYED, MF_OWNERDRAW, MSG, SW_SHOWNORMAL, TPM_RIGHTBUTTON, WM_APP,
                WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_DRAWITEM, WM_LBUTTONDBLCLK,
                WM_LBUTTONUP, WM_MEASUREITEM, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
            },
        },
    };

    const TRAY_ICON_ICO: &[u8] = include_bytes!("../assets/dscc-tray.ico");
    const DASHBOARD_URL: &str = "http://127.0.0.1:43473/#/adaptive-triggers-haptics";
    const BUTTON_MAPPING_URL: &str = "http://127.0.0.1:43473/#/button-mapping";
    const STATUS_PATH: &str = "/api/status";
    const DIAGNOSTICS_PATH: &str = "/api/diagnostics";
    const API_HOST: &str = "127.0.0.1:43473";
    const TRAY_ICON_ID: u32 = 1;
    const WM_TRAYICON: u32 = WM_APP + 1;
    const CMD_OPEN_UI: usize = 1001;
    const CMD_START: usize = 1002;
    const CMD_STOP: usize = 1003;
    const CMD_RESTART: usize = 1004;
    const CMD_QUIT: usize = 1005;
    const CMD_OPEN_BUTTON_MAPPING: usize = 1006;
    const NIN_KEYSELECT: u32 = NIN_SELECT + 1;
    const MENU_WIDTH: u32 = 286;
    const MENU_HEADER_HEIGHT: u32 = 58;
    const MENU_READOUT_HEIGHT: u32 = 44;
    const MENU_ITEM_HEIGHT: u32 = 36;
    const MENU_SEPARATOR_HEIGHT: u32 = 10;
    const COLOR_OBSIDIAN: COLORREF = rgb(10, 10, 12);
    const COLOR_CARBON: COLORREF = rgb(18, 18, 20);
    const COLOR_SELECTED: COLORREF = rgb(11, 34, 54);
    const COLOR_ACTUATION: COLORREF = rgb(0, 112, 204);
    const COLOR_HAPTIC: COLORREF = rgb(226, 232, 240);
    const COLOR_TUNGSTEN: COLORREF = rgb(113, 113, 122);
    const COLOR_OVERDRIVE: COLORREF = rgb(240, 62, 62);
    const COLOR_READY: COLORREF = rgb(34, 197, 94);
    const COLOR_LINE: COLORREF = rgb(54, 57, 66);
    const COLOR_DISABLED: COLORREF = rgb(86, 86, 96);
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TrayHealthSummary {
        agent_running: bool,
        agent_label: String,
        agent_detail: String,
        agent_accent: TrayMenuAccent,
        diagnostics_label: String,
        diagnostics_detail: String,
        diagnostics_accent: TrayMenuAccent,
    }

    fn tray_menu_entries(summary: &TrayHealthSummary) -> Vec<TrayMenuEntry> {
        vec![
            TrayMenuEntry {
                command: 0,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Header,
                    "DualSense Command Center",
                    "",
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
                    "Open Dashboard",
                    "Adaptive triggers and haptics",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: CMD_OPEN_BUTTON_MAPPING,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Button Mapping",
                    "Steam Input helper view",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: separator_descriptor(),
                disabled: true,
            },
            TrayMenuEntry {
                command: CMD_START,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Start Agent",
                    "Launch local runtime",
                    TrayMenuAccent::Ready,
                ),
                disabled: summary.agent_running,
            },
            TrayMenuEntry {
                command: CMD_STOP,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Stop Agent",
                    "Stops DSCC runtime",
                    TrayMenuAccent::Danger,
                ),
                disabled: !summary.agent_running,
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
            TrayMenuEntry {
                command: CMD_QUIT,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Quit DSCC",
                    "Stop agent and tray",
                    TrayMenuAccent::Danger,
                ),
                disabled: false,
            },
        ]
    }

    fn separator_descriptor() -> TrayMenuDescriptor {
        TrayMenuDescriptor::new(TrayMenuKind::Separator, "", "", TrayMenuAccent::Neutral)
    }

    struct TrayState {
        agent: Option<Child>,
        install_dir: PathBuf,
    }

    impl TrayState {
        fn new() -> Result<Self> {
            let exe = env::current_exe().context("could not resolve tray executable path")?;
            let install_dir = exe
                .parent()
                .ok_or_else(|| anyhow!("tray executable has no parent directory"))?
                .to_path_buf();
            Ok(Self {
                agent: None,
                install_dir,
            })
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

        fn ensure_agent(&mut self) -> Result<()> {
            self.prune_exited_child();
            if agent_is_healthy() {
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
            Ok(())
        }

        fn stop_agent(&mut self) {
            if let Some(mut child) = self.agent.take() {
                let _ = child.kill();
                let _ = child.wait();
            }

            let taskkill = env::var_os("SystemRoot")
                .map(PathBuf::from)
                .map(|root| root.join("System32").join("taskkill.exe"))
                .filter(|path| path.exists())
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\taskkill.exe"));

            let _ = Command::new(taskkill)
                .args(["/IM", "dscc-agent.exe", "/F", "/T"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
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
            WM_MEASUREITEM => measure_menu_item(lparam),
            WM_DRAWITEM => draw_menu_item(lparam),
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
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }

            let summary = tray_health_summary();
            let entries = tray_menu_entries(&summary);
            for entry in &entries {
                append_menu_item(menu, entry);
            }

            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 {
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, null());
                PostMessageW(hwnd, WM_NULL, 0, 0);
            }
            DestroyMenu(menu);
        }
    }

    fn append_menu_item(menu: HMENU, entry: &TrayMenuEntry) {
        let mut flags = MF_OWNERDRAW;
        if entry.disabled {
            flags |= MF_GRAYED | MF_DISABLED;
        }
        let item_data = &entry.descriptor as *const TrayMenuDescriptor as *const u16;
        unsafe {
            AppendMenuW(menu, flags, entry.command, item_data);
        }
    }

    fn measure_menu_item(lparam: LPARAM) -> LRESULT {
        if lparam == 0 {
            return 0;
        }
        unsafe {
            let measure = &mut *(lparam as *mut MEASUREITEMSTRUCT);
            if measure.CtlType != ODT_MENU {
                return 0;
            }
            let Some(descriptor) = descriptor_from_item_data(measure.itemData) else {
                return 0;
            };
            measure.itemWidth = MENU_WIDTH;
            measure.itemHeight = match descriptor.kind {
                TrayMenuKind::Header => MENU_HEADER_HEIGHT,
                TrayMenuKind::Readout => MENU_READOUT_HEIGHT,
                TrayMenuKind::Action => MENU_ITEM_HEIGHT,
                TrayMenuKind::Separator => MENU_SEPARATOR_HEIGHT,
            };
            1
        }
    }

    fn draw_menu_item(lparam: LPARAM) -> LRESULT {
        if lparam == 0 {
            return 0;
        }
        unsafe {
            let draw = &*(lparam as *const DRAWITEMSTRUCT);
            if draw.CtlType != ODT_MENU {
                return 0;
            }
            let Some(descriptor) = descriptor_from_item_data(draw.itemData) else {
                return 0;
            };
            match descriptor.kind {
                TrayMenuKind::Header => draw_menu_header(draw, descriptor),
                TrayMenuKind::Readout => draw_menu_readout(draw, descriptor),
                TrayMenuKind::Action => draw_menu_action(draw, descriptor),
                TrayMenuKind::Separator => draw_menu_separator(draw),
            }
            1
        }
    }

    unsafe fn descriptor_from_item_data(item_data: usize) -> Option<&'static TrayMenuDescriptor> {
        if item_data == 0 {
            None
        } else {
            Some(&*(item_data as *const TrayMenuDescriptor))
        }
    }

    unsafe fn draw_menu_header(draw: &DRAWITEMSTRUCT, descriptor: &TrayMenuDescriptor) {
        let rect = draw.rcItem;
        fill_rect(draw.hDC, rect, COLOR_OBSIDIAN);
        fill_rect(
            draw.hDC,
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.left + 4,
                bottom: rect.bottom,
            },
            COLOR_ACTUATION,
        );

        let title_rect = RECT {
            left: rect.left + 18,
            top: rect.top + 8,
            right: rect.right - 14,
            bottom: rect.top + 30,
        };
        draw_text_line(
            draw.hDC,
            &descriptor.label,
            title_rect,
            COLOR_WHITE,
            16,
            FW_SEMIBOLD,
        );

        draw_dot(draw.hDC, rect.left + 20, rect.top + 38, 8, COLOR_ACTUATION);
        let status_rect = RECT {
            left: rect.left + 34,
            top: rect.top + 31,
            right: rect.right - 14,
            bottom: rect.bottom - 6,
        };
        draw_text_line(
            draw.hDC,
            "Tray controls and live health",
            status_rect,
            COLOR_TUNGSTEN,
            12,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_readout(draw: &DRAWITEMSTRUCT, descriptor: &TrayMenuDescriptor) {
        let rect = draw.rcItem;
        fill_rect(draw.hDC, rect, COLOR_CARBON);
        draw_dot(
            draw.hDC,
            rect.left + 18,
            rect.top + 15,
            10,
            menu_accent_color(descriptor.accent),
        );

        let label_rect = RECT {
            left: rect.left + 38,
            top: rect.top + 5,
            right: rect.right - 14,
            bottom: rect.top + 23,
        };
        draw_text_line(
            draw.hDC,
            &descriptor.label,
            label_rect,
            COLOR_HAPTIC,
            13,
            FW_SEMIBOLD,
        );

        let detail_rect = RECT {
            left: rect.left + 38,
            top: rect.top + 22,
            right: rect.right - 14,
            bottom: rect.bottom - 4,
        };
        draw_text_line(
            draw.hDC,
            &descriptor.detail,
            detail_rect,
            COLOR_TUNGSTEN,
            11,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_action(draw: &DRAWITEMSTRUCT, descriptor: &TrayMenuDescriptor) {
        let rect = draw.rcItem;
        let selected = draw.itemState & ODS_SELECTED != 0;
        let disabled = draw.itemState & ODS_GRAYED != 0;
        let background = if selected && !disabled {
            COLOR_SELECTED
        } else {
            COLOR_CARBON
        };
        fill_rect(draw.hDC, rect, background);

        let accent = if disabled {
            COLOR_LINE
        } else {
            menu_accent_color(descriptor.accent)
        };
        fill_rect(
            draw.hDC,
            RECT {
                left: rect.left + 8,
                top: rect.top + 9,
                right: rect.left + 12,
                bottom: rect.bottom - 9,
            },
            accent,
        );

        let label_color = if disabled {
            COLOR_DISABLED
        } else {
            COLOR_HAPTIC
        };
        let detail_color = if disabled {
            COLOR_DISABLED
        } else {
            COLOR_TUNGSTEN
        };
        let label_rect = RECT {
            left: rect.left + 22,
            top: rect.top + 3,
            right: rect.right - 14,
            bottom: rect.top + 20,
        };
        draw_text_line(
            draw.hDC,
            &descriptor.label,
            label_rect,
            label_color,
            13,
            FW_SEMIBOLD,
        );

        let detail_rect = RECT {
            left: rect.left + 22,
            top: rect.top + 18,
            right: rect.right - 14,
            bottom: rect.bottom - 3,
        };
        draw_text_line(
            draw.hDC,
            &descriptor.detail,
            detail_rect,
            detail_color,
            11,
            FW_NORMAL,
        );
    }

    unsafe fn draw_menu_separator(draw: &DRAWITEMSTRUCT) {
        let rect = draw.rcItem;
        fill_rect(draw.hDC, rect, COLOR_CARBON);
        let top = rect.top + ((rect.bottom - rect.top) / 2);
        fill_rect(
            draw.hDC,
            RECT {
                left: rect.left + 16,
                top,
                right: rect.right - 16,
                bottom: top + 1,
            },
            COLOR_LINE,
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

    fn tray_health_summary() -> TrayHealthSummary {
        let Some(status_body) = http_get_body(STATUS_PATH, Duration::from_millis(450)) else {
            return TrayHealthSummary {
                agent_running: false,
                agent_label: "Agent Offline".to_string(),
                agent_detail: "Start the agent to enable controller control".to_string(),
                agent_accent: TrayMenuAccent::Danger,
                diagnostics_label: "Diagnostics Unavailable".to_string(),
                diagnostics_detail: "Waiting for the local runtime".to_string(),
                diagnostics_accent: TrayMenuAccent::Neutral,
            };
        };

        let version =
            json_string_value(&status_body, "version").unwrap_or_else(|| "0.1.9".to_string());
        let active_profile = json_string_value(&status_body, "active_profile_id");
        let active_adapter = json_string_value(&status_body, "active_adapter_id");
        let agent_detail = match (active_profile.as_deref(), active_adapter.as_deref()) {
            (_, Some(adapter)) => format!("v{version} - telemetry via {adapter}"),
            (Some(_), None) => format!("v{version} - profile ready"),
            _ => format!("v{version} - local runtime ready"),
        };

        let (diagnostics_label, diagnostics_detail, diagnostics_accent) = diagnostics_summary()
            .unwrap_or_else(|| {
                (
                    "Diagnostics Warming Up".to_string(),
                    "Health checks are warming up".to_string(),
                    TrayMenuAccent::Neutral,
                )
            });

        TrayHealthSummary {
            agent_running: true,
            agent_label: "Agent Online".to_string(),
            agent_detail,
            agent_accent: TrayMenuAccent::Ready,
            diagnostics_label,
            diagnostics_detail,
            diagnostics_accent,
        }
    }

    fn diagnostics_summary() -> Option<(String, String, TrayMenuAccent)> {
        let body = http_get_body(DIAGNOSTICS_PATH, Duration::from_millis(700))?;
        let statuses = json_string_values(&body, "status");
        if statuses.is_empty() {
            return Some((
                "Diagnostics Warming Up".to_string(),
                "No checks reported yet".to_string(),
                TrayMenuAccent::Neutral,
            ));
        }

        let pending = statuses
            .iter()
            .filter(|status| status.as_str() == "pending")
            .count();
        let attention = statuses
            .iter()
            .filter(|status| {
                !matches!(
                    status.as_str(),
                    "ok" | "hidapi" | "pending" | "ready" | "connected"
                )
            })
            .count();

        if attention > 0 {
            Some((
                "Diagnostics Need Attention".to_string(),
                format!("{attention} of {} checks need review", statuses.len()),
                TrayMenuAccent::Danger,
            ))
        } else if pending > 0 {
            Some((
                "Diagnostics Warming Up".to_string(),
                format!(
                    "{pending} check warming up, {} checks healthy",
                    statuses.len() - pending
                ),
                TrayMenuAccent::Neutral,
            ))
        } else {
            Some((
                "Diagnostics Clear".to_string(),
                format!("{} checks healthy", statuses.len()),
                TrayMenuAccent::Ready,
            ))
        }
    }

    fn handle_command(hwnd: HWND, command: usize) {
        match command {
            CMD_OPEN_UI => open_ui(hwnd),
            CMD_OPEN_BUTTON_MAPPING => open_ui_url(hwnd, BUTTON_MAPPING_URL),
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
        if let Err(error) = with_state(|state| state.ensure_agent()) {
            show_error(hwnd, &error.to_string());
            return;
        }

        open_url(hwnd, url);
    }

    fn open_browser(hwnd: HWND) {
        open_url(hwnd, DASHBOARD_URL);
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

    fn json_string_value(body: &str, key: &str) -> Option<String> {
        json_string_values(body, key).into_iter().next()
    }

    fn json_string_values(body: &str, key: &str) -> Vec<String> {
        let needle = format!("\"{key}\":\"");
        let mut values = Vec::new();
        let mut rest = body;
        while let Some(offset) = rest.find(&needle) {
            let value_start = offset + needle.len();
            let value_rest = &rest[value_start..];
            let Some(value_end) = value_rest.find('"') else {
                break;
            };
            values.push(value_rest[..value_end].to_string());
            rest = &value_rest[value_end + 1..];
        }
        values
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
        fn tray_menu_exposes_useful_actions_and_agent_state() {
            let running_summary = TrayHealthSummary {
                agent_running: true,
                agent_label: "Agent Online".to_string(),
                agent_detail: "v0.1.9 - local runtime ready".to_string(),
                agent_accent: TrayMenuAccent::Ready,
                diagnostics_label: "Diagnostics Clear".to_string(),
                diagnostics_detail: "7 checks healthy".to_string(),
                diagnostics_accent: TrayMenuAccent::Ready,
            };
            let running = tray_menu_entries(&running_summary);
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_OPEN_BUTTON_MAPPING && !entry.disabled));
            assert!(running
                .iter()
                .any(|entry| entry.descriptor.label == "Agent Online"
                    && entry.descriptor.kind == TrayMenuKind::Readout));
            assert!(running
                .iter()
                .any(|entry| entry.descriptor.label == "Diagnostics Clear"
                    && entry.descriptor.kind == TrayMenuKind::Readout));
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
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_START && entry.disabled));
            assert!(running
                .iter()
                .any(|entry| entry.command == CMD_STOP && !entry.disabled));

            let offline_summary = TrayHealthSummary {
                agent_running: false,
                agent_label: "Agent Offline".to_string(),
                agent_detail: "Start the agent to enable controller control".to_string(),
                agent_accent: TrayMenuAccent::Danger,
                diagnostics_label: "Diagnostics Unavailable".to_string(),
                diagnostics_detail: "Waiting for the local runtime".to_string(),
                diagnostics_accent: TrayMenuAccent::Neutral,
            };
            let offline = tray_menu_entries(&offline_summary);
            assert!(offline
                .iter()
                .any(|entry| entry.command == CMD_START && !entry.disabled));
            assert!(offline
                .iter()
                .any(|entry| entry.command == CMD_STOP && entry.disabled));
        }

        #[test]
        fn bundled_tray_icon_contains_usable_images() {
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 16).is_some());
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 32).is_some());
        }
    }
}
