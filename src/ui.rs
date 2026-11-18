use std::ffi::c_void;
use std::mem::size_of;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT,
    WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH,
    DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject,
    DrawTextW, Ellipse, EndPaint, FW_NORMAL, FW_SEMIBOLD, FillRect, GRADIENT_FILL_RECT_V,
    GRADIENT_RECT, GetMonitorInfoW, GetStockObject, GradientFill, HDC, HFONT, HGDIOBJ, LineTo,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MoveToEx, NULL_BRUSH, NULL_PEN,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, Polygon, RoundRect, SRCCOPY, ScreenToClient,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TRIVERTEX,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, ReleaseCapture, SetCapture, SetFocus,
    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, UnregisterHotKey, VK_ESCAPE,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_DROPSHADOW, CreatePopupMenu, CreateWindowExW, DI_NORMAL, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, DrawIconEx, EnumWindows,
    GWLP_USERDATA, GetClassNameW, GetClientRect, GetCursorPos, GetMessageW, GetWindowLongPtrW,
    HCURSOR, HICON, HMENU, HTCAPTION, HTCLIENT, HWND_TOPMOST, IDC_ARROW, IDC_HAND, IDI_APPLICATION,
    IMAGE_ICON, IsWindowVisible, KillTimer, LR_DEFAULTCOLOR, LoadCursorW, LoadIconW, LoadImageW,
    MB_ICONERROR, MB_OK, MENU_ITEM_FLAGS, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG,
    MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SPI_GETWORKAREA, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SetCursor,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    SystemParametersInfoW, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
    WA_INACTIVE, WM_ACTIVATE, WM_APP, WM_CAPTURECHANGED, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU,
    WM_DESTROY, WM_DPICHANGED, WM_ENDSESSION, WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT,
    WM_QUERYENDSESSION, WM_RBUTTONUP, WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

use crate::autostart;
use crate::config::Config;
use crate::modes::{Mode, label_for_raw};
use crate::power;
use crate::wmi::Status;
use crate::worker::{self, ApplyOutcome, Command, Event, WM_APP_EVENT};

const WM_TRAYICON: u32 = WM_APP + 1;
const WM_APP_SHOW: u32 = WM_APP + 2;
const WM_MOUSELEAVE: u32 = 0x02A3;

const TIMER_REFRESH: usize = 1;
const TIMER_TRAY: usize = 2;
const HOTKEY_QUIET: i32 = 1;
const HOTKEY_BALANCED: i32 = 2;
const HOTKEY_BEAST: i32 = 3;
const HOTKEY_PANEL: i32 = 4;

const MENU_QUIET: usize = 1001;
const MENU_BALANCED: usize = 1002;
const MENU_BEAST: usize = 1003;
const MENU_SHOW: usize = 1010;
const MENU_SYNC_OVERLAY: usize = 1020;
const MENU_TELEMETRY: usize = 1021;
const MENU_AUTOSTART: usize = 1030;
const MENU_ABOUT: usize = 1040;
const MENU_EXIT: usize = 1050;

const PANEL_CLASS: &str = "LegionSwitch.Panel";

const BG: u32 = rgb(10, 14, 20);
const BG_TOP: u32 = rgb(15, 24, 37);
const BG_MID: u32 = rgb(15, 20, 28);
const BG_BOTTOM: u32 = rgb(8, 12, 18);
const PANEL_BORDER: u32 = rgb(42, 51, 64);
const CARD: u32 = rgb(16, 22, 31);
const CARD_HOVER: u32 = rgb(23, 31, 43);
const CARD_SELECTED: u32 = rgb(26, 36, 51);
const STATUS_BG: u32 = rgb(14, 20, 29);
const STATUS_BORDER: u32 = rgb(37, 47, 61);
const LINE: u32 = rgb(38, 48, 62);
const TEXT: u32 = rgb(234, 247, 255);
const MUTED: u32 = rgb(153, 166, 184);
const FAINT: u32 = rgb(96, 110, 128);
const RED: u32 = rgb(226, 60, 72);
const AMBER: u32 = rgb(255, 180, 84);
const CYAN: u32 = rgb(60, 141, 255);

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwattribute: u32,
        pvattribute: *const c_void,
        cbattribute: u32,
    ) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    None,
    Card(usize),
    Close,
}

#[derive(Clone, Copy)]
struct Layout {
    width: i32,
    height: i32,
    logo: RECT,
    title: RECT,
    subtitle: RECT,
    divider_y: i32,
    close: RECT,
    cards: [RECT; 3],
    status: RECT,
    hint: RECT,
}

impl Layout {
    fn new(dpi: u32) -> Self {
        let dpi = if dpi == 0 { 96 } else { dpi };
        let s = |value: i32| ((value as f32) * (dpi as f32) / 96.0).round() as i32;
        let width = s(440);
        let height = s(306);

        let close = RECT {
            left: width - s(38),
            top: s(12),
            right: width - s(14),
            bottom: s(36),
        };

        let card_top = s(58);
        let card_height = s(166);
        let card_width = s(120);
        let gap = s(16);
        let left = s(24);
        let cards = [
            RECT {
                left,
                top: card_top,
                right: left + card_width,
                bottom: card_top + card_height,
            },
            RECT {
                left: left + card_width + gap,
                top: card_top,
                right: left + card_width * 2 + gap,
                bottom: card_top + card_height,
            },
            RECT {
                left: left + (card_width + gap) * 2,
                top: card_top,
                right: left + card_width * 3 + gap * 2,
                bottom: card_top + card_height,
            },
        ];

        Self {
            width,
            height,
            logo: RECT {
                left: s(18),
                top: s(12),
                right: s(46),
                bottom: s(40),
            },
            title: RECT {
                left: s(54),
                top: s(9),
                right: width - s(52),
                bottom: s(32),
            },
            subtitle: RECT {
                left: s(54),
                top: s(30),
                right: width - s(52),
                bottom: s(45),
            },
            divider_y: s(50),
            close,
            cards,
            status: RECT {
                left: s(24),
                top: s(234),
                right: width - s(24),
                bottom: s(280),
            },
            hint: RECT {
                left: s(24),
                top: s(284),
                right: width - s(24),
                bottom: s(298),
            },
        }
    }

    fn top_drag_area(&self) -> RECT {
        let scale = self.width as f32 / 440.0;
        RECT {
            left: 0,
            top: 0,
            right: self.width - (44.0 * scale).round() as i32,
            bottom: (52.0 * scale).round() as i32,
        }
    }
}

struct Icons {
    handles: [HICON; 5],
    mode_icons: [HICON; 3],
    fallback: HICON,
}

impl Icons {
    unsafe fn load(instance: HINSTANCE) -> Self {
        let mut handles = [std::ptr::null_mut(); 5];
        for id in 1..=4u16 {
            handles[id as usize] = LoadIconW(instance, resource_id(id));
        }
        let fallback = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        for handle in handles.iter_mut().skip(1) {
            if handle.is_null() {
                *handle = fallback;
            }
        }

        let mut mode_icons = [std::ptr::null_mut(); 3];
        for (index, id) in [5u16, 6, 7].into_iter().enumerate() {
            let handle = LoadImageW(
                instance,
                resource_id(id),
                IMAGE_ICON,
                192,
                192,
                LR_DEFAULTCOLOR,
            );
            if !handle.is_null() {
                mode_icons[index] = handle as HICON;
            }
        }

        Self {
            handles,
            mode_icons,
            fallback,
        }
    }

