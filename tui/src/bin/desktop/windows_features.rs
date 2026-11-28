use std::{
    ffi::{OsString, c_void},
    fs, io,
    mem::size_of,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
    ptr,
};

use super::native::{CREATE_NO_WINDOW, Handle, Hwnd, Point, wide};

pub(super) const TRAY_CALLBACK: u32 = 0x8001;
pub(super) const TRAY_OPEN: u32 = 1;
pub(super) const TRAY_RECEIVE: u32 = 2;
pub(super) const TRAY_BACKGROUND: u32 = 3;
pub(super) const TRAY_EXPLORER: u32 = 4;
pub(super) const TRAY_QUIT: u32 = 5;
pub(super) const TRAY_UPDATES: u32 = 6;
pub(super) const COPYDATA: u32 = 0x004a;
const LEGACY_COPY_TAG: usize = 0x5450_5344;
const PICKER_COPY_TAG: usize = 0x5450_5302;
// Older Explorer launchers can still hand files to this build.
const SEARCH_COPY_TAG: usize = 0x5450_5303;
const OUTBOX_COPY_TAG: usize = 0x5450_5304;
const PASTE_COPY_TAG: usize = 0x5450_5305;
const SCREEN_COPY_TAG: usize = 0x5450_5306;
const PAIRING_COPY_TAG: usize = 0x5450_5307;
const SCREEN_BUTTON_COPY_TAG: usize = 0x5450_5308;
const SMOOTH_SCREEN_COPY_TAG: usize = 0x5450_5309;
const COPY_TAG: usize = 0x5450_530a;
const MAX_SELECTION_BYTES: usize = 256 * 1024;
const NIF_MESSAGE: u32 = 1;
const NIF_ICON: u32 = 2;
const NIF_TIP: u32 = 4;
const NIF_INFO: u32 = 16;

#[repr(C)]
struct NotifyIcon {
    size: u32,
    window: Hwnd,
    id: u32,
    flags: u32,
    callback: u32,
    icon: Handle,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    timeout_or_version: u32,
    title: [u16; 64],
    info_flags: u32,
    guid: [u8; 16],
    balloon_icon: Handle,
}

#[repr(C)]
pub(super) struct CopyData {
    tag: usize,
    bytes: u32,
    data: *const c_void,
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn Shell_NotifyIconW(action: u32, data: *mut NotifyIcon) -> i32;
    fn SHGetFolderPathW(
        window: Hwnd,
        folder: i32,
        token: Handle,
        flags: u32,
        path: *mut u16,
    ) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub(super) fn RegisterWindowMessageW(message: *const u16) -> u32;
    pub(super) fn SetForegroundWindow(window: Hwnd) -> i32;
    pub(super) fn LoadIconW(instance: Handle, name: *const u16) -> Handle;
    fn LoadImageW(
        instance: Handle,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Handle;
    fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
    fn SendMessageTimeoutW(
        window: Hwnd,
        msg: u32,
        w: usize,
        l: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, flags: u32, id: usize, label: *const u16) -> i32;
    fn TrackPopupMenu(
        menu: Handle,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        window: Hwnd,
        rect: *const c_void,
    ) -> u32;
    fn DestroyMenu(menu: Handle) -> i32;
    fn PostMessageW(window: Hwnd, msg: u32, w: usize, l: isize) -> i32;
    fn OpenClipboard(window: Hwnd) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> Handle;
    fn SetClipboardData(format: u32, memory: Handle) -> Handle;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: Handle) -> i32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalSize(memory: Handle) -> usize;
}

const CF_UNICODETEXT: u32 = 13;

struct Clipboard;

