use anyhow::{anyhow, Context, Result};
use std::{
    env,
    ffi::OsStr,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::windows::{ffi::OsStrExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT},
    System::{LibraryLoader::GetModuleHandleW, Threading::CREATE_NO_WINDOW},
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT},
        Shell::{
            ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
            NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
        },
        WindowsAndMessaging::{
            CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow,
            DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, GetSystemMetrics,
            GetWindowLongPtrW, LoadCursorW, LoadIconW, MessageBoxW, PostMessageW, PostQuitMessage,
            RegisterClassW, RegisterWindowMessageW, SetCursor, SetForegroundWindow,
            SetWindowLongPtrW, ShowWindow, CREATESTRUCTW, CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW,
            CW_USEDEFAULT, GWLP_USERDATA, HICON, IDC_ARROW, IDI_APPLICATION, MB_ICONERROR, MB_OK,
            MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SW_SHOWNORMAL, WA_INACTIVE, WM_ACTIVATE,
            WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION, WM_KILLFOCUS,
            WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_NULL,
            WM_PAINT, WM_QUERYENDSESSION, WM_RBUTTONUP, WM_SETCURSOR, WNDCLASSW, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

const TRAY_ICON_ICO: &[u8] = include_bytes!("../assets/dscc-tray.ico");
const DASHBOARD_URL: &str = "http://127.0.0.1:43473/#/tuning";
const HAPTICS_URL: &str = "http://127.0.0.1:43473/#/tuning";
const BUTTON_MAPPING_URL: &str = "http://127.0.0.1:43473/#/advanced/button-mapping";
const STATUS_PATH: &str = "/api/status";
const SNAPSHOT_PATH: &str = "/api/snapshot";
const API_HOST: &str = "127.0.0.1:43473";
const LAN_API_ENABLE_ENV: &str = "DSCC_ENABLE_LAN_API";
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

mod health;
mod menu;
mod painting;
use health::*;
use menu::*;
use painting::*;

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

struct TrayState {
    agent: Option<Child>,
    install_dir: PathBuf,
    last_open_ui: Option<(Instant, String)>,
    health_cache: Arc<Mutex<TrayHealthCache>>,
    health_refresh_tx: SyncSender<()>,
    session_ending: bool,
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
            session_ending: false,
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
        configure_agent_command(&mut command, &self.install_dir, self.web_dist());

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

    fn stop_agent_for_session_end(&mut self) {
        if let Some(mut child) = self.agent.take() {
            let _ = child.kill();
            let _ = child.try_wait();
        }
    }

    fn begin_session_end(&mut self) {
        if self.session_ending {
            return;
        }
        self.session_ending = true;
        self.stop_agent_for_session_end();
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
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if session_end_was_confirmed(wparam) {
                end_tray_session(hwnd);
            }
            0
        }
        WM_CLOSE => {
            quit_tray(hwnd, true);
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
                    (!entry.disabled && entry.command != 0).then_some((state.owner, entry.command))
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
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if session_end_was_confirmed(wparam) {
                DestroyWindow(hwnd);
            }
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
            quit_tray(hwnd, true);
        }
        _ => {}
    }
}

fn session_end_was_confirmed(wparam: WPARAM) -> bool {
    wparam != 0
}

fn end_tray_session(hwnd: HWND) {
    quit_tray(hwnd, false);
}

fn quit_tray(hwnd: HWND, wait_for_agent: bool) {
    let _ = with_state(|state| {
        if wait_for_agent {
            state.stop_agent();
        } else {
            state.begin_session_end();
        }
        Ok(())
    });
    remove_tray_icon(hwnd);
    unsafe {
        PostQuitMessage(0);
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
    http_get_body(STATUS_PATH, Duration::from_millis(450))
        .is_some_and(|body| status_body_reports_healthy(&body))
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
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
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

fn configure_agent_command(command: &mut Command, install_dir: &Path, web_dist: PathBuf) {
    command
        .current_dir(install_dir)
        .env("DSCC_WEB_DIST", web_dist)
        .env("DSCC_AGENT_ADDR", agent_spawn_addr())
        .env(LAN_API_ENABLE_ENV, "1")
        .creation_flags(CREATE_NO_WINDOW);
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
mod tests;
