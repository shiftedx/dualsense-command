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
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::CREATE_NO_WINDOW},
        UI::{
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
                CS_VREDRAW, CW_USEDEFAULT, HICON, IDI_APPLICATION, MB_ICONERROR, MB_OK, MF_GRAYED,
                MF_SEPARATOR, MF_STRING, MSG, SW_SHOWNORMAL, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND,
                WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
                WNDCLASSW,
            },
        },
    };

    const TRAY_ICON_ICO: &[u8] = include_bytes!("../assets/dscc-tray.ico");
    const UI_URL: &str = "http://127.0.0.1:43473/";
    const API_HOST: &str = "127.0.0.1:43473";
    const TRAY_ICON_ID: u32 = 1;
    const WM_TRAYICON: u32 = WM_APP + 1;
    const CMD_OPEN_UI: usize = 1001;
    const CMD_START: usize = 1002;
    const CMD_STOP: usize = 1003;
    const CMD_RESTART: usize = 1004;
    const CMD_QUIT: usize = 1005;
    const NIN_KEYSELECT: u32 = NIN_SELECT + 1;

    static STATE: OnceLock<Mutex<TrayState>> = OnceLock::new();
    static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

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

            let running = agent_is_healthy();
            append_menu(menu, CMD_OPEN_UI, "Open UI", false);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append_menu(menu, CMD_START, "Start DSCC", running);
            append_menu(menu, CMD_STOP, "Stop DSCC", !running);
            append_menu(menu, CMD_RESTART, "Restart DSCC", false);
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append_menu(menu, CMD_QUIT, "Quit DSCC", false);

            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 {
                SetForegroundWindow(hwnd);
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, null());
                PostMessageW(hwnd, WM_NULL, 0, 0);
            }
            DestroyMenu(menu);
        }
    }

    fn append_menu(menu: *mut std::ffi::c_void, id: usize, label: &str, disabled: bool) {
        let label = wide_null(label);
        let mut flags = MF_STRING;
        if disabled {
            flags |= MF_GRAYED;
        }
        unsafe {
            AppendMenuW(menu, flags, id, label.as_ptr());
        }
    }

    fn handle_command(hwnd: HWND, command: usize) {
        match command {
            CMD_OPEN_UI => open_ui(hwnd),
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
        if let Err(error) = with_state(|state| state.ensure_agent()) {
            show_error(hwnd, &error.to_string());
            return;
        }

        open_browser(hwnd);
    }

    fn open_browser(hwnd: HWND) {
        unsafe {
            let operation = wide_null("open");
            let url = wide_null(UI_URL);
            ShellExecuteW(
                hwnd,
                operation.as_ptr(),
                url.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            );
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
        let Ok(addr) = API_HOST.parse::<SocketAddr>() else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(450)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(450)));
        if stream
            .write_all(b"GET /api/status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .is_err()
        {
            return false;
        }
        let mut response = String::new();
        stream.read_to_string(&mut response).is_ok()
            && response.contains("200 OK")
            && response.contains("DualSense Command Center Agent")
    }

    fn agent_spawn_addr() -> String {
        if persisted_listen_on_all_interfaces() {
            "0.0.0.0:43473".to_string()
        } else {
            API_HOST.to_string()
        }
    }

    fn persisted_listen_on_all_interfaces() -> bool {
        let Some(appdata) = env::var_os("APPDATA") else {
            return false;
        };
        let state_file = PathBuf::from(appdata)
            .join("DualSenseCommand")
            .join("DualSenseCommandCenter")
            .join("config")
            .join("state.json");
        let Ok(contents) = std::fs::read_to_string(state_file) else {
            return false;
        };
        let compact = contents
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        compact.contains("\"listenOnAllInterfaces\":true")
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
        fn decodes_legacy_tray_messages() {
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
        fn bundled_tray_icon_contains_usable_images() {
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 16).is_some());
            assert!(icon_image_from_ico(TRAY_ICON_ICO, 32).is_some());
        }
    }
}
