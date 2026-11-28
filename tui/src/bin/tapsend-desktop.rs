#![allow(dead_code)] // The terminal and desktop binaries share the same application modules.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("TapSend Desktop is available on Windows; use the terminal binary on Linux.");
}

#[cfg(windows)]
fn main() {
    native::main();
}

#[path = "../app.rs"]
#[cfg(windows)]
mod app;
#[path = "../explorer.rs"]
#[cfg(windows)]
mod explorer;
#[path = "../server.rs"]
#[cfg(windows)]
mod server;
#[path = "../terminal.rs"]
#[cfg(windows)]
mod terminal;
#[path = "../ui.rs"]
#[cfg(windows)]
mod ui;
#[path = "desktop/updates.rs"]
#[cfg(windows)]
mod updates;
#[path = "../windows_console.rs"]
#[cfg(windows)]
mod windows_console;
#[path = "desktop/windows_features.rs"]
#[cfg(windows)]
mod windows_features;

#[cfg(windows)]
mod native {
    use crate::{app, terminal, ui, windows_features as desktop};
    use app::App;
    use std::{
        ffi::c_void,
        io::{self, Read},
        mem::MaybeUninit,
        path::PathBuf,
        process::{Child, Command, Stdio},
        ptr,
        time::{Duration, Instant},
    };
    use terminal::{Key, ModalKey};