impl Clipboard {
    fn open(window: Hwnd) -> io::Result<Self> {
        if unsafe { OpenClipboard(window) } == 0 {
            return Err(io::Error::other("Clipboard is busy. Try again."));
        }
        Ok(Self)
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

pub(super) fn read_clipboard(window: Hwnd) -> io::Result<String> {
    let _clipboard = Clipboard::open(window)?;
    let memory = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if memory.is_null() {
        return Err(io::Error::other("Copy some text first."));
    }
    let length = unsafe { GlobalSize(memory) } / size_of::<u16>();
    let pointer = unsafe { GlobalLock(memory) }.cast::<u16>();
    if pointer.is_null() {
        return Err(io::Error::last_os_error());
    }
    let units = unsafe { std::slice::from_raw_parts(pointer, length.min(2001)) };
    let result = units
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| io::Error::other("Clipboard text exceeds the scratchpad limit"))
        .and_then(|end| String::from_utf16(&units[..end]).map_err(io::Error::other));
    unsafe {
        GlobalUnlock(memory);
    }
    result
}

pub(super) fn copy_clipboard(window: Hwnd, text: &str) -> io::Result<()> {
    let bytes: Vec<_> = text
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    copy_clipboard_bytes(window, CF_UNICODETEXT, &bytes)
}

pub(super) fn copy_clipboard_image(window: Hwnd, bitmap: &[u8]) -> io::Result<()> {
    if !valid_paste_bitmap(bitmap) {
        return Err(io::Error::other(
            "The screenshot changed or could not be opened",
        ));
    }
    copy_clipboard_bytes(window, 8, bitmap)
}

pub(super) fn copy_clipboard_file(window: Hwnd, path: &std::path::Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    if !path.is_file() {
        return Err(io::Error::other(
            "The held file has moved or is no longer available",
        ));
    }
    let mut bytes = vec![0; 20];
    bytes[0] = 20; // DROPFILES.pFiles
    bytes[16] = 1; // DROPFILES.fWide
    for value in path.as_os_str().encode_wide().chain([0, 0]) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    copy_clipboard_bytes(window, 15, &bytes) // CF_HDROP
}

fn valid_paste_bitmap(bitmap: &[u8]) -> bool {
    if bitmap.len() < 40
        || bitmap[..4] != 40_u32.to_le_bytes()
        || bitmap[12..16] != [1, 0, 24, 0]
        || bitmap[16..20] != [0; 4]
    {
        return false;
    }
    let width = u32::from_le_bytes(bitmap[4..8].try_into().unwrap()) as u64;
    let height = u32::from_le_bytes(bitmap[8..12].try_into().unwrap()) as u64;
    width > 0
        && height > 0
        && width <= 12_000_000 / height
        && 40 + ((width * 3 + 3) & !3) * height == bitmap.len() as u64
}

fn copy_clipboard_bytes(window: Hwnd, format: u32, bytes: &[u8]) -> io::Result<()> {
    let _clipboard = Clipboard::open(window)?;
    let memory = unsafe { GlobalAlloc(2, bytes.len()) };
    if memory.is_null() {
        return Err(io::Error::last_os_error());
    }
    let target = unsafe { GlobalLock(memory) }.cast::<u8>();
    if target.is_null() {
        unsafe {
            GlobalFree(memory);
        }
        return Err(io::Error::last_os_error());
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        GlobalUnlock(memory);
        if EmptyClipboard() == 0 || SetClipboardData(format, memory).is_null() {
            let error = io::Error::last_os_error();
            GlobalFree(memory);
            return Err(error);
        }
    }
    Ok(())
}

pub(super) struct InstanceGuard(Handle);

impl InstanceGuard {
    pub(super) fn acquire() -> io::Result<Option<Self>> {
        let name = wide(r"Local\TapSendDesktopInstance");
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        let code = unsafe { GetLastError() };
        if handle.is_null() {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let guard = Self(handle);
        Ok((code != 183).then_some(guard))
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn forward_to_instance(class: &str, files: &[PathBuf]) -> io::Result<()> {
    let class = wide(class);
    let payload: Vec<u16> = files
        .iter()
        .flat_map(|path| path.as_os_str().encode_wide().chain([0]))
        .collect();
    let bytes = payload.len() * size_of::<u16>();
    if bytes > MAX_SELECTION_BYTES {
        return Err(io::Error::other(
            "File Explorer selected too many paths for one share",
        ));
    }
    let data = CopyData {
        tag: COPY_TAG,
        bytes: bytes as u32,
        data: payload.as_ptr().cast(),
    };
    for _ in 0..30 {
        let window = unsafe { FindWindowW(class.as_ptr(), ptr::null()) };
        if !window.is_null() {
            let mut accepted = 0;
            let sent = unsafe {
                SendMessageTimeoutW(
                    window,
                    COPYDATA,
                    0,
                    (&data as *const CopyData) as isize,
                    0x0002,
                    1500,
                    &mut accepted,
                )
            };
            return if sent != 0 && accepted != 0 {
                Ok(())
            } else if sent != 0 {
                Err(io::Error::other(
                    "An older TapSend is already running. Quit it from the Windows tray or press Q in its window, then open this build again.",
                ))
            } else {
                Err(io::Error::other(
                    "The open TapSend window could not accept these files",
                ))
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(io::Error::other(
        "TapSend is already starting. Try again in a moment.",
    ))
}

// WM_COPYDATA owns this buffer only until the window procedure returns.
pub(super) unsafe fn copy_selection(message: isize) -> Option<Vec<PathBuf>> {
    let data = unsafe { (message as *const CopyData).as_ref()? };
    let bytes = data.bytes as usize;
    if !matches!(
        data.tag,
        COPY_TAG
            | SMOOTH_SCREEN_COPY_TAG
            | SCREEN_BUTTON_COPY_TAG
            | PAIRING_COPY_TAG
            | SCREEN_COPY_TAG
            | PASTE_COPY_TAG
            | OUTBOX_COPY_TAG
            | SEARCH_COPY_TAG
            | PICKER_COPY_TAG
            | LEGACY_COPY_TAG
    ) || bytes > MAX_SELECTION_BYTES
        || bytes % 2 != 0
    {
        return None;
    }
    if bytes == 0 {
        return Some(Vec::new());
    }
    if data.data.is_null() || !(data.data as usize).is_multiple_of(size_of::<u16>()) {
        return None;
    }
    let payload = unsafe { std::slice::from_raw_parts(data.data.cast::<u16>(), bytes / 2) };
    decode_selection(payload)
}

fn decode_selection(payload: &[u16]) -> Option<Vec<PathBuf>> {
    if payload.last() != Some(&0) {
        return None;
    }
    let mut paths = Vec::new();
    for part in payload[..payload.len() - 1].split(|unit| *unit == 0) {
        let path = PathBuf::from(OsString::from_wide(part));
        if !path.is_absolute() {
            return None;
        }
        if !paths.contains(&path) {
            if paths.len() == 1024 {
                return None;
            }
            paths.push(path);
        }
    }
    Some(paths)
}

pub(super) fn app_icon() -> Handle {
    static ICON: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *ICON.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe() {
            let icon = exe.with_file_name("TapSend.ico");
            let path: Vec<u16> = icon.as_os_str().encode_wide().chain([0]).collect();
            let image = unsafe { LoadImageW(ptr::null_mut(), path.as_ptr(), 1, 0, 0, 0x10 | 0x40) };
            if !image.is_null() {
                return image as usize;
            }
        }
        unsafe { LoadIconW(ptr::null_mut(), 32512 as *const u16) as usize }
    }) as Handle
}

fn copy_text(target: &mut [u16], text: &str) {
    let limit = target.len() - 1;
    let mut at = 0;
    for ch in text.chars().filter(|ch| !ch.is_control()) {
        let mut units = [0; 2];
        let encoded = ch.encode_utf16(&mut units);
        if at + encoded.len() > limit {
            break;
        }
        target[at..at + encoded.len()].copy_from_slice(encoded);
        at += encoded.len();
    }
    target[at] = 0;
}

fn tray_data(window: Hwnd) -> NotifyIcon {
    let mut data: NotifyIcon = unsafe { std::mem::zeroed() };
    data.size = size_of::<NotifyIcon>() as u32;
    data.window = window;
    data.id = 1;
    data
}

pub(super) fn add_icon(window: Hwnd) -> bool {
    let mut data = tray_data(window);
    data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.callback = TRAY_CALLBACK;
    data.icon = app_icon();
    copy_text(&mut data.tip, "TapSend Desktop");
    unsafe { Shell_NotifyIconW(0, &mut data) != 0 }
}

pub(super) fn remove_icon(window: Hwnd) {
    unsafe { Shell_NotifyIconW(2, &mut tray_data(window)) };
}

pub(super) fn notify(window: Hwnd, title: &str, message: &str) {
    let mut data = tray_data(window);
    data.flags = NIF_INFO;
    data.info_flags = 1; // NIIF_INFO
    copy_text(&mut data.title, title);
    copy_text(&mut data.info, message);
    unsafe { Shell_NotifyIconW(1, &mut data) };
}

pub(super) fn settings_menu(window: Hwnd, tray: bool, background: bool, explorer: bool) -> u32 {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return 0;
    }
    let add = |id, text, checked| {
        let label = wide(text);
        unsafe {
            AppendMenuW(
                menu,
                if checked { 0x8 } else { 0 },
                id as usize,
                label.as_ptr(),
            )
        };
    };
    if tray {
        add(TRAY_OPEN, "Open TapSend", false);
        add(TRAY_RECEIVE, "Start receiving", false);
    }
    add(
        TRAY_BACKGROUND,
        "Receive in background and show notifications",
        background,
    );
    add(TRAY_EXPLORER, "File Explorer Send to shortcut", explorer);
    add(TRAY_UPDATES, "TapSend updates", false);
    if tray {
        add(TRAY_QUIT, "Quit TapSend", false);
    }
    let mut cursor = Point::default();
    let chosen = unsafe {
        GetCursorPos(&mut cursor);
        SetForegroundWindow(window);
        let chosen = TrackPopupMenu(menu, 0x0102, cursor.x, cursor.y, 0, window, ptr::null());
        DestroyMenu(menu);
        PostMessageW(window, 0, 0, 0);
        chosen
    };
    chosen
}

pub(super) fn background_setting() -> io::Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|path| {
            PathBuf::from(path)
                .join("TapSend")
                .join("desktop-background")
        })
        .ok_or_else(|| io::Error::other("Windows settings folder is unavailable"))
}

pub(super) fn save_background(path: &Path, enabled: bool) -> io::Result<()> {
    if enabled {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Invalid settings path"))?;
        fs::create_dir_all(parent)?;
        fs::write(path, b"enabled")
    } else {
        match fs::remove_file(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

pub(super) fn explorer_shortcut() -> io::Result<PathBuf> {
    let mut buffer = [0_u16; 260];
    let result =
        unsafe { SHGetFolderPathW(ptr::null_mut(), 9, ptr::null_mut(), 0, buffer.as_mut_ptr()) };
    if result < 0 {
        return Err(io::Error::other("Windows Send to folder is unavailable"));
    }
    let length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    Ok(PathBuf::from(OsString::from_wide(&buffer[..length])).join("TapSend Desktop.lnk"))
}

pub(super) fn set_explorer_shortcut(path: &Path, enabled: bool) -> io::Result<()> {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$shortcut = $env:TAPSEND_SHORTCUT_PATH
$target = $env:TAPSEND_SHORTCUT_TARGET
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
if (Test-Path -LiteralPath $shortcut) {
    if ($link.Arguments -ne '--share') { throw 'This shortcut is not managed by TapSend.' }
}
if ($env:TAPSEND_SHORTCUT_ENABLE -eq '1') {
    $link.TargetPath = $target
    $link.Arguments = '--share'
    $link.WorkingDirectory = [IO.Path]::GetDirectoryName($target)
    $link.Description = 'Review files in TapSend Desktop before sharing'
    $link.Save()
} elseif (Test-Path -LiteralPath $shortcut) {
    Remove-Item -LiteralPath $shortcut
}
"#;
    let windows = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::other("Windows folder is unavailable"))?;
    let powershell = PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let result = Command::new(powershell)
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .env("TAPSEND_SHORTCUT_PATH", path)
        .env("TAPSEND_SHORTCUT_TARGET", std::env::current_exe()?)
        .env("TAPSEND_SHORTCUT_ENABLE", if enabled { "1" } else { "0" })
        .output()?;
    if !result.status.success() {
        let error = String::from_utf8_lossy(&result.stderr);
        return Err(io::Error::other(
            error
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Could not update the Send to shortcut")
                .to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_image_requires_a_complete_windows_bitmap() {
        let mut dib = [0; 44];
        dib[0] = 40;
        dib[4] = 1;
        dib[8] = 1;
        dib[12] = 1;
        dib[14] = 24;
        assert!(valid_paste_bitmap(&dib));
        assert!(!valid_paste_bitmap(&dib[..43]));
        dib[16] = 1;
        assert!(!valid_paste_bitmap(&dib));
    }

    #[test]
    fn accepts_existing_explorer_shortcuts_with_a_distinct_launch_tag() {
        for tag in [
            LEGACY_COPY_TAG,
            PICKER_COPY_TAG,
            SEARCH_COPY_TAG,
            OUTBOX_COPY_TAG,
            PASTE_COPY_TAG,
            SCREEN_COPY_TAG,
            PAIRING_COPY_TAG,
            SCREEN_BUTTON_COPY_TAG,
            SMOOTH_SCREEN_COPY_TAG,
            COPY_TAG,
        ] {
            let message = CopyData {
                tag,
                bytes: 0,
                data: ptr::null(),
            };
            assert_eq!(
                unsafe { copy_selection(&message as *const CopyData as isize) },
                Some(Vec::new())
            );
        }
        assert_ne!(COPY_TAG, PASTE_COPY_TAG);
    }

    #[test]
    fn explorer_paths_preserve_spaces_unicode_and_deduplicate() {
        let file = r"C:\My files\日本語.png";
        let data: Vec<_> = [file, file, r"D:\notes.txt"]
            .into_iter()
            .flat_map(|path| path.encode_utf16().chain([0]))
            .collect();
        assert_eq!(
            decode_selection(&data),
            Some(vec![PathBuf::from(file), PathBuf::from(r"D:\notes.txt")])
        );
        assert!(decode_selection(&data[..data.len() - 1]).is_none());
        assert!(decode_selection(&"relative.txt\0".encode_utf16().collect::<Vec<_>>()).is_none());
    }

    #[test]
    fn notifications_truncate_without_splitting_surrogates() {
        let mut target = [0_u16; 4];
        copy_text(&mut target, "ab🌟");
        assert_eq!(target, [97, 98, 0, 0]);
    }
}