    fn get(&self, mode: Option<Mode>) -> HICON {
        let index = mode
            .map(|mode| mode.icon_resource_id() as usize)
            .unwrap_or(1);
        let icon = self.handles[index];
        if icon.is_null() { self.fallback } else { icon }
    }

    fn mode_icon(&self, mode: Mode) -> HICON {
        self.mode_icons[mode.index()]
    }

    unsafe fn destroy(&mut self) {
        for icon in &mut self.mode_icons {
            if !icon.is_null() {
                DestroyIcon(*icon);
                *icon = std::ptr::null_mut();
            }
        }
    }
}

struct Fonts {
    dpi: u32,
    title: HFONT,
    subtitle: HFONT,
    card_title: HFONT,
    card_en: HFONT,
    glyph: HFONT,
    badge: HFONT,
    status: HFONT,
    small: HFONT,
    telemetry: HFONT,
}

impl Fonts {
    unsafe fn new(dpi: u32) -> Self {
        let dpi = if dpi == 0 { 96 } else { dpi };
        Self {
            dpi,
            title: create_font(16, dpi, FW_SEMIBOLD, "Bahnschrift"),
            subtitle: create_font(8, dpi, FW_NORMAL, "Bahnschrift"),
            card_title: create_font(16, dpi, FW_SEMIBOLD, "Microsoft YaHei UI"),
            card_en: create_font(8, dpi, FW_NORMAL, "Bahnschrift"),
            glyph: create_font(20, dpi, FW_SEMIBOLD, "Microsoft YaHei UI"),
            badge: create_font(8, dpi, FW_SEMIBOLD, "Microsoft YaHei UI"),
            status: create_font(10, dpi, FW_NORMAL, "Microsoft YaHei UI"),
            small: create_font(8, dpi, FW_NORMAL, "Bahnschrift"),
            telemetry: create_font(9, dpi, FW_NORMAL, "Bahnschrift"),
        }
    }

    unsafe fn destroy(&mut self) {
        for font in [
            &mut self.title,
            &mut self.subtitle,
            &mut self.card_title,
            &mut self.card_en,
            &mut self.glyph,
            &mut self.badge,
            &mut self.status,
            &mut self.small,
            &mut self.telemetry,
        ] {
            if !font.is_null() {
                DeleteObject(*font as HGDIOBJ);
                *font = std::ptr::null_mut();
            }
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe { self.destroy() }
    }
}

struct App {
    hwnd: HWND,
    config: Config,
    worker: Option<Sender<Command>>,
    status: Option<Status>,
    error: Option<String>,
    warning: Option<String>,
    busy: bool,
    pending_mode: Option<Mode>,
    pending_notify: bool,
    hover_card: Option<usize>,
    hover_close: bool,
    pressed_card: Option<usize>,
    pressed_close: bool,
    mouse_tracking: bool,
    last_shown: Instant,
    icons: Icons,
    fonts: Fonts,
    arrow_cursor: HCURSOR,
    hand_cursor: HCURSOR,
    tray_added: bool,
    taskbar_created_message: u32,
    autostart_enabled: Option<bool>,
    cleaned_up: bool,
}

impl App {
    unsafe fn new(hwnd: HWND, instance: HINSTANCE) -> Self {
        let dpi = GetDpiForWindow(hwnd);
        let icons = Icons::load(instance);
        let fonts = Fonts::new(dpi);
        let arrow_cursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        let hand_cursor = LoadCursorW(std::ptr::null_mut(), IDC_HAND);
        Self {
            hwnd,
            config: Config::load(),
            worker: None,
            status: None,
            error: None,
            warning: None,
            busy: false,
            pending_mode: None,
            pending_notify: false,
            hover_card: None,
            hover_close: false,
            pressed_card: None,
            pressed_close: false,
            mouse_tracking: false,
            last_shown: Instant::now() - Duration::from_secs(10),
            icons,
            fonts,
            arrow_cursor,
            hand_cursor,
            tray_added: false,
            taskbar_created_message: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            autostart_enabled: None,
            cleaned_up: false,
        }
    }

    unsafe fn init(&mut self) {
        self.worker = Some(worker::spawn(self.hwnd));
        self.add_tray();
        if !self.tray_added {
            SetTimer(self.hwnd, TIMER_TRAY, 3_000, None);
        }
        self.register_hotkeys();
        self.refresh();
    }

    unsafe fn current_dpi(&self) -> u32 {
        let dpi = GetDpiForWindow(self.hwnd);
        if dpi == 0 { 96 } else { dpi }
    }

    unsafe fn layout(&self) -> Layout {
        Layout::new(self.current_dpi())
    }

    unsafe fn invalidate(&self) {
        windows_sys::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, std::ptr::null(), 0);
    }

    unsafe fn show_panel(&mut self) {
        let layout = self.layout();
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);