    pub(super) type Hwnd = *mut c_void;
    type Hdc = *mut c_void;
    pub(super) type Handle = *mut c_void;
    type Lresult = isize;
    const CLASS: &str = "TapSendDesktopWindow";
    const TITLE_HEIGHT: i32 = 42;
    const MARGIN: i32 = 8;
    const BUTTON: i32 = 46;
    const SCREEN_BUTTON_WIDTH: i32 = 168;
    const WS_POPUP: u32 = 0x80000000;
    const WS_THICKFRAME: u32 = 0x00040000;
    const WS_MINIMIZEBOX: u32 = 0x00020000;
    const WS_EX_APPWINDOW: u32 = 0x00040000;
    const WM_DESTROY: u32 = 0x0002;
    const WM_SIZE: u32 = 0x0005;
    const WM_PAINT: u32 = 0x000f;
    const WM_CLOSE: u32 = 0x0010;
    const WM_ERASEBKGND: u32 = 0x0014;
    const WM_DISPLAYCHANGE: u32 = 0x007e;
    const WM_NCCALCSIZE: u32 = 0x0083;
    const WM_NCHITTEST: u32 = 0x0084;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_CHAR: u32 = 0x0102;
    const WM_TIMER: u32 = 0x0113;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_MOUSEWHEEL: u32 = 0x020a;
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_S: usize = 0x53;
    const VK_R: usize = 0x52;
    const VK_H: usize = 0x48;
    const VK_J: usize = 0x4a;
    const VK_V: usize = 0x56;
    const HTCLIENT: isize = 1;
    const HTCAPTION: isize = 2;
    const HTLEFT: isize = 10;
    const HTRIGHT: isize = 11;
    const HTTOP: isize = 12;
    const HTTOPLEFT: isize = 13;
    const HTTOPRIGHT: isize = 14;
    const HTBOTTOM: isize = 15;
    const HTBOTTOMLEFT: isize = 16;
    const HTBOTTOMRIGHT: isize = 17;
    const GWLP_USERDATA: i32 = -21;
    const BLACK: u32 = 0x0d0d0d;
    const WHITE: u32 = 0xeeeeee;
    const GREY: u32 = 0x808080;
    const SRCCOPY: u32 = 0x00cc0020;
    pub(super) const CREATE_NO_WINDOW: u32 = 0x08000000;
    const HELP_WIDTH: i32 = 110;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub(super) struct Point {
        pub(super) x: i32,
        pub(super) y: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct Paint {
        hdc: Hdc,
        erase: i32,
        rect: Rect,
        restore: i32,
        update: i32,
        reserved: [u8; 32],
    }
    #[repr(C)]
    struct Message {
        hwnd: Hwnd,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: Point,
        private: u32,
    }
    #[repr(C)]
    struct WindowClass {
        style: u32,
        procedure: Option<unsafe extern "system" fn(Hwnd, u32, usize, isize) -> Lresult>,
        class_extra: i32,
        window_extra: i32,
        instance: Handle,
        icon: Handle,
        cursor: Handle,
        background: Handle,
        menu: *const u16,
        class_name: *const u16,
    }
    #[repr(C)]
    struct TextSize {
        cx: i32,
        cy: i32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(class: *const WindowClass) -> u16;
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: Hwnd,
            menu: Handle,
            instance: Handle,
            param: *mut c_void,
        ) -> Hwnd;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> Lresult;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
        fn PostQuitMessage(code: i32);
        fn GetMessageW(msg: *mut Message, hwnd: Hwnd, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Message) -> i32;
        fn GetKeyState(key: i32) -> i16;
        fn DispatchMessageW(msg: *const Message) -> Lresult;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn UpdateWindow(hwnd: Hwnd) -> i32;
        fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: i32) -> i32;
        fn BeginPaint(hwnd: Hwnd, paint: *mut Paint) -> Hdc;
        fn EndPaint(hwnd: Hwnd, paint: *const Paint) -> i32;
        fn FillRect(hdc: Hdc, rect: *const Rect, brush: Handle) -> i32;
        fn SetTimer(hwnd: Hwnd, id: usize, ms: u32, callback: *const c_void) -> usize;
        fn KillTimer(hwnd: Hwnd, id: usize) -> i32;
        fn LoadCursorW(instance: Handle, id: *const u16) -> Handle;
        fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
        fn SetProcessDPIAware() -> i32;
        fn GetDC(hwnd: Hwnd) -> Hdc;
        fn ReleaseDC(hwnd: Hwnd, dc: Hdc) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> Handle;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: Hdc) -> Hdc;
        fn CreateCompatibleBitmap(dc: Hdc, width: i32, height: i32) -> Handle;
        fn DeleteDC(dc: Hdc) -> i32;
        fn BitBlt(
            dst: Hdc,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            source: Hdc,
            sx: i32,
            sy: i32,
            operation: u32,
        ) -> i32;
        fn CreateSolidBrush(color: u32) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn CreateFontW(
            height: i32,
            width: i32,
            escapement: i32,
            orientation: i32,
            weight: i32,
            italic: u32,
            underline: u32,
            strike: u32,
            charset: u32,
            output: u32,
            clip: u32,
            quality: u32,
            pitch: u32,
            face: *const u16,
        ) -> Handle;
        fn SelectObject(hdc: Hdc, object: Handle) -> Handle;
        fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
        fn SetTextColor(hdc: Hdc, color: u32) -> u32;
        fn TextOutW(hdc: Hdc, x: i32, y: i32, text: *const u16, len: i32) -> i32;
        fn GetTextExtentPoint32W(hdc: Hdc, text: *const u16, len: i32, size: *mut TextSize) -> i32;
    }

    pub(super) fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }
    unsafe fn fill(dc: Hdc, rect: Rect, color: u32) {
        let brush = unsafe { CreateSolidBrush(color) };
        unsafe {
            FillRect(dc, &rect, brush);
            DeleteObject(brush);
        }
    }
    unsafe fn label(dc: Hdc, x: i32, y: i32, value: &str, color: u32) {
        let utf16: Vec<u16> = value.encode_utf16().collect();
        unsafe {
            SetTextColor(dc, color);
            TextOutW(dc, x, y, utf16.as_ptr(), utf16.len() as i32);
        }
    }

    struct Host {
        app: App,
        font: Handle,
        cell: i32,
        line: i32,
        error: String,
        scanner: Option<Child>,
        screen_viewer: Option<Child>,
        screen_requested: Option<Instant>,
        screen_choosing_phone: bool,
        background_path: Option<PathBuf>,
        background: bool,
        tray: bool,
        taskbar_created: u32,
        pending_selection: Vec<PathBuf>,
        shortcut_help: bool,
        updates_open: bool,
        updates: crate::updates::Updates,
        pending_surrogate: Option<u16>,
        frame: Option<Frame>,
        buffer: Option<BackBuffer>,
        frame_dirty: bool,
    }

    #[derive(PartialEq, Eq)]
    struct Frame {
        rect: Rect,
        columns: usize,
        rows: usize,
        lines: Vec<String>,
        overlay: Option<(usize, usize, usize, Vec<String>)>,
        update_available: bool,
    }

    struct BackBuffer {
        dc: Hdc,
        bitmap: Handle,
        stock_bitmap: Handle,
        rect: Rect,
    }

    impl BackBuffer {
        fn new(window_dc: Hdc, rect: Rect) -> Option<Self> {
            let dc = unsafe { CreateCompatibleDC(window_dc) };
            if dc.is_null() {
                return None;
            }
            // The window DC supplies the color depth. A fresh memory DC has a
            // monochrome placeholder bitmap until we select the real bitmap.
            let bitmap = unsafe { CreateCompatibleBitmap(window_dc, rect.right, rect.bottom) };
            if bitmap.is_null() {
                unsafe { DeleteDC(dc) };
                return None;
            }
            let stock_bitmap = unsafe { SelectObject(dc, bitmap) };
            if stock_bitmap.is_null() || stock_bitmap as isize == -1 {
                unsafe {
                    DeleteObject(bitmap);
                    DeleteDC(dc);
                }
                return None;
            }
            Some(Self {
                dc,
                bitmap,
                stock_bitmap,
                rect,
            })
        }
    }

    impl Drop for BackBuffer {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.stock_bitmap);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
        }
    }

    impl Host {
        fn update_frame(&mut self, rect: Rect) -> bool {
            let columns = ((rect.right - 2 * MARGIN) / self.cell).max(1) as usize;
            let rows = ((rect.bottom - TITLE_HEIGHT - 2 * MARGIN) / self.line).max(1) as usize;
            let (lines, mut overlay) = self.app.render(columns, rows + 2, &self.error);
            if self.shortcut_help {
                overlay = Some(ui::desktop_help_popup(columns, rows + 2));
            } else if self.updates_open {
                overlay = Some(ui::desktop_update_popup(
                    columns,
                    rows + 2,
                    self.updates.details(),
                ));
            }
            let frame = Frame {
                rect,
                columns,
                rows,
                lines,
                overlay,
                update_available: self.updates.available(),
            };
            if self.frame.as_ref() == Some(&frame) {
                return false;
            }
            self.frame = Some(frame);
            self.frame_dirty = true;
            true
        }
    }

    fn request_frame(hwnd: Hwnd) {
        // Tray and minimized windows still process transfers without rendering.
        if unsafe { IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 } {
            return;
        }
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Host };
        let Some(host) = (unsafe { state.as_mut() }) else {
            return;
        };
        let mut rect = Rect::default();
        if unsafe { GetClientRect(hwnd, &mut rect) } != 0
            && rect.right > 0
            && rect.bottom > 0
            && host.update_frame(rect)
        {
            unsafe { InvalidateRect(hwnd, ptr::null(), 0) };
        }
    }

    fn restore_window(hwnd: Hwnd) {
        unsafe {
            ShowWindow(hwnd, 9);
            desktop::SetForegroundWindow(hwnd);
        }
    }

    fn close_window(hwnd: Hwnd, background: bool) {
        unsafe {
            if background {
                ShowWindow(hwnd, 0);
            } else {
                DestroyWindow(hwnd);
            }
        }
    }

    fn queue_selection(host: &mut Host, paths: Vec<PathBuf>) {
        for path in paths {
            if host.pending_selection.len() >= 1024 {
                host.error = "Review the current File Explorer selection before adding more".into();
                break;
            }
            if !host.pending_selection.contains(&path) {
                host.pending_selection.push(path);
            }
        }
    }

    fn prepare_selection(host: &mut Host) {
        if !host.shortcut_help
            && !host.updates_open
            && !host.pending_selection.is_empty()
            && host.app.can_review_desktop_files()
        {
            host.app
                .review_desktop_files(std::mem::take(&mut host.pending_selection));
        }
    }

    fn open_updates(host: &mut Host) {
        if !host.app.modal_open() {
            host.shortcut_help = false;
            host.updates_open = true;
            host.updates.check(false);
        }
    }

    fn enable_background(hwnd: Hwnd, host: &mut Host, enabled: bool) -> std::io::Result<()> {
        let path = host
            .background_path
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Windows settings folder is unavailable"))?;
        if enabled {
            if !host.tray && !desktop::add_icon(hwnd) {
                return Err(std::io::Error::other(
                    "Windows could not add the TapSend tray icon",
                ));
            }
            host.tray = true;
            if let Err(error) = desktop::save_background(path, true) {
                desktop::remove_icon(hwnd);
                host.tray = false;
                return Err(error);
            }
            host.background = true;
            host.app.desktop_receive();
        } else {
            desktop::save_background(path, false)?;
            desktop::remove_icon(hwnd);
            host.background = false;
            host.tray = false;
        }
        Ok(())
    }

    // TrackPopupMenu runs its own message loop. Only borrow state after it returns.
    unsafe fn windows_settings(hwnd: Hwnd, state: *mut Host, tray: bool) {
        let explorer_path = desktop::explorer_shortcut();
        let installed = explorer_path.as_ref().is_ok_and(|path| path.is_file());
        let background = unsafe { (*state).background };
        let selected = desktop::settings_menu(hwnd, tray, background, installed);
        if unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } != state as isize {
            return;
        }
        if selected == desktop::TRAY_QUIT {
            unsafe { DestroyWindow(hwnd) };
            return;
        }
        if matches!(
            selected,
            desktop::TRAY_OPEN | desktop::TRAY_RECEIVE | desktop::TRAY_UPDATES
        ) {
            restore_window(hwnd);
        }
        let host = unsafe { &mut *state };
        let result = match selected {
            desktop::TRAY_UPDATES => {
                open_updates(host);
                Ok(())
            }
            desktop::TRAY_RECEIVE => {
                host.app.desktop_receive();
                Ok(())
            }
            desktop::TRAY_BACKGROUND => enable_background(hwnd, host, !background).map(|()| {
                host.error = if background {
                    "Background receive off"
                } else {
                    "Background receive on"
                }
                .into();
            }),
            desktop::TRAY_EXPLORER => explorer_path
                .and_then(|path| desktop::set_explorer_shortcut(&path, !installed))
                .map(|()| {
                    host.error = if installed {
                        "File Explorer shortcut removed"
                    } else {
                        "File Explorer: Send to > TapSend Desktop"
                    }
                    .into();
                }),
            _ => Ok(()),
        };
        if let Err(error) = result {
            host.error = error.to_string();
        }
        if tray && !host.background {
            restore_window(hwnd);
        }
        request_frame(hwnd);
    }
    fn phone_share_url(value: &str) -> bool {
        let Some(rest) = value.strip_prefix("http://") else {
            return false;
        };
        let Some((host, path)) = rest.split_once('/') else {
            return false;
        };
        let Some((ip, port)) = host.split_once(':') else {
            return false;
        };
        let Ok(ip) = ip.parse::<std::net::Ipv4Addr>() else {
            return false;
        };
        let octets = ip.octets();
        if !(octets[0] == 10
            || octets[0] == 172 && (16..=31).contains(&octets[1])
            || octets[0] == 192 && octets[1] == 168)
        {
            return false;
        }
        if port.parse::<u16>().unwrap_or(0) == 0 {
            return false;
        }
        let Some(token) = path.strip_prefix("s/") else {
            return false;
        };
        let token = token.strip_suffix('/').unwrap_or(token);
        token.len() == 48
            && token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    #[cfg(test)]
    mod camera_tests {
        use super::phone_share_url;
        #[test]
        fn accepts_only_local_phone_share_links() {
            let token = "a".repeat(48);
            assert!(phone_share_url(&format!(
                "http://192.168.1.5:8080/s/{token}/"
            )));
            assert!(!phone_share_url(&format!(
                "https://192.168.1.5:8080/s/{token}/"
            )));
            assert!(!phone_share_url(&format!("http://8.8.8.8:8080/s/{token}/")));
            assert!(!phone_share_url(&format!(
                "http://192.168.1.5:8080/s/{token}/file/0"
            )));
        }
    }
    fn start_scanner(host: &mut Host) {
        if host.scanner.is_some() {
            host.error = "Camera scanner is already open".into();
            return;
        }
        let path = std::env::current_exe()
            .ok()
            .map(|p| p.with_file_name("TapSend.CameraScanner.exe"));
        let Some(path) = path.filter(|p| p.is_file()) else {
            host.error = "Camera scanner missing. Keep all extracted files together.".into();
            return;
        };
        use std::os::windows::process::CommandExt;
        match Command::new(path)
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => {
                host.scanner = Some(child);
                host.error =
                    "Camera open: point at your phone's TapSend share QR; Esc cancels".into();
            }
            Err(error) => host.error = format!("Could not open camera: {error}"),
        }
    }
    fn finish_scanner(host: &mut Host) {
        let Some(child) = host.scanner.as_mut() else {
            return;
        };
        let Ok(Some(status)) = child.try_wait() else {
            return;
        };
        let mut child = host.scanner.take().unwrap();
        let mut output = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            let _ = stdout.read_to_string(&mut output);
        }
        if status.success() && phone_share_url(output.trim()) {
            host.error.clear();
            host.app.load_phone_share(output.trim().to_owned());
        } else if status.code() == Some(1) {
            host.error = "Camera scan cancelled".into();
        } else {
            let mut error = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut error);
            }
            host.error = error.trim().chars().take(95).collect();
            if host.error.is_empty() {
                host.error = "No valid TapSend phone QR scanned".into();
            }
        }
    }

    fn stop_screen_viewer(host: &mut Host) {
        host.screen_requested = None;
        host.screen_choosing_phone = false;
        host.app.close_screen();
        if let Some(mut viewer) = host.screen_viewer.take() {
            let _ = viewer.kill();
            let _ = viewer.wait();
        }
    }

    fn toggle_screen_viewer(host: &mut Host) {
        if host.screen_viewer.is_some() || host.screen_requested.is_some() {
            stop_screen_viewer(host);
            host.error = "Screen viewer closed".into();
        } else if host.app.has_recipient() {
            match host.app.request_screen() {
                Ok(()) => {
                    host.screen_requested = Some(Instant::now());
                    host.error = "Opening screen viewer…".into();
                }
                Err(error) => host.error = error.to_string(),
            }
        } else {
            host.screen_choosing_phone = host.app.pick_screen_phone();
            host.error.clear();
        }
    }

    fn update_screen_viewer(host: &mut Host) {
        if host.screen_choosing_phone && !host.app.modal_open() {
            host.screen_choosing_phone = false;
            if host.app.has_recipient() {
                toggle_screen_viewer(host);
            }
        }
        if let Some(started) = host.screen_requested {
            if let Some(result) = host.app.take_screen_ticket() {
                host.screen_requested = None;
                let viewer = result.and_then(|ticket| {
                    use std::os::windows::process::CommandExt;
                    let helper = std::env::current_exe()
                        .map_err(|error| error.to_string())?
                        .with_file_name("TapSend.CameraScanner.exe");
                    if !helper.is_file() {
                        return Err(
                            "Screen viewer missing. Keep the extracted bundle together.".into()
                        );
                    }
                    Command::new(helper)
                        .creation_flags(CREATE_NO_WINDOW)
                        .args([
                            "--screen",
                            &ticket.port.to_string(),
                            &ticket.key,
                            &ticket.pin,
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|error| format!("Could not open screen viewer: {error}"))
                });
                match viewer {
                    Ok(viewer) => {
                        host.screen_viewer = Some(viewer);
                        host.error =
                            "Screen viewer open · Android Tools → Share screen to PC".into();
                    }
                    Err(error) => {
                        host.app.close_screen();
                        host.error = error;
                    }
                }
            } else if started.elapsed() > Duration::from_secs(5) {
                stop_screen_viewer(host);
                host.error =
                    "Screen viewer did not respond. Check that the server is running.".into();
            }
        }
        if let Some(viewer) = host.screen_viewer.as_mut()
            && let Ok(Some(status)) = viewer.try_wait()
        {
            host.screen_viewer = None;
            host.app.close_screen();
            host.error = if status.success() {
                "Screen viewer closed".into()
            } else {
                "Screen viewer stopped unexpectedly. Reopen with L.".into()
            };
        }
    }
    #[derive(Clone, Copy, PartialEq)]
    struct Style {
        fg: u32,
        bg: u32,
        dim: bool,
    }
    impl Default for Style {
        fn default() -> Self {
            Self {
                fg: WHITE,
                bg: BLACK,
                dim: false,
            }
        }
    }
    fn width_of(c: char) -> usize {
        let n = c as u32;
        if c.is_control() || matches!(n, 0x0300..=0x036f | 0xfe00..=0xfe0f) {
            0
        } else if matches!(n, 0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xff01..=0xff60 | 0xffe0..=0xffe6 | 0x1f000..=0x1faff)
        {
            2
        } else {
            1
        }
    }
    fn style_code(style: &mut Style, code: u32) {
        match code {
            0 => *style = Style::default(),
            2 => style.dim = true,
            22 => style.dim = false,
            30 => style.fg = BLACK,
            37 | 97 => style.fg = WHITE,
            39 => style.fg = WHITE,
            40 | 49 => style.bg = BLACK,
            47 | 107 => style.bg = WHITE,
            _ => {}
        }
    }
    fn qr_halves(ch: char, style: Style) -> Option<(bool, bool)> {
        if style.bg != WHITE || style.fg != BLACK {
            return None;
        }
        match ch {
            '█' => Some((true, true)),
            '▀' => Some((true, false)),
            '▄' => Some((false, true)),
            _ => None,
        }
    }
    #[cfg(test)]
    mod qr_tests {
        use super::*;

        #[test]
        fn terminal_qr_halves_map_to_solid_pixels() {
            let mut style = Style::default();
            style_code(&mut style, 47);
            style_code(&mut style, 30);
            assert_eq!(qr_halves('█', style), Some((true, true)));
            assert_eq!(qr_halves('▀', style), Some((true, false)));
            assert_eq!(qr_halves('▄', style), Some((false, true)));
            assert_eq!(qr_halves('█', Style::default()), None);
        }
    }
    unsafe fn draw_line(dc: Hdc, line: &str, x: i32, y: i32, cell: i32, line_h: i32, limit: usize) {
        let mut style = Style::default();
        let mut col = 0usize;
        let mut part = String::new();
        let mut part_style = style;
        let mut part_col = 0usize;
        let mut chars = line.chars().peekable();
        let flush = |part: &mut String, at: usize, style: Style| {
            if part.is_empty() {
                return;
            }
            let cells = part.chars().map(width_of).sum::<usize>() as i32;
            if style.bg != BLACK {
                unsafe {
                    fill(
                        dc,
                        Rect {
                            left: x + at as i32 * cell,
                            top: y,
                            right: x + (at as i32 + cells) * cell,
                            bottom: y + line_h,
                        },
                        style.bg,
                    )
                };
            }
            unsafe {
                label(
                    dc,
                    x + at as i32 * cell,
                    y + 2,
                    part,
                    if style.dim { GREY } else { style.fg },
                )
            };
            part.clear();
        };
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                flush(&mut part, part_col, part_style);
                if chars.next() == Some('[') {
                    let mut codes = String::new();
                    for c in chars.by_ref() {
                        if c == 'm' {
                            break;
                        }
                        if codes.len() < 48 {
                            codes.push(c);
                        }
                    }
                    for code in codes.split(';').filter_map(|n| n.parse::<u32>().ok()) {
                        style_code(&mut style, code);
                    }
                }
                part_style = style;
                part_col = col;
                continue;
            }
            if ch.is_control() {
                continue;
            }
            let units = width_of(ch);
            if col + units > limit {
                break;
            }
            if let Some((top, bottom)) = qr_halves(ch, style) {
                flush(&mut part, part_col, part_style);
                let left = x + col as i32 * cell;
                let middle = y + line_h / 2;
                unsafe {
                    fill(
                        dc,
                        Rect {
                            left,
                            top: y,
                            right: left + cell,
                            bottom: y + line_h,
                        },
                        WHITE,
                    );
                    if top {
                        fill(
                            dc,
                            Rect {
                                left,
                                top: y,
                                right: left + cell,
                                bottom: middle,
                            },
                            BLACK,
                        );
                    }
                    if bottom {
                        fill(
                            dc,
                            Rect {
                                left,
                                top: middle,
                                right: left + cell,
                                bottom: y + line_h,
                            },
                            BLACK,
                        );
                    }
                }
                col += units;
                part_col = col;
                continue;
            }
            if part.is_empty() {
                part_style = style;
                part_col = col;
            }
            part.push(ch);
            col += units;
        }
        flush(&mut part, part_col, part_style);
    }

    unsafe fn draw_frame(dc: Hdc, frame: &Frame, font: Handle, cell: i32, line_height: i32) {
        let rect = frame.rect;
        unsafe { fill(dc, rect, BLACK) };
        let old = unsafe { SelectObject(dc, font) };
        unsafe {
            SetBkMode(dc, 1);
        }
        unsafe {
            fill(
                dc,
                Rect {
                    bottom: TITLE_HEIGHT,
                    ..rect
                },
                0x141414,
            );
        }
        unsafe {
            if frame.update_available {
                let left = update_button_left(rect.right, cell);
                if left > 16 {
                    label(dc, 16, 10, ">_  TapSend Desktop", WHITE);
                }
                label(dc, left, 10, "[I] UPDATE", 0x777bff);
            } else {
                label(dc, 16, 10, ">_  TapSend Desktop", WHITE);
            }
            if rect.right >= 330 {
                label(
                    dc,
                    rect.right - 2 * BUTTON - HELP_WIDTH + 10,
                    10,
                    "[N] HELP",
                    GREY,
                );
            }
            if let Some(left) = screen_button_left(rect.right) {
                fill(
                    dc,
                    Rect {
                        left,
                        top: 5,
                        right: left + SCREEN_BUTTON_WIDTH - 4,
                        bottom: TITLE_HEIGHT - 5,
                    },
                    0x242424,
                );
                label(dc, left + 8, 10, "[L] SCREEN SHARE", WHITE);
            }
        }
        for (index, symbol) in ["─", "×"].iter().enumerate() {
            let left = rect.right - (2 - index as i32) * BUTTON;
            unsafe {
                label(dc, left + 16, 10, symbol, WHITE);
            }
        }
        unsafe {
            fill(
                dc,
                Rect {
                    left: 0,
                    top: TITLE_HEIGHT - 1,
                    right: rect.right,
                    bottom: TITLE_HEIGHT,
                },
                GREY,
            );
        }
        let columns = frame.columns;
        let rows = frame.rows;
        for (index, line) in frame.lines.iter().skip(2).take(rows).enumerate() {
            unsafe {
                draw_line(
                    dc,
                    line,
                    MARGIN,
                    TITLE_HEIGHT + MARGIN + index as i32 * line_height,
                    cell,
                    line_height,
                    columns,
                );
            }
        }
        if let Some((left, top, width, popup)) = &frame.overlay {
            for (index, line) in popup.iter().enumerate() {
                let x = MARGIN + *left as i32 * cell;
                let y =
                    TITLE_HEIGHT + MARGIN + (top + index).saturating_sub(2) as i32 * line_height;
                unsafe {
                    fill(
                        dc,
                        Rect {
                            left: x,
                            top: y,
                            right: x + *width as i32 * cell,
                            bottom: y + line_height,
                        },
                        BLACK,
                    );
                    draw_line(dc, line, x, y, cell, line_height, *width);
                }
            }
        }
        unsafe {
            SelectObject(dc, old);
        }
    }

    unsafe fn paint(hwnd: Hwnd, host: &mut Host) {
        let mut paint = MaybeUninit::<Paint>::zeroed();
        let window_dc = unsafe { BeginPaint(hwnd, paint.as_mut_ptr()) };
        if window_dc.is_null() {
            return;
        }
        let paint = unsafe { paint.assume_init() };
        let mut rect = Rect::default();
        if unsafe { GetClientRect(hwnd, &mut rect) } != 0 && rect.right > 0 && rect.bottom > 0 {
            if host.frame.as_ref().is_none_or(|frame| frame.rect != rect) {
                host.update_frame(rect);
            }
            if let Some(frame) = &host.frame {
                if host
                    .buffer
                    .as_ref()
                    .is_none_or(|buffer| buffer.rect != rect)
                {
                    host.buffer = BackBuffer::new(window_dc, rect);
                    host.frame_dirty = true;
                }
                if let Some(buffer) = &host.buffer {
                    if host.frame_dirty {
                        unsafe { draw_frame(buffer.dc, frame, host.font, host.cell, host.line) };
                        host.frame_dirty = false;
                    }
                    let area = paint.rect;
                    let width = area.right - area.left;
                    let height = area.bottom - area.top;
                    if width > 0
                        && height > 0
                        && unsafe {
                            BitBlt(
                                window_dc, area.left, area.top, width, height, buffer.dc,
                                area.left, area.top, SRCCOPY,
                            )
                        } == 0
                    {
                        // A display change can invalidate a device-dependent bitmap.
                        host.buffer = None;
                        unsafe { draw_frame(window_dc, frame, host.font, host.cell, host.line) };
                    }
                } else {
                    unsafe { draw_frame(window_dc, frame, host.font, host.cell, host.line) };
                }
            }
        }
        unsafe { EndPaint(hwnd, &paint) };
    }

    #[cfg(test)]
    mod paint_tests {
        use super::*;

        #[link(name = "gdi32")]
        unsafe extern "system" {
            fn GetPixel(dc: Hdc, x: i32, y: i32) -> u32;
            fn GetStockObject(index: i32) -> Handle;
        }

        #[test]
        fn complete_frame_reaches_a_color_surface_in_one_copy() {
            let screen = unsafe { GetDC(ptr::null_mut()) };
            assert!(!screen.is_null());
            let rect = Rect {
                right: 200,
                bottom: 140,
                ..Rect::default()
            };
            let source = BackBuffer::new(screen, rect);
            let destination = BackBuffer::new(screen, rect);
            unsafe { ReleaseDC(ptr::null_mut(), screen) };
            let source = source.expect("create a frame buffer");
            let destination = destination.expect("create a destination surface");
            let frame = Frame {
                rect,
                columns: 20,
                rows: 2,
                update_available: false,
                lines: vec![String::new(), String::new(), "\x1b[47;30m▄\x1b[0m".into()],
                overlay: None,
            };
            let font = unsafe { GetStockObject(17) };
            unsafe {
                draw_frame(source.dc, &frame, font, 9, 20);
                assert_ne!(
                    BitBlt(
                        destination.dc,
                        0,
                        0,
                        rect.right,
                        rect.bottom,
                        source.dc,
                        0,
                        0,
                        SRCCOPY
                    ),
                    0
                );
                assert_eq!(GetPixel(destination.dc, 1, 1), 0x141414);
                assert_eq!(
                    GetPixel(destination.dc, MARGIN + 1, TITLE_HEIGHT + MARGIN + 1),
                    WHITE
                );
                assert_eq!(
                    GetPixel(destination.dc, MARGIN + 1, TITLE_HEIGHT + MARGIN + 11),
                    BLACK
                );
                assert_eq!(GetPixel(destination.dc, 1, rect.bottom - 1), BLACK);
            }
        }
    }

    fn char_key(c: char) -> Key {
        match c.to_ascii_lowercase() {
            'd' => Key::View(0),
            'f' => Key::Files,
            'v' => Key::Devices,
            't' => Key::View(3),
            'm' => Key::View(4),
            'r' => Key::Receive,
            'e' => Key::Compose,
            'x' => Key::ToggleTransfers,
            'u' => Key::RefreshPairing,
            ':' | ';' => Key::Command,
            'g' => Key::Settings,
            's' => Key::Start,
            'a' => Key::Add,
            'h' => Key::Hidden,
            'l' => Key::Enter,
            'q' => Key::Quit,
            'k' => Key::Up,
            'j' => Key::Down,
            ' ' => Key::Space,
            _ => Key::Other,
        }
    }

    fn help_button_hit(width: i32, x: i32) -> bool {
        width >= 330 && (width - 2 * BUTTON - HELP_WIDTH..width - 2 * BUTTON).contains(&x)
    }

    fn screen_button_left(width: i32) -> Option<i32> {
        (width >= 580).then(|| width - 2 * BUTTON - HELP_WIDTH - SCREEN_BUTTON_WIDTH)
    }

    fn update_button_left(width: i32, cell: i32) -> i32 {
        let side = screen_button_left(width).unwrap_or(width - 2 * BUTTON - HELP_WIDTH);
        let left = 28 + 19 * cell;
        if left + 11 * cell < side { left } else { 16 }
    }

    fn update_button_hit(host: &Host, width: i32, x: i32) -> bool {
        if !host.updates.available() {
            return false;
        }
        let left = update_button_left(width, host.cell);
        (left..left + 10 * host.cell).contains(&x)
    }

    fn screen_button_hit(width: i32, x: i32) -> bool {
        screen_button_left(width)
            .is_some_and(|left| (left..left + SCREEN_BUTTON_WIDTH).contains(&x))
    }

    #[test]
    fn screen_button_and_help_have_separate_hit_targets() {
        let left = screen_button_left(600).unwrap();
        assert!(screen_button_hit(600, left));
        assert!(!help_button_hit(600, left));
        assert!(!screen_button_hit(600, left + SCREEN_BUTTON_WIDTH));
        assert!(help_button_hit(600, left + SCREEN_BUTTON_WIDTH));
        assert!(screen_button_left(320).is_none());
    }

    #[cfg(test)]
    #[test]
    fn desktop_letters_open_views_without_losing_transfer_controls() {
        assert!(matches!(char_key('D'), Key::View(0)));
        assert!(matches!(char_key('F'), Key::Files));
        assert!(matches!(char_key('V'), Key::Devices));
        assert!(matches!(char_key('T'), Key::View(3)));
        assert!(matches!(char_key('M'), Key::View(4)));
        assert!(matches!(char_key('R'), Key::Receive));
        assert!(matches!(char_key('X'), Key::ToggleTransfers));
        assert!(matches!(char_key('U'), Key::RefreshPairing));
        assert!(matches!(char_key('E'), Key::Compose));
        assert!(matches!(char_key('1'), Key::Other));
    }

    fn help_key_at(host: &Host, x: i32, y: i32) -> Option<char> {
        let (left, top, width, popup) = host.frame.as_ref()?.overlay.as_ref()?;
        if x < MARGIN || y < TITLE_HEIGHT + MARGIN {
            return None;
        }
        let col = ((x - MARGIN) / host.cell) as usize;
        let row = ((y - TITLE_HEIGHT - MARGIN) / host.line) as usize;
        let column = col.checked_sub(*left + 2)?;
        let index = row.checked_sub(top.saturating_sub(2) + 2)?;
        if column >= width.saturating_sub(4) || index + 3 >= popup.len() {
            return None;
        }
        let side = usize::from(column >= width.saturating_sub(4) / 2);
        ui::DESKTOP_HELP_KEYS.get(index).map(|keys| keys[side].0)
    }

    fn recipient_row_at(host: &Host, x: i32, y: i32) -> Option<(usize, usize)> {
        let frame = host.frame.as_ref()?;
        let (left, top, width, _) = frame.overlay.as_ref()?;
        let col = x.checked_sub(MARGIN).filter(|value| *value >= 0)? / host.cell;
        let row = y
            .checked_sub(TITLE_HEIGHT + MARGIN)
            .filter(|value| *value >= 0)?
            / host.line;
        let col = (col as usize).checked_sub(*left)?;
        if col < 1 || col >= width.saturating_sub(1) {
            return None;
        }
        let first = (top + 1).saturating_sub(2);
        Some(((row as usize).checked_sub(first)?, frame.rows + 2))
    }

    unsafe fn desktop_char(hwnd: Hwnd, state: *mut Host, key: char) {
        let lower = key.to_ascii_lowercase();
        if unsafe { (*state).updates_open } {
            let host = unsafe { &mut *state };
            match lower {
                'r' => host.updates.check(true),
                'i' => host.updates_open = false,
                _ => {}
            }
            request_frame(hwnd);
            return;
        }
        if lower == 'i' && !unsafe { (*state).app.modal_open() } {
            open_updates(unsafe { &mut *state });
            request_frame(hwnd);
            return;
        }
        if matches!(lower, 'n' | '?') && !unsafe { (*state).app.modal_open() } {
            unsafe { (*state).shortcut_help = !(*state).shortcut_help };
            request_frame(hwnd);
            return;
        }
        if lower == 'w' && !unsafe { (*state).app.modal_open() } {
            unsafe { (*state).shortcut_help = false };
            unsafe { windows_settings(hwnd, state, false) };
            return;
        }
        let host = unsafe { &mut *state };
        if lower == 'y'
            && !host.app.modal_open()
            && let Some(pass) = host.app.desktop_pairing_pass()
        {
            host.error = match pass.and_then(|value| desktop::copy_clipboard(hwnd, &value)) {
                Ok(()) => "Pairing pass copied · paste it in TapSend on your phone".into(),
                Err(error) => error.to_string(),
            };
            request_frame(hwnd);
            return;
        }
        let result = if host.app.modal_open() {
            if lower == 'p' && host.app.scratchpad_open() {
                let result =
                    desktop::read_clipboard(hwnd).and_then(|text| host.app.share_clipboard(&text));
                if let Err(error) = result {
                    host.app.scratchpad_status(error.to_string());
                }
                Ok(())
            } else {
                host.app.handle_modal(ModalKey::Char(key))
            }
        } else {
            let mapped = char_key(key);
            if matches!(mapped, Key::Other) && !matches!(lower, 'c' | 'p' | 'k' | 'l') {
                return;
            }
            host.shortcut_help = false;
            host.error.clear();
            if lower == 'c' {
                start_scanner(host);
                Ok(())
            } else if lower == 'k' {
                host.app.desktop_scratchpad();
                Ok(())
            } else if lower == 'l' {
                toggle_screen_viewer(host);
                Ok(())
            } else if lower == 'p' {
                host.app.desktop_pair()
            } else if lower == 'u' {
                host.app.desktop_refresh_pairing();
                Ok(())
            } else if lower == 'm' && host.app.desktop_pairing_details() {
                Ok(())
            } else if lower == 'r' {
                host.app.desktop_receive_key()
            } else if lower == 'v' {
                host.app.desktop_devices()
            } else if matches!(lower, 's' | 'a') {
                host.app.desktop_share_selected();
                Ok(())
            } else {
                host.app.handle_key(mapped)
            }
        };
        if let Err(error) = result {
            host.error = error.to_string();
        }
        if host.app.quitting() {
            unsafe { DestroyWindow(hwnd) };
            return;
        }
        request_frame(hwnd);
    }

    fn modal_special(vk: usize) -> Option<ModalKey> {
        Some(match vk {
            0x26 => ModalKey::Up,
            0x28 => ModalKey::Down,
            0x0d => ModalKey::Enter,
            0x08 => ModalKey::Backspace,
            0x1b => ModalKey::Escape,
            0x09 => ModalKey::Tab,
            _ => return None,
        })
    }
    fn ordinary_special(vk: usize) -> Option<Key> {
        Some(match vk {
            0x26 => Key::Up,
            0x28 => Key::Down,
            0x0d => Key::Enter,
            0x08 => Key::Back,
            0x09 => Key::NextView,
            0x1b => Key::Other,
            _ => return None,
        })
    }

    fn file_search_chord(key: usize, shift: bool, control: bool) -> bool {
        key == VK_S && shift && !control
    }

    fn paste_stack_chord(key: usize, shift: bool, control: bool) -> bool {
        key == VK_V && shift && !control
    }

    fn copy_paste_item(hwnd: Hwnd, item: &crate::server::PasteItem) -> io::Result<()> {
        if item.file {
            return desktop::copy_clipboard_file(hwnd, &item.path);
        }
        let mut content = Vec::new();
        let limit = if item.image { 48_000_040 } else { 16 * 1024 };
        std::fs::File::open(&item.path)?
            .take(limit + 1)
            .read_to_end(&mut content)?;
        if content.len() as u64 > limit {
            return Err(io::Error::other("Paste Stack item exceeds its size limit"));
        }
        if item.image {
            desktop::copy_clipboard_image(hwnd, &content)
        } else {
            let text = std::str::from_utf8(&content).map_err(io::Error::other)?;
            if text.contains('\0') {
                return Err(io::Error::other("Paste Stack text is invalid"));
            }
            desktop::copy_clipboard(hwnd, text)
        }
    }

    #[test]
    fn file_search_requires_shift_without_affecting_plain_send() {
        assert!(file_search_chord(VK_S, true, false));
        assert!(!file_search_chord(VK_S, false, false));
        assert!(!file_search_chord(VK_S, true, true));
        assert!(!file_search_chord(0x56, true, false));
        assert!(paste_stack_chord(VK_V, true, false));
        assert!(!paste_stack_chord(VK_V, false, false));
        assert!(!paste_stack_chord(VK_V, true, true));
    }

    unsafe extern "system" fn window_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> Lresult {
        let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Host };
        if !state.is_null() && msg != 0 && msg == unsafe { (*state).taskbar_created } {
            let host = unsafe { &mut *state };
            host.tray = host.background && desktop::add_icon(hwnd);
            if host.background && !host.tray {
                restore_window(hwnd);
            }
            return 0;
        }
        match msg {
            desktop::COPYDATA if !state.is_null() => {
                let Some(paths) = (unsafe { desktop::copy_selection(l) }) else {
                    return 0;
                };
                queue_selection(unsafe { &mut *state }, paths);
                restore_window(hwnd);
                return 1;
            }
            desktop::TRAY_CALLBACK if !state.is_null() => {
                match l as u32 {
                    0x0202 | 0x0203 | 0x0400 | 0x0401 => restore_window(hwnd),
                    0x0205 => unsafe { windows_settings(hwnd, state, true) },
                    0x0405 => {
                        restore_window(hwnd);
                        let _ = unsafe { (*state).app.handle_key(Key::View(3)) };
                        request_frame(hwnd);
                    }
                    _ => {}
                }
                return 0;
            }
            WM_NCCALCSIZE if w != 0 => return 0,
            WM_NCHITTEST => {
                let mut rect = Rect::default();
                unsafe {
                    GetWindowRect(hwnd, &mut rect);
                }
                let x = (l as u32 & 0xffff) as i16 as i32 - rect.left;
                let y = ((l as u32 >> 16) & 0xffff) as i16 as i32 - rect.top;
                let at_left = x < 7;
                let at_right = x >= rect.right - rect.left - 7;
                let at_top = y < 7;
                let at_bottom = y >= rect.bottom - rect.top - 7;
                if at_top && at_left {
                    return HTTOPLEFT;
                }
                if at_top && at_right {
                    return HTTOPRIGHT;
                }
                if at_bottom && at_left {
                    return HTBOTTOMLEFT;
                }
                if at_bottom && at_right {
                    return HTBOTTOMRIGHT;
                }
                if at_left {
                    return HTLEFT;
                }
                if at_right {
                    return HTRIGHT;
                }
                if at_top {
                    return HTTOP;
                }
                if at_bottom {
                    return HTBOTTOM;
                }
                if y < TITLE_HEIGHT
                    && (help_button_hit(rect.right - rect.left, x)
                        || screen_button_hit(rect.right - rect.left, x)
                        || (!state.is_null()
                            && update_button_hit(unsafe { &*state }, rect.right - rect.left, x)))
                {
                    return HTCLIENT;
                }
                if y < TITLE_HEIGHT && x < rect.right - rect.left - 2 * BUTTON {
                    return HTCAPTION;
                }
                return HTCLIENT;
            }
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                if !state.is_null() {
                    unsafe {
                        paint(hwnd, &mut *state);
                    }
                } else {
                    let mut ps = MaybeUninit::<Paint>::zeroed();
                    unsafe {
                        let _ = BeginPaint(hwnd, ps.as_mut_ptr());
                        EndPaint(hwnd, ps.as_ptr());
                    }
                }
                return 0;
            }
            WM_TIMER if !state.is_null() => {
                let host = unsafe { &mut *state };
                host.app.tick();
                if host.app.updates_allowed() {
                    host.updates.check(false);
                }
                if let Some(message) = host.updates.poll()
                    && host.tray
                {
                    desktop::notify(hwnd, "TapSend update available", &message);
                }
                if let Some(pass) = host.app.take_relay_pass() {
                    host.error = match desktop::copy_clipboard(hwnd, &pass) {
                        Ok(()) => "Relay pass copied · use Shift+J on the sending PC".into(),
                        Err(error) => error.to_string(),
                    };
                }
                finish_scanner(host);
                update_screen_viewer(host);
                if host.app.modal_open() {
                    host.shortcut_help = false;
                    host.updates_open = false;
                }
                prepare_selection(host);
                let notices = host.app.desktop_notices();
                if host.tray {
                    if let Some((title, message)) = notices.last() {
                        let received = notices
                            .iter()
                            .filter(|(title, _)| title == "File received")
                            .count();
                        if title == "File received" && received > 1 {
                            desktop::notify(hwnd, &format!("{received} files received"), message);
                        } else {
                            desktop::notify(hwnd, title, message);
                        }
                    }
                }
                request_frame(hwnd);
                return 0;
            }
            WM_SIZE if w == 1 && !state.is_null() && unsafe { (*state).tray } => {
                unsafe { ShowWindow(hwnd, 0) };
                return 0;
            }
            WM_DISPLAYCHANGE if !state.is_null() => {
                unsafe {
                    (*state).buffer = None;
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
                return 0;
            }
            WM_SIZE => {
                request_frame(hwnd);
                return 0;
            }
            WM_LBUTTONDOWN if !state.is_null() => {
                let mut rect = Rect::default();
                unsafe {
                    GetClientRect(hwnd, &mut rect);
                }
                let x = (l as u32 & 0xffff) as i16 as i32;
                let y = ((l as u32 >> 16) & 0xffff) as i16 as i32;
                if y < TITLE_HEIGHT {
                    if x >= rect.right - BUTTON {
                        close_window(hwnd, unsafe { (*state).tray });
                    } else if x >= rect.right - 2 * BUTTON {
                        unsafe {
                            ShowWindow(hwnd, 6);
                        }
                    } else if help_button_hit(rect.right, x) {
                        unsafe { desktop_char(hwnd, state, 'n') };
                        return 0;
                    } else if update_button_hit(unsafe { &*state }, rect.right, x) {
                        unsafe { desktop_char(hwnd, state, 'i') };
                        return 0;
                    } else if screen_button_hit(rect.right, x)
                        && !unsafe { (*state).app.modal_open() }
                    {
                        unsafe { desktop_char(hwnd, state, 'l') };
                        return 0;
                    }
                } else if unsafe { (*state).shortcut_help } {
                    if let Some(key) = help_key_at(unsafe { &*state }, x, y) {
                        unsafe { desktop_char(hwnd, state, key) };
                    }
                    return 0;
                } else {
                    let host = unsafe { &mut *state };
                    if !host.updates_open
                        && let Some((row, height)) = recipient_row_at(host, x, y)
                        && let Err(error) = host.app.click_recipient_row(row, height)
                    {
                        host.error = error.to_string();
                    }
                }
                request_frame(hwnd);
                return 0;
            }
            WM_MOUSEWHEEL if !state.is_null() => {
                if unsafe { (*state).shortcut_help || (*state).updates_open } {
                    return 0;
                }
                let up = ((w >> 16) as i16) > 0;
                let host = unsafe { &mut *state };
                let result = if host.app.modal_open() {
                    host.app
                        .handle_modal(if up { ModalKey::Up } else { ModalKey::Down })
                } else {
                    host.app.handle_key(if up { Key::Up } else { Key::Down })
                };
                if let Err(err) = result {
                    host.error = err.to_string();
                }
                request_frame(hwnd);
                return 0;
            }
            WM_KEYDOWN if !state.is_null() => {
                if unsafe { (*state).shortcut_help } {
                    if w == 0x1b {
                        unsafe { (*state).shortcut_help = false };
                        request_frame(hwnd);
                    }
                    return 0;
                }
                let host = unsafe { &mut *state };
                if host.updates_open {
                    if w == 0x1b {
                        host.updates_open = false;
                    } else if w == 0x0d
                        && l & (1 << 30) == 0
                        && let Err(error) = host.updates.open_release()
                    {
                        host.updates_open = false;
                        host.error = error.to_string();
                    }
                    request_frame(hwnd);
                    return 0;
                }
                if w == 0x0d && host.app.paste_stack_open() {
                    if l & (1 << 30) == 0
                        && let Some(item) = host.app.selected_paste()
                    {
                        match copy_paste_item(hwnd, &item) {
                            Ok(()) => host.app.finish_paste_copy(item.id),
                            Err(error) => host.app.paste_stack_error(error.to_string()),
                        }
                    }
                    request_frame(hwnd);
                    return 0;
                }
                if w == 0x0d && host.app.scratchpad_open() {
                    if let Some(text) = host.app.scratchpad_text() {
                        let status = desktop::copy_clipboard(hwnd, text)
                            .map(|()| "Copied to clipboard".into())
                            .unwrap_or_else(|error| error.to_string());
                        host.app.scratchpad_status(status);
                    }
                    request_frame(hwnd);
                    return 0;
                }
                let result = if host.app.modal_open() {
                    modal_special(w).map(|k| host.app.handle_modal(k))
                } else {
                    ordinary_special(w).map(|k| host.app.handle_key(k))
                };
                if let Some(Err(err)) = result {
                    host.error = err.to_string();
                }
                if host.app.quitting() {
                    unsafe {
                        DestroyWindow(hwnd);
                    }
                    return 0;
                }
                request_frame(hwnd);
                return 0;
            }
            WM_CHAR if !state.is_null() => {
                let host = unsafe { &mut *state };
                let pending = host.pending_surrogate.take();
                let unit = w as u16;
                let key = match unit {
                    0xd800..=0xdbff => {
                        host.pending_surrogate = Some(unit);
                        return 0;
                    }
                    0xdc00..=0xdfff => {
                        pending.and_then(|high| char::decode_utf16([high, unit]).next()?.ok())
                    }
                    _ => char::from_u32(w as u32),
                }
                .unwrap_or('\0');
                let help_repeat =
                    matches!(key, 'n' | 'N' | '?') && l & (1 << 30) != 0 && !host.app.modal_open();
                let picker_repeat = matches!(key, 'v' | 'V')
                    && l & (1 << 30) != 0
                    && (host.app.recipient_picker_open() || !host.app.modal_open());
                let scratchpad_repeat = matches!(key, 'k' | 'K')
                    && l & (1 << 30) != 0
                    && (host.app.scratchpad_open() || !host.app.modal_open());
                let screen_repeat =
                    matches!(key, 'l' | 'L') && l & (1 << 30) != 0 && !host.app.modal_open();
                if (!key.is_control() || key == ' ')
                    && !help_repeat
                    && !picker_repeat
                    && !scratchpad_repeat
                    && !screen_repeat
                {
                    unsafe { desktop_char(hwnd, state, key) };
                }
                return 0;
            }
            WM_CLOSE => {
                close_window(hwnd, !state.is_null() && unsafe { (*state).tray });
                return 0;
            }
            WM_DESTROY => {
                unsafe {
                    KillTimer(hwnd, 1);
                    if !state.is_null() {
                        desktop::remove_icon(hwnd);
                        if let Some(mut scanner) = (*state).scanner.take() {
                            let _ = scanner.kill();
                            let _ = scanner.wait();
                        }
                        stop_screen_viewer(&mut *state);
                    }
                    PostQuitMessage(0);
                }
                return 0;
            }
            WM_NCDESTROY if !state.is_null() => unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DeleteObject((*state).font);
                drop(Box::from_raw(state));
            },
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    pub(super) fn main() {
        let mut arguments = std::env::args_os().skip(1).peekable();
        if arguments.peek().is_some_and(|value| value == "--share") {
            arguments.next();
        }
        let files: Vec<_> = arguments
            .filter_map(|value| std::path::absolute(PathBuf::from(value)).ok())
            .collect();
        let _instance = match desktop::InstanceGuard::acquire() {
            Ok(Some(guard)) => guard,
            Ok(None) => {
                if let Err(error) = desktop::forward_to_instance(CLASS, &files) {
                    startup_error(&error.to_string());
                }
                return;
            }
            Err(error) => {
                startup_error(&error.to_string());
                return;
            }
        };
        unsafe {
            SetProcessDPIAware();
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let _ = std::env::set_current_dir(home);
        }
        let app = match App::new() {
            Ok(app) => app,
            Err(err) => {
                startup_error(&err.to_string());
                return;
            }
        };
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let name = wide(CLASS);
            let title = wide("TapSend Desktop");
            let face = wide("Consolas");
            let class = WindowClass {
                style: 0x0003,
                procedure: Some(window_proc),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: desktop::app_icon(),
                cursor: LoadCursorW(ptr::null_mut(), 32512 as *const u16),
                background: ptr::null_mut(),
                menu: ptr::null(),
                class_name: name.as_ptr(),
            };
            if RegisterClassW(&class) == 0 {
                return;
            }
            let hwnd = CreateWindowExW(
                WS_EX_APPWINDOW,
                name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX,
                120,
                70,
                1440,
                850,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            if hwnd.is_null() {
                return;
            }
            let font = CreateFontW(-17, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
            let dc = GetDC(hwnd);
            let previous = SelectObject(dc, font);
            let letter = ['M' as u16];
            let mut size = TextSize { cx: 9, cy: 18 };
            GetTextExtentPoint32W(dc, letter.as_ptr(), 1, &mut size);
            SelectObject(dc, previous);
            ReleaseDC(hwnd, dc);
            let background_path = desktop::background_setting().ok();
            let background = background_path.as_ref().is_some_and(|path| path.is_file());
            let tray = background && desktop::add_icon(hwnd);
            let mut host = Box::new(Host {
                app,
                font,
                cell: size.cx.max(7),
                line: size.cy.max(18) + 3,
                error: if background && !tray {
                    "Windows could not add the TapSend tray icon".into()
                } else {
                    String::new()
                },
                scanner: None,
                screen_viewer: None,
                screen_requested: None,
                screen_choosing_phone: false,
                background_path,
                background,
                tray,
                taskbar_created: desktop::RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                pending_selection: Vec::new(),
                shortcut_help: false,
                updates_open: false,
                updates: crate::updates::Updates::new(),
                pending_surrogate: None,
                frame: None,
                buffer: None,
                frame_dirty: true,
            });
            host.app.desktop_initialize();
            queue_selection(&mut host, files);
            prepare_selection(&mut host);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(host) as isize);
            SetTimer(hwnd, 1, 250, ptr::null());
            ShowWindow(hwnd, 1);
            UpdateWindow(hwnd);
            let mut msg = MaybeUninit::<Message>::zeroed();
            while GetMessageW(msg.as_mut_ptr(), ptr::null_mut(), 0, 0) > 0 {
                let event = msg.assume_init_ref();
                if event.hwnd == hwnd
                    && event.message == WM_KEYDOWN
                    && matches!(event.wparam, VK_H | VK_J | VK_R | VK_S | VK_V)
                {
                    let shift = GetKeyState(VK_SHIFT) < 0;
                    let control = GetKeyState(VK_CONTROL) < 0;
                    let search = file_search_chord(event.wparam, shift, control);
                    let paste = paste_stack_chord(event.wparam, shift, control);
                    let drop = event.wparam == VK_R && shift && !control;
                    let hold = event.wparam == VK_H && shift && !control;
                    let relay = event.wparam == VK_J && shift && !control;
                    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Host;
                    if (search || paste || drop || hold || relay)
                        && let Some(host) = state.as_mut()
                    {
                        let repeated = event.lparam & (1 << 30) != 0;
                        if !host.app.modal_open() && !host.updates_open {
                            host.shortcut_help = false;
                            host.error.clear();
                            if paste {
                                host.app.desktop_paste_stack();
                            } else if drop {
                                if !repeated && let Err(error) = host.app.desktop_folder_drop() {
                                    host.error = error.to_string();
                                }
                            } else if hold {
                                if !repeated && let Err(error) = host.app.desktop_hold_file() {
                                    host.error = error.to_string();
                                }
                            } else if relay {
                                if !repeated
                                    && let Err(error) = desktop::read_clipboard(hwnd)
                                        .and_then(|text| host.app.connect_relay_pass(text))
                                {
                                    host.error = error.to_string();
                                }
                            } else if let Err(error) = host.app.desktop_file_search() {
                                host.error = format!("Could not search this folder: {error}");
                            }
                            request_frame(hwnd);
                            continue;
                        }
                        if repeated
                            && ((search && host.app.file_search_open())
                                || (paste && host.app.paste_stack_open()))
                        {
                            continue;
                        }
                    }
                }
                TranslateMessage(msg.as_ptr());
                DispatchMessageW(msg.as_ptr());
            }
        }
    }

    fn startup_error(error: &str) {
        let caption = wide("TapSend Desktop");
        let message = wide(error);
        unsafe { MessageBoxW(ptr::null_mut(), message.as_ptr(), caption.as_ptr(), 0x10) };
    }
}