        let mut work = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info: MONITORINFO = std::mem::zeroed();
        monitor_info.cbSize = size_of::<MONITORINFO>() as u32;
        let got_monitor = !monitor.is_null() && GetMonitorInfoW(monitor, &mut monitor_info) != 0;
        if got_monitor {
            work = monitor_info.rcWork;
        } else {
            let got_work_area =
                SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut c_void, 0)
                    != 0;
            if !got_work_area {
                work = RECT {
                    left: 0,
                    top: 0,
                    right: windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                        windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
                    ),
                    bottom: windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                        windows_sys::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
                    ),
                };
            }
        }

        let mut x = point.x - layout.width + 24;
        let mut y = point.y - layout.height - 12;
        if y < work.top + 8 {
            y = point.y + 24;
        }
        let min_x = work.left + 8;
        let max_x = (work.right - layout.width - 8).max(min_x);
        let min_y = work.top + 8;
        let max_y = (work.bottom - layout.height - 8).max(min_y);
        x = x.max(min_x).min(max_x);
        y = y.max(min_y).min(max_y);

        SetWindowPos(
            self.hwnd,
            HWND_TOPMOST,
            x,
            y,
            layout.width,
            layout.height,
            SWP_SHOWWINDOW,
        );
        ShowWindow(self.hwnd, SW_SHOW);
        SetForegroundWindow(self.hwnd);
        SetFocus(self.hwnd);
        self.last_shown = Instant::now();

        KillTimer(self.hwnd, TIMER_REFRESH);
        SetTimer(self.hwnd, TIMER_REFRESH, 5_000, None);
        self.refresh();
        self.invalidate();
    }

    unsafe fn hide_panel(&mut self) {
        KillTimer(self.hwnd, TIMER_REFRESH);
        ShowWindow(self.hwnd, SW_HIDE);
        self.hover_card = None;
        self.hover_close = false;
        self.pressed_card = None;
        self.pressed_close = false;
        self.mouse_tracking = false;
        self.invalidate();
    }

    unsafe fn toggle_panel(&mut self) {
        if IsWindowVisible(self.hwnd) != 0 {
            self.hide_panel();
        } else {
            self.show_panel();
        }
    }

    fn apply_mode(&mut self, mode: Mode, notify: bool) {
        if self.busy {
            return;
        }
        let Some(worker) = &self.worker else {
            self.error = Some("WMI 后台线程未启动".to_string());
            unsafe { self.invalidate() };
            return;
        };

        self.busy = true;
        self.pending_mode = Some(mode);
        self.pending_notify = notify;
        self.error = None;
        self.warning = None;
        unsafe { self.invalidate() };

        if worker
            .send(Command::Apply {
                mode,
                sync_overlay: self.config.sync_overlay,
            })
            .is_err()
        {
            self.busy = false;
            self.pending_mode = None;
            self.pending_notify = false;
            self.error = Some("WMI 后台线程已退出".to_string());
            unsafe { self.invalidate() };
        }
    }

    unsafe fn refresh(&mut self) {
        #[cfg(debug_assertions)]
        if std::env::var_os("LEGIONSWITCH_UI_DEMO").is_some() {
            return;
        }
        if self.busy {
            return;
        }
        if let Some(worker) = &self.worker {
            if worker.send(Command::Refresh).is_err() {
                self.error = Some("WMI 后台线程未启动".to_string());
                self.invalidate();
            }
        }
    }

    unsafe fn handle_event(&mut self, event: Event) {
        match event {
            Event::Status(result) => {
                if self.busy {
                    return;
                }
                match result {
                    Ok(status) => {
                        self.status = Some(status);
                        self.error = None;
                        self.warning = None;
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }
                self.update_tray();
                self.invalidate();
            }
            Event::Applied { requested, result } => {
                let notify = self.pending_notify;
                self.busy = false;
                self.pending_mode = None;
                self.pending_notify = false;

                match result {
                    Ok(ApplyOutcome {
                        status,
                        overlay_warning,
                    }) => {
                        let mismatch = status.mode != Some(requested);
                        let detail = if mismatch {
                            Some(format!(
                                "模式已发送，但回读为{}（{}）",
                                label_for_raw(status.raw_mode),
                                status.raw_mode
                            ))
                        } else {
                            overlay_warning
                        };
                        self.status = Some(status);
                        self.error = None;
                        self.warning = detail.clone();

                        if let Some(detail) = detail {
                            self.show_balloon("切换完成（有警告）", &detail, false);
                        } else if notify {
                            self.show_balloon(
                                &format!("已切换到{}", requested.label_zh_full()),
                                "LegionSwitch",
                                false,
                            );
                        }
                    }
                    Err(error) => {
                        self.error = Some(error.clone());
                        self.show_balloon("切换失败", &error, true);
                    }
                }

                self.update_tray();
                self.invalidate();
            }
        }
    }

    unsafe fn add_tray(&mut self) {
        if self.tray_added {
            return;
        }
        let mut nid = self.notify_data();
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = self.icons.get(self.current_mode());
        set_wide(&mut nid.szTip, &self.tooltip_text());
        self.tray_added = Shell_NotifyIconW(NIM_ADD, &nid) != 0;
        if self.tray_added {
            KillTimer(self.hwnd, TIMER_TRAY);
        }
    }

    unsafe fn remove_tray(&mut self) {
        if self.tray_added {
            let nid = self.notify_data();
            Shell_NotifyIconW(NIM_DELETE, &nid);
            self.tray_added = false;
        }
    }

    unsafe fn update_tray(&mut self) {
        if !self.tray_added {
            return;
        }
        let mut nid = self.notify_data();
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = self.icons.get(self.current_mode());
        set_wide(&mut nid.szTip, &self.tooltip_text());
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    unsafe fn show_balloon(&mut self, title: &str, text: &str, error: bool) {
        if !self.tray_added {
            return;
        }
        let mut nid = self.notify_data();
        nid.uFlags = NIF_INFO | NIF_ICON;
        nid.hIcon = self.icons.get(self.current_mode());
        nid.dwInfoFlags = if error { NIIF_ERROR } else { NIIF_INFO };
        nid.Anonymous.uTimeout = 3_000;
        set_wide(&mut nid.szInfoTitle, title);
        set_wide(&mut nid.szInfo, text);
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    fn notify_data(&self) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid
    }

    fn current_mode(&self) -> Option<Mode> {
        self.status.as_ref().and_then(|status| status.mode)
    }

    fn tooltip_text(&self) -> String {
        let mut tip = if let Some(error) = &self.error {
            format!("LegionSwitch - {}", truncate_chars(error, 52))
        } else if let Some(status) = &self.status {
            match status.mode {
                Some(mode) => format!("LegionSwitch - {}", mode.label_zh_full()),
                None => format!("LegionSwitch - {}", label_for_raw(status.raw_mode)),
            }
        } else {
            "LegionSwitch - 正在读取".to_string()
        };

        if self.config.show_telemetry {
            if let Some(status) = &self.status {
                if let Some(cpu) = status.telemetry.cpu_temp {
                    tip.push_str(&format!(" | CPU {cpu}°C"));
                }
                if let Some(gpu) = status.telemetry.gpu_temp {
                    tip.push_str(&format!(" | GPU {gpu}°C"));
                }
                if let Some(fan) = status.telemetry.fan1 {
                    tip.push_str(&format!(" | FAN {fan}RPM"));
                }
            }
        }
        truncate_chars(&tip, 120)
    }

    unsafe fn register_hotkeys(&mut self) {
        let modifiers = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
        let _ = RegisterHotKey(self.hwnd, HOTKEY_QUIET, modifiers, '1' as u32);
        let _ = RegisterHotKey(self.hwnd, HOTKEY_BALANCED, modifiers, '2' as u32);
        let _ = RegisterHotKey(self.hwnd, HOTKEY_BEAST, modifiers, '3' as u32);
        let _ = RegisterHotKey(self.hwnd, HOTKEY_PANEL, modifiers, 'L' as u32);
    }

    unsafe fn unregister_hotkeys(&mut self) {
        for id in [HOTKEY_QUIET, HOTKEY_BALANCED, HOTKEY_BEAST, HOTKEY_PANEL] {
            UnregisterHotKey(self.hwnd, id);
        }
    }

    unsafe fn cleanup(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;
        KillTimer(self.hwnd, TIMER_REFRESH);
        KillTimer(self.hwnd, TIMER_TRAY);
        self.remove_tray();
        self.unregister_hotkeys();
        self.fonts.destroy();
        self.icons.destroy();
        self.worker = None;
    }

    unsafe fn show_tray_menu(&mut self) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }

        if self.autostart_enabled.is_none() {
            self.autostart_enabled = Some(autostart::task_exists());
        }
        let autostart_on = self.autostart_enabled.unwrap_or(false);
        let current = self.current_mode();
        let enabled = !self.busy;

        append_menu(
            menu,
            MENU_QUIET,
            "安静模式\tCtrl+Alt+1",
            current == Some(Mode::Quiet),
            enabled,
        );
        append_menu(
            menu,
            MENU_BALANCED,
            "普通模式\tCtrl+Alt+2",
            current == Some(Mode::Balanced),
            enabled,
        );
        append_menu(
            menu,
            MENU_BEAST,
            "野兽模式\tCtrl+Alt+3",
            current == Some(Mode::Beast),
            enabled,
        );
        append_menu(menu, 0, "", false, true);
        append_menu(menu, MENU_SHOW, "显示面板\tCtrl+Alt+L", false, true);
        append_menu(
            menu,
            MENU_SYNC_OVERLAY,
            "同步 Windows 电源模式",
            self.config.sync_overlay,
            true,
        );
        append_menu(
            menu,
            MENU_TELEMETRY,
            "显示温度/风扇",
            self.config.show_telemetry,
            true,
        );
        append_menu(
            menu,
            MENU_AUTOSTART,
            if autostart_on {
                "关闭开机自启动"
            } else {
                "启用开机自启动"
            },
            autostart_on,
            true,
        );
        append_menu(menu, 0, "", false, true);
        append_menu(menu, MENU_ABOUT, "关于 LegionSwitch", false, true);
        append_menu(menu, MENU_EXIT, "退出", false, true);

        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        SetForegroundWindow(self.hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            self.hwnd,
            std::ptr::null(),
        );
        PostMessageW(self.hwnd, 0, 0, 0);
        DestroyMenu(menu);

        if command > 0 {
            self.handle_command(command as usize);
        }
    }

    unsafe fn handle_command(&mut self, command: usize) {
        match command {
            MENU_QUIET => self.apply_mode(Mode::Quiet, false),
            MENU_BALANCED => self.apply_mode(Mode::Balanced, false),
            MENU_BEAST => self.apply_mode(Mode::Beast, false),
            MENU_SHOW => self.show_panel(),
            MENU_SYNC_OVERLAY => self.toggle_sync_overlay(),
            MENU_TELEMETRY => self.toggle_telemetry(),
            MENU_AUTOSTART => self.toggle_autostart(),
            MENU_ABOUT => self.show_balloon(
                "LegionSwitch 0.2.0",
                "Ctrl+Alt+1/2/3 切换模式，Ctrl+Alt+L 显示面板。",
                false,
            ),
            MENU_EXIT => {
                DestroyWindow(self.hwnd);
            }
            _ => {}
        }
    }

    unsafe fn toggle_sync_overlay(&mut self) {
        self.config.sync_overlay = !self.config.sync_overlay;
        let save_error = self.config.save().err();
        if self.config.sync_overlay {
            if let Some(mode) = self.current_mode() {
                if let Err(error) = power::set_overlay(mode) {
                    self.show_balloon("电源模式同步失败", &error, true);
                }
            }
        }
        if let Some(error) = save_error {
            self.show_balloon("保存设置失败", &error, true);
        }
        self.invalidate();
    }

    unsafe fn toggle_telemetry(&mut self) {
        self.config.show_telemetry = !self.config.show_telemetry;
        if let Err(error) = self.config.save() {
            self.show_balloon("保存设置失败", &error, true);
        }
        self.update_tray();
        self.invalidate();
    }

    unsafe fn toggle_autostart(&mut self) {
        let was_enabled = self
            .autostart_enabled
            .unwrap_or_else(autostart::task_exists);
        let result = if was_enabled {
            autostart::disable()
        } else {
            autostart::enable()
        };
        match result {
            Ok(()) => {
                self.autostart_enabled = Some(!was_enabled);
                self.show_balloon(
                    "开机自启动",
                    if was_enabled {
                        "已关闭开机自启动"
                    } else {
                        "已启用开机自启动（计划任务，最高权限）"
                    },
                    false,
                );
            }
            Err(error) => {
                self.autostart_enabled = None;
                self.show_balloon("开机自启动设置失败", &error, true);
            }
        }
    }

    unsafe fn hit_test(&self, x: i32, y: i32) -> Hit {
        let layout = self.layout();
        if contains(&layout.close, x, y) {
            return Hit::Close;
        }
        for (index, card) in layout.cards.iter().enumerate() {
            if contains(card, x, y) {
                return Hit::Card(index);
            }
        }
        Hit::None
    }

    unsafe fn handle_message(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if self.taskbar_created_message != 0 && message == self.taskbar_created_message {
            self.tray_added = false;
            self.add_tray();
            if !self.tray_added {
                SetTimer(self.hwnd, TIMER_TRAY, 3_000, None);
            }
            return 0;
        }

        match message {
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(self.hwnd, &mut paint);
                self.paint(hdc);
                EndPaint(self.hwnd, &paint);
                0
            }
            WM_ERASEBKGND => 1,
            WM_CLOSE => {
                self.hide_panel();
                0
            }
            WM_DESTROY => {
                self.cleanup();
                PostQuitMessage(0);
                0
            }
            WM_QUERYENDSESSION => 1,
            WM_ENDSESSION => 0,
            WM_ACTIVATE => {
                let active = (wparam as u32) & 0xFFFF;
                if active == WA_INACTIVE
                    && IsWindowVisible(self.hwnd) != 0
                    && self.last_shown.elapsed() > Duration::from_millis(350)
                {
                    self.hide_panel();
                }
                0
            }
            WM_DPICHANGED => {
                let dpi = ((wparam >> 16) & 0xFFFF) as u32;
                if dpi != 0 {
                    self.fonts = Fonts::new(dpi);
                }
                if lparam != 0 {
                    let suggested = &*(lparam as *const RECT);
                    SetWindowPos(
                        self.hwnd,
                        std::ptr::null_mut(),
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                self.invalidate();
                0
            }
            WM_TIMER => {
                if wparam == TIMER_REFRESH && IsWindowVisible(self.hwnd) != 0 {
                    self.refresh();
                } else if wparam == TIMER_TRAY {
                    self.add_tray();
                }
                0
            }
            WM_HOTKEY => {
                match wparam as i32 {
                    HOTKEY_QUIET => self.apply_mode(Mode::Quiet, true),
                    HOTKEY_BALANCED => self.apply_mode(Mode::Balanced, true),
                    HOTKEY_BEAST => self.apply_mode(Mode::Beast, true),
                    HOTKEY_PANEL => self.toggle_panel(),
                    _ => {}
                }
                0
            }
            WM_TRAYICON => {
                let event = lparam as u32;
                if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    self.show_tray_menu();
                } else if event == WM_LBUTTONDBLCLK {
                    self.toggle_panel();
                }
                0
            }
            WM_APP_SHOW => {
                self.show_panel();
                0
            }
            WM_APP_EVENT => {
                if lparam != 0 {
                    let event = *Box::from_raw(lparam as *mut Event);
                    self.handle_event(event);
                }
                0
            }
            WM_MOUSEMOVE => {
                self.on_mouse_move(lparam);
                0
            }
            WM_MOUSELEAVE => {
                self.mouse_tracking = false;
                if self.hover_card.is_some() || self.hover_close {
                    self.hover_card = None;
                    self.hover_close = false;
                    self.invalidate();
                }
                0
            }
            WM_LBUTTONDOWN => {
                self.on_mouse_down(lparam);
                0
            }
            WM_LBUTTONUP => {
                self.on_mouse_up(lparam);
                0
            }
            WM_CAPTURECHANGED => {
                self.pressed_card = None;
                self.pressed_close = false;
                0
            }
            WM_KEYDOWN => {
                if (wparam as u16) == VK_ESCAPE {
                    self.hide_panel();
                }
                0
            }
            WM_SETCURSOR => {
                if ((lparam as u32) & 0xFFFF) == HTCLIENT {
                    let mut point = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut point);
                    ScreenToClient(self.hwnd, &mut point);
                    let cursor = match self.hit_test(point.x, point.y) {
                        Hit::Card(_) | Hit::Close => self.hand_cursor,
                        Hit::None => self.arrow_cursor,
                    };
                    if !cursor.is_null() {
                        SetCursor(cursor);
                    }
                    1
                } else {
                    DefWindowProcW(self.hwnd, message, wparam, lparam)
                }
            }
            WM_NCHITTEST => {
                let mut point = POINT { x: 0, y: 0 };
                GetCursorPos(&mut point);
                ScreenToClient(self.hwnd, &mut point);
                let layout = self.layout();
                if contains(&layout.close, point.x, point.y) {
                    HTCLIENT as LRESULT
                } else if contains(&layout.top_drag_area(), point.x, point.y) {
                    HTCAPTION as LRESULT
                } else {
                    HTCLIENT as LRESULT
                }
            }
            WM_COMMAND => {
                self.handle_command((wparam & 0xFFFF) as usize);
                0
            }
            _ => DefWindowProcW(self.hwnd, message, wparam, lparam),
        }
    }

    unsafe fn on_mouse_move(&mut self, lparam: LPARAM) {
        if !self.mouse_tracking {
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut track);
            self.mouse_tracking = true;
        }

        let (x, y) = mouse_position(lparam);
        let hit = self.hit_test(x, y);
        let new_card = match hit {
            Hit::Card(index) => Some(index),
            _ => None,
        };
        let new_close = hit == Hit::Close;
        if self.hover_card != new_card || self.hover_close != new_close {
            self.hover_card = new_card;
            self.hover_close = new_close;
            self.invalidate();
        }
    }

    unsafe fn on_mouse_down(&mut self, lparam: LPARAM) {
        let (x, y) = mouse_position(lparam);
        match self.hit_test(x, y) {
            Hit::Card(index) => {
                self.pressed_card = Some(index);
                self.pressed_close = false;
                SetCapture(self.hwnd);
            }
            Hit::Close => {
                self.pressed_close = true;
                self.pressed_card = None;
                SetCapture(self.hwnd);
            }
            Hit::None => {
                self.pressed_card = None;
                self.pressed_close = false;
            }
        }
    }

    unsafe fn on_mouse_up(&mut self, lparam: LPARAM) {
        // `ReleaseCapture` synchronously sends WM_CAPTURECHANGED to this
        // window, whose handler clears `pressed_card` / `pressed_close`.
        // Snapshot the press state first, otherwise the click is lost before
        // it can be matched to the card under the cursor.
        let pressed_card = self.pressed_card;
        let pressed_close = self.pressed_close;
        self.pressed_card = None;
        self.pressed_close = false;
        ReleaseCapture();

        let (x, y) = mouse_position(lparam);
        let hit = self.hit_test(x, y);

        if pressed_close && hit == Hit::Close {
            self.hide_panel();
            return;
        }
        if let Some(index) = pressed_card {
            if hit == Hit::Card(index) {
                if let Some(mode) = Mode::ALL.get(index).copied() {
                    self.apply_mode(mode, false);
                }
            }
        }
    }

    unsafe fn paint(&mut self, hdc: HDC) {
        let mut client = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(self.hwnd, &mut client);
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width <= 0 || height <= 0 {
            return;
        }

        let dpi = self.current_dpi();
        if self.fonts.dpi != dpi {
            self.fonts = Fonts::new(dpi);
        }
        let layout = Layout::new(dpi);

        let memory = CreateCompatibleDC(hdc);
        if memory.is_null() {
            self.paint_content(hdc, &layout);
            return;
        }
        let bitmap = CreateCompatibleBitmap(hdc, width, height);
        if bitmap.is_null() {
            DeleteDC(memory);
            self.paint_content(hdc, &layout);
            return;
        }
        let old_bitmap = SelectObject(memory, bitmap as HGDIOBJ);
        self.paint_content(memory, &layout);
        BitBlt(hdc, 0, 0, width, height, memory, 0, 0, SRCCOPY);
        SelectObject(memory, old_bitmap);
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(memory);
    }

    unsafe fn paint_content(&mut self, hdc: HDC, layout: &Layout) {
        self.paint_background(hdc, layout);
        self.paint_header(hdc, layout);

        for (index, mode) in Mode::ALL.into_iter().enumerate() {
            self.paint_mode_button(hdc, layout, index, mode);
        }

        self.paint_status(hdc, layout);

        draw_text(
            hdc,
            "CTRL+ALT+1 / 2 / 3        L  PANEL",
            &layout.hint,
            self.fonts.small,
            FAINT,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
    }

    unsafe fn paint_background(&self, hdc: HDC, layout: &Layout) {
        let client = RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.height,
        };
        let background = CreateSolidBrush(BG);
        FillRect(hdc, &client, background);
        DeleteObject(background as HGDIOBJ);

        let mid_y = layout.height * 56 / 100;
        vertical_gradient(
            hdc,
            &RECT {
                left: 0,
                top: 0,
                right: layout.width,
                bottom: mid_y,
            },
            BG_TOP,
            BG_MID,
        );
        vertical_gradient(
            hdc,
            &RECT {
                left: 0,
                top: mid_y,
                right: layout.width,
                bottom: layout.height,
            },
            BG_MID,
            BG_BOTTOM,
        );

        // Angular shards, kept very close to the background so they read as
        // lighting rather than decoration.
        fill_polygon(
            hdc,
            &[
                (0, 0),
                (s(layout, 214), 0),
                (s(layout, 132), s(layout, 62)),
                (0, s(layout, 96)),
            ],
            blend(BG_TOP, CYAN, 7),
        );
        draw_line(
            hdc,
            s(layout, 214),
            0,
            s(layout, 132),
            s(layout, 62),
            blend(LINE, CYAN, 22),
            s(layout, 1).max(1),
        );
        fill_polygon(
            hdc,
            &[
                (layout.width - s(layout, 150), 0),
                (layout.width, 0),
                (layout.width, s(layout, 78)),
            ],
            blend(BG_TOP, RED, 5),
        );
        draw_line(
            hdc,
            layout.width - s(layout, 150),
            0,
            layout.width,
            s(layout, 78),
            blend(LINE, RED, 20),
            s(layout, 1).max(1),
        );

        // Thin technical corner brackets.
        let inset = s(layout, 10);
        let len = s(layout, 14);
        let bracket = rgb(45, 57, 72);
        let w = layout.width;
        let h = layout.height;
        for (x1, y1, x2, y2) in [
            (inset, inset, inset + len, inset),
            (inset, inset, inset, inset + len),
            (w - inset, inset, w - inset - len, inset),
            (w - inset, inset, w - inset, inset + len),
            (inset, h - inset, inset + len, h - inset),
            (inset, h - inset, inset, h - inset - len),
            (w - inset, h - inset, w - inset - len, h - inset),
            (w - inset, h - inset, w - inset, h - inset - len),
        ] {
            draw_line(hdc, x1, y1, x2, y2, bracket, s(layout, 1).max(1));
        }

        // 1px rounded frame. DWM clips the actual window corners on Win11.
        stroke_round_rect(hdc, &client, s(layout, 14), PANEL_BORDER, 1);
    }

    unsafe fn paint_header(&self, hdc: HDC, layout: &Layout) {
        let app_icon = self.icons.handles[1];
        if !app_icon.is_null() {
            DrawIconEx(
                hdc,
                layout.logo.left,
                layout.logo.top,
                app_icon,
                layout.logo.right - layout.logo.left,
                layout.logo.bottom - layout.logo.top,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            );
        }

        draw_text(
            hdc,
            "LEGIONSWITCH",
            &layout.title,
            self.fonts.title,
            TEXT,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        draw_text(
            hdc,
            "PERFORMANCE MODE",
            &layout.subtitle,
            self.fonts.subtitle,
            MUTED,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );

        // `01 / 02 / 03` mode numbering lives in the cards; the header line is
        // deliberately short and accent-colored so it reads as a power rail.
        let accent = self.current_mode().map(mode_color).unwrap_or(CYAN);
        draw_line(
            hdc,
            s(layout, 24),
            layout.divider_y,
            layout.width - s(layout, 24),
            layout.divider_y,
            LINE,
            s(layout, 1).max(1),
        );
        draw_line(
            hdc,
            s(layout, 24),
            layout.divider_y,
            s(layout, 84),
            layout.divider_y,
            accent,
            s(layout, 2).max(1),
        );

        if self.hover_close {
            fill_round_rect(
                hdc,
                &layout.close,
                s(layout, 8),
                rgb(30, 40, 53),
                rgb(30, 40, 53),
                1,
            );
        }
        let close_center_x = (layout.close.left + layout.close.right) / 2;
        let close_center_y = (layout.close.top + layout.close.bottom) / 2;
        let half = s(layout, 5);
        draw_line(
            hdc,
            close_center_x - half,
            close_center_y - half,
            close_center_x + half,
            close_center_y + half,
            if self.hover_close { TEXT } else { MUTED },
            s(layout, 1).max(1),
        );
        draw_line(
            hdc,
            close_center_x - half,
            close_center_y + half,
            close_center_x + half,
            close_center_y - half,
            if self.hover_close { TEXT } else { MUTED },
            s(layout, 1).max(1),
        );
    }

    unsafe fn paint_mode_button(&self, hdc: HDC, layout: &Layout, index: usize, mode: Mode) {
        let card = &layout.cards[index];
        let selected = self.status.as_ref().and_then(|status| status.mode) == Some(mode);
        let pending = self.busy && self.pending_mode == Some(mode);
        let hovered = self.hover_card == Some(index) && !self.busy;
        let accent = mode_color(mode);

        if selected || pending || hovered {
            let fill = if selected || pending {
                blend(CARD_SELECTED, accent, 8)
            } else {
                CARD_HOVER
            };
            let border = if selected || pending {
                blend(PANEL_BORDER, accent, 55)
            } else {
                rgb(44, 56, 72)
            };
            fill_round_rect(hdc, card, s(layout, 14), fill, border, 1);
        }

        let center_x = (card.left + card.right) / 2;
        let icon_center_y = card.top + s(layout, 60);
        let icon_size = if selected {
            s(layout, 96)
        } else if hovered {
            s(layout, 92)
        } else {
            s(layout, 86)
        };

        if selected || pending {
            let glow_color = blend(BG_MID, accent, 12);
            for extra in [10, 7, 4] {
                stroke_circle(
                    hdc,
                    center_x,
                    icon_center_y,
                    icon_size / 2 + s(layout, extra),
                    blend(BG_MID, glow_color, 45),
                    1,
                );
            }
            stroke_circle(
                hdc,
                center_x,
                icon_center_y,
                icon_size / 2 + s(layout, 2),
                accent,
                s(layout, 2).max(1),
            );
        }

        let icon = self.icons.mode_icon(mode);
        if !icon.is_null() {
            DrawIconEx(
                hdc,
                center_x - icon_size / 2,
                icon_center_y - icon_size / 2,
                icon,
                icon_size,
                icon_size,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            );
        } else {
            let radius = icon_size / 2;
            let circle = RECT {
                left: center_x - radius,
                top: icon_center_y - radius,
                right: center_x + radius,
                bottom: icon_center_y + radius,
            };
            fill_round_rect(
                hdc,
                &circle,
                radius,
                blend(CARD, accent, 14),
                accent,
                s(layout, 2).max(1),
            );
            draw_text(
                hdc,
                mode.glyph(),
                &circle,
                self.fonts.glyph,
                accent,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
        }

        let label_rect = RECT {
            left: card.left,
            top: card.top + s(layout, 110),
            right: card.right,
            bottom: card.top + s(layout, 132),
        };
        draw_text(
            hdc,
            mode.label_zh_full(),
            &label_rect,
            self.fonts.card_title,
            if selected || hovered { TEXT } else { MUTED },
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );

        let english_rect = RECT {
            left: card.left,
            top: card.top + s(layout, 132),
            right: card.right,
            bottom: card.top + s(layout, 144),
        };
        draw_text(
            hdc,
            mode.label_en(),
            &english_rect,
            self.fonts.card_en,
            if selected { accent } else { FAINT },
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );

        let badge_width = s(layout, 54);
        let badge_height = s(layout, 16);
        let badge = RECT {
            left: center_x - badge_width / 2,
            top: card.top + s(layout, 148),
            right: center_x + badge_width / 2,
            bottom: card.top + s(layout, 148) + badge_height,
        };

        if selected {
            fill_round_rect(hdc, &badge, badge_height / 2, accent, accent, 1);
            draw_text(
                hdc,
                "当前",
                &badge,
                self.fonts.badge,
                BG,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
        } else if pending {
            fill_round_rect(hdc, &badge, badge_height / 2, CARD_SELECTED, accent, 1);
            draw_text(
                hdc,
                "切换中",
                &badge,
                self.fonts.badge,
                accent,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
        } else if hovered {
            draw_text(
                hdc,
                "点击切换",
                &badge,
                self.fonts.badge,
                MUTED,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
        }
    }

    unsafe fn paint_status(&self, hdc: HDC, layout: &Layout) {
        let status = self.status.as_ref();
        let mode = status.and_then(|status| status.mode);
        let accent = mode
            .map(mode_color)
            .unwrap_or_else(|| if self.error.is_some() { RED } else { MUTED });

        let border = if self.error.is_some() {
            blend(STATUS_BORDER, RED, 38)
        } else if self.warning.is_some() {
            blend(STATUS_BORDER, AMBER, 38)
        } else {
            STATUS_BORDER
        };
        fill_round_rect(hdc, &layout.status, s(layout, 12), STATUS_BG, border, 1);
        draw_line(
            hdc,
            layout.status.left + s(layout, 14),
            layout.status.top + s(layout, 2),
            layout.status.right - s(layout, 14),
            layout.status.top + s(layout, 2),
            blend(STATUS_BG, TEXT, 7),
            s(layout, 1).max(1),
        );

        let bar_width = s(layout, 3).max(2);
        let bar_height = s(layout, 22);
        let bar_left = layout.status.left + s(layout, 12);
        let bar_top = (layout.status.top + layout.status.bottom) / 2 - bar_height / 2;
        fill_round_rect(
            hdc,
            &RECT {
                left: bar_left,
                top: bar_top,
                right: bar_left + bar_width,
                bottom: bar_top + bar_height,
            },
            bar_width / 2,
            accent,
            accent,
            1,
        );

        let left_rect = RECT {
            left: layout.status.left + s(layout, 25),
            right: layout.status.right - s(layout, 156),
            top: layout.status.top,
            bottom: layout.status.bottom,
        };
        let right_rect = RECT {
            left: layout.status.right - s(layout, 176),
            right: layout.status.right - s(layout, 14),
            top: layout.status.top,
            bottom: layout.status.bottom,
        };

        let (left_text, left_color) = if self.busy {
            let pending = self
                .pending_mode
                .map(|mode| mode.label_zh_full())
                .unwrap_or("模式");
            (format!("正在切换  /  {pending}"), AMBER)
        } else if let Some(error) = &self.error {
            if let Some(status) = status {
                let label = status
                    .mode
                    .map(|mode| mode.label_zh_full())
                    .unwrap_or_else(|| label_for_raw(status.raw_mode));
                (format!("当前模式  /  {label}"), TEXT)
            } else if error.contains("管理员") {
                ("需要管理员权限".to_string(), RED)
            } else {
                ("WMI 接口不可用".to_string(), RED)
            }
        } else if let Some(status) = status {
            let label = status
                .mode
                .map(|mode| mode.label_zh_full())
                .unwrap_or_else(|| label_for_raw(status.raw_mode));
            (format!("当前模式  /  {label}"), TEXT)
        } else {
            ("正在读取模式…".to_string(), MUTED)
        };
        draw_text(
            hdc,
            &left_text,
            &left_rect,
            self.fonts.status,
            left_color,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );

        let (right_text, right_color) = if self.warning.is_some() {
            ("SYNC WARNING".to_string(), AMBER)
        } else if let Some(error) = &self.error {
            if status.is_none() && error.contains("管理员") {
                ("ADMIN REQUIRED".to_string(), RED)
            } else if status.is_some() {
                ("ACTION FAILED".to_string(), RED)
            } else {
                ("CHECK TRAY NOTICE".to_string(), RED)
            }
        } else if let Some(status) = status {
            let telemetry = &status.telemetry;
            if self.config.show_telemetry
                && (telemetry.cpu_temp.is_some() || telemetry.gpu_temp.is_some())
            {
                let mut parts = Vec::new();
                if let Some(cpu) = telemetry.cpu_temp {
                    parts.push(format!("CPU {cpu}°"));
                }
                if let Some(gpu) = telemetry.gpu_temp {
                    parts.push(format!("GPU {gpu}°"));
                }
                (parts.join("   "), MUTED)
            } else if let Some(fan) = telemetry.fan1 {
                (format!("FAN  {fan} RPM"), MUTED)
            } else if let Some(overlay) = status.overlay {
                (format!("POWER  {}", overlay.overlay_name_en()), MUTED)
            } else {
                ("WMI  READY".to_string(), MUTED)
            }
        } else {
            ("ROOT\\WMI".to_string(), FAINT)
        };
        draw_text(
            hdc,
            &right_text,
            &right_rect,
            self.fonts.telemetry,
            right_color,
            DT_RIGHT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
    }
}

struct FindWindowState {
    class: Vec<u16>,
    found: HWND,
}

unsafe extern "system" fn find_window_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    let state = &mut *(lparam as *mut FindWindowState);
    let mut buffer = [0u16; 128];
    let length = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
    if length > 0 && buffer[..length as usize] == state.class[..] {
        state.found = hwnd;
        return 0;
    }
    1
}

unsafe fn find_existing_window() -> HWND {
    let mut state = FindWindowState {
        class: PANEL_CLASS.encode_utf16().collect(),
        found: std::ptr::null_mut(),
    };
    EnumWindows(
        Some(find_window_proc),
        &mut state as *mut FindWindowState as isize,
    );
    state.found
}

pub fn run(show: bool) -> Result<(), String> {
    unsafe {
        let mutex_name = wide("Local\\LegionSwitch.SingleInstance");
        let mutex = windows_sys::Win32::System::Threading::CreateMutexW(
            std::ptr::null(),
            0,
            mutex_name.as_ptr(),
        );
        let already_running = !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS;

        if already_running {
            let existing = find_existing_window();
            if !existing.is_null() {
                if show {
                    SetForegroundWindow(existing);
                    PostMessageW(existing, WM_APP_SHOW, 0, 0);
                }
                CloseHandle(mutex);
                return Ok(());
            }
        }

        let instance = GetModuleHandleW(std::ptr::null());
        let icon = {
            let loaded = LoadIconW(instance, resource_id(1));
            if loaded.is_null() {
                LoadIconW(std::ptr::null_mut(), IDI_APPLICATION)
            } else {
                loaded
            }
        };

        let class_name = wide(PANEL_CLASS);
        let window_title = wide("LegionSwitch");
        let cursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);

        let window_class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: icon,
            hCursor: cursor,
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: icon,
        };
        RegisterClassExW(&window_class);

        let dpi = GetDpiForSystem();
        let layout = Layout::new(dpi);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP,
            0,
            0,
            layout.width,
            layout.height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            if !mutex.is_null() {
                CloseHandle(mutex);
            }
            return Err(crate::wmi::last_os_error_text("创建 LegionSwitch 窗口失败"));
        }
        let corner_preference: i32 = 2;
        DwmSetWindowAttribute(
            hwnd,
            33,
            &corner_preference as *const i32 as *const c_void,
            size_of::<i32>() as u32,
        );

        let app = Box::new(App::new(hwnd, instance));
        let app_pointer = Box::into_raw(app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_pointer as isize);
        #[cfg(debug_assertions)]
        if std::env::var_os("LEGIONSWITCH_UI_DEMO").is_some() {
            (*app_pointer).status = Some(Status {
                raw_mode: Mode::Balanced.smart_fan_value(),
                mode: Some(Mode::Balanced),
                telemetry: crate::wmi::Telemetry {
                    cpu_temp: Some(54),
                    gpu_temp: Some(47),
                    fan1: Some(2_300),
                    fan2: Some(2_250),
                },
                overlay: Some(Mode::Balanced),
            });
        }
        (*app_pointer).init();
        if show {
            (*app_pointer).show_panel();
        }
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        (*app_pointer).cleanup();
        drop(Box::from_raw(app_pointer));
        if !mutex.is_null() {
            CloseHandle(mutex);
        }
        Ok(())
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    (*pointer).handle_message(message, wparam, lparam)
}

fn resource_id(id: u16) -> windows_sys::core::PCWSTR {
    id as usize as windows_sys::core::PCWSTR
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_wide(destination: &mut [u16], text: &str) {
    if destination.is_empty() {
        return;
    }
    let mut index = 0;
    for unit in text.encode_utf16() {
        if index + 1 >= destination.len() {
            break;
        }
        destination[index] = unit;
        index += 1;
    }
    destination[index] = 0;
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut result: String = text.chars().take(max.saturating_sub(1)).collect();
        result.push('…');
        result
    }
}

fn contains(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn mouse_position(lparam: LPARAM) -> (i32, i32) {
    let value = lparam as u32;
    let x = (value & 0xFFFF) as u16 as i16 as i32;
    let y = ((value >> 16) & 0xFFFF) as u16 as i16 as i32;
    (x, y)
}

fn s(layout: &Layout, value: i32) -> i32 {
    // Layout width is 440 at 96 DPI; derive the scale from the rounded
    // dimensions so all call sites stay consistent with `Layout::new`.
    let scale = layout.width as f32 / 440.0;
    ((value as f32) * scale).round() as i32
}

fn mode_color(mode: Mode) -> u32 {
    let (r, g, b) = mode.accent_rgb();
    rgb(r, g, b)
}

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn blend(first: u32, second: u32, percent: u32) -> u32 {
    let percent = percent.min(100);
    let a = first & 0xFF;
    let b = (first >> 8) & 0xFF;
    let c = (first >> 16) & 0xFF;
    let d = second & 0xFF;
    let e = (second >> 8) & 0xFF;
    let f = (second >> 16) & 0xFF;
    let r = (a * (100 - percent) + d * percent) / 100;
    let g = (b * (100 - percent) + e * percent) / 100;
    let bl = (c * (100 - percent) + f * percent) / 100;
    r | (g << 8) | (bl << 16)
}

unsafe fn create_font(pixel_height: i32, dpi: u32, weight: u32, face: &str) -> HFONT {
    let face = wide(face);
    let height = -(((pixel_height as f32) * (dpi as f32) / 96.0).round() as i32).max(1);
    CreateFontW(
        height,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        face.as_ptr(),
    )
}

unsafe fn fill_round_rect(
    hdc: HDC,
    rect: &RECT,
    radius: i32,
    fill: u32,
    border: u32,
    border_width: i32,
) {
    let brush = CreateSolidBrush(fill);
    if brush.is_null() {
        return;
    }
    let pen = if border_width > 0 {
        CreatePen(PS_SOLID, border_width, border)
    } else {
        std::ptr::null_mut()
    };
    let old_brush = SelectObject(hdc, brush as HGDIOBJ);
    let old_pen = if pen.is_null() {
        SelectObject(hdc, GetStockObject(NULL_PEN))
    } else {
        SelectObject(hdc, pen as HGDIOBJ)
    };
    RoundRect(
        hdc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius * 2,
        radius * 2,
    );
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(brush as HGDIOBJ);
    if !pen.is_null() {
        DeleteObject(pen as HGDIOBJ);
    }
}

unsafe fn vertical_gradient(hdc: HDC, rect: &RECT, top: u32, bottom: u32) {
    let vertices = [
        TRIVERTEX {
            x: rect.left,
            y: rect.top,
            Red: ((top & 0xFF) as u16) << 8,
            Green: (((top >> 8) & 0xFF) as u16) << 8,
            Blue: (((top >> 16) & 0xFF) as u16) << 8,
            Alpha: 0,
        },
        TRIVERTEX {
            x: rect.right,
            y: rect.bottom,
            Red: ((bottom & 0xFF) as u16) << 8,
            Green: (((bottom >> 8) & 0xFF) as u16) << 8,
            Blue: (((bottom >> 16) & 0xFF) as u16) << 8,
            Alpha: 0,
        },
    ];
    let mesh = GRADIENT_RECT {
        UpperLeft: 0,
        LowerRight: 1,
    };
    GradientFill(
        hdc,
        vertices.as_ptr(),
        2,
        &mesh as *const GRADIENT_RECT as *const c_void,
        1,
        GRADIENT_FILL_RECT_V,
    );
}

unsafe fn fill_polygon(hdc: HDC, points: &[(i32, i32)], color: u32) {
    if points.len() < 3 {
        return;
    }
    let brush = CreateSolidBrush(color);
    if brush.is_null() {
        return;
    }
    let old_brush = SelectObject(hdc, brush as HGDIOBJ);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let points: Vec<POINT> = points.iter().map(|(x, y)| POINT { x: *x, y: *y }).collect();
    Polygon(hdc, points.as_ptr(), points.len() as i32);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(brush as HGDIOBJ);
}

unsafe fn stroke_round_rect(hdc: HDC, rect: &RECT, radius: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width, color);
    if pen.is_null() {
        return;
    }
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    RoundRect(
        hdc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius * 2,
        radius * 2,
    );
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn stroke_circle(hdc: HDC, cx: i32, cy: i32, radius: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width, color);
    if pen.is_null() {
        return;
    }
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    Ellipse(hdc, cx - radius, cy - radius, cx + radius, cy + radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_line(hdc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width, color);
    if pen.is_null() {
        return;
    }
    let old = SelectObject(hdc, pen as HGDIOBJ);
    MoveToEx(hdc, x1, y1, std::ptr::null_mut());
    LineTo(hdc, x2, y2);
    SelectObject(hdc, old);
    DeleteObject(pen as HGDIOBJ);
}

unsafe fn draw_text(hdc: HDC, text: &str, rect: &RECT, font: HFONT, color: u32, flags: u32) {
    if font.is_null() {
        return;
    }
    let old_font = SelectObject(hdc, font as HGDIOBJ);
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT as i32);
    let wide_text: Vec<u16> = text.encode_utf16().collect();
    let mut bounds = *rect;
    DrawTextW(
        hdc,
        wide_text.as_ptr(),
        wide_text.len() as i32,
        &mut bounds,
        flags,
    );
    SelectObject(hdc, old_font);
}

unsafe fn append_menu(menu: HMENU, id: usize, text: &str, checked: bool, enabled: bool) {
    if text.is_empty() {
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        return;
    }
    let mut flags: MENU_ITEM_FLAGS = MF_STRING;
    if checked {
        flags |= MF_CHECKED;
    }
    if !enabled {
        flags |= MF_GRAYED;
    }
    let wide_text = wide(text);
    AppendMenuW(menu, flags, id, wide_text.as_ptr());
}

pub fn fatal_error(message: &str) {
    let text = wide(message);
    let title = wide("LegionSwitch");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
