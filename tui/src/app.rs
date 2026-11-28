use std::{env, io, path::PathBuf, process::Command};
#[cfg(windows)]
use std::{
    io::{BufRead, BufReader, Read},
    os::windows::process::CommandExt,
    sync::mpsc,
};

use crate::{
    explorer::Explorer,
    server::ServerProcess,
    terminal::{Key, ModalKey, RawTerminal},
    ui,
};

pub struct App {
    explorer: Explorer,
    server: Option<ServerProcess>,
    message: String,
    pairing_view: u8,
    auto_dashboard: bool,
    device_view: bool,
    page: usize,
    queue_cursor: usize,
    stopped: bool,
    overlay: Option<Overlay>,
    quit_requested: bool,
    last_join_request: String,
    #[cfg(windows)]
    phone_download: Option<mpsc::Receiver<(Overlay, String)>>,
    #[cfg(windows)]
    recipient: Option<(String, String)>,
    #[cfg(windows)]
    reminder_folder: Option<PathBuf>,
    #[cfg(windows)]
    relay_job: Option<mpsc::Receiver<Result<(String, bool), String>>>,
    #[cfg(windows)]
    relay_pass: Option<String>,
    #[cfg(windows)]
    desktop_setup_blocked: bool,
}

// The server prints Go %q strings for received names and the final on-disk path.
fn go_quoted(input: &str) -> Option<(String, &str)> {
    let rest = input.strip_prefix('"')?;
    let mut chars = rest.char_indices();
    let mut value = String::new();
    while let Some((index, ch)) = chars.next() {
        if ch == '"' {
            return Some((value, &rest[index + 1..]));
        }
        if ch != '\\' {
            value.push(ch);
            continue;
        }
        let (_, escape) = chars.next()?;
        match escape {
            '\\' => value.push('\\'),
            '"' => value.push('"'),
            'n' => value.push('\n'),
            'r' => value.push('\r'),
            't' => value.push('\t'),
            'x' | 'u' | 'U' => {
                let digits = match escape {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let mut hex = String::new();
                for _ in 0..digits {
                    hex.push(chars.next()?.1);
                }
                value.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    None
}

fn received_path(logs: &[String], name: &str) -> Option<PathBuf> {
    logs.iter().rev().find_map(|line| {
        let (_, body) = line.split_once("RECEIVED ")?;
        let (logged_name, rest) = go_quoted(body)?;
        if logged_name != name {
            return None;
        }
        let (_, path) = rest.split_once(" saved to ")?;
        let (path, _) = go_quoted(path)?;
        Some(PathBuf::from(path))
    })
}

#[cfg(windows)]
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
        || port.parse::<u16>().unwrap_or(0) == 0
    {
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

#[cfg(windows)]
fn hex_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(windows)]
pub(crate) fn hex_decode(value: &str) -> Option<String> {
    if value.len() % 2 != 0 {
        return None;
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

#[cfg(windows)]
pub(crate) fn open_local_file(path: &std::path::Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut std::ffi::c_void;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let open: Vec<u16> = "open\0".encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            open.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if (result as isize) > 32 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Windows open error {}",
            result as isize
        )))
    }
}

#[cfg(not(windows))]
fn open_local_file(path: &std::path::Path) -> io::Result<()> {
    Command::new("xdg-open").arg(path).spawn().map(|_| ())
}

#[cfg(test)]
mod received_tests {
    use super::*;
    #[test]
    fn finds_saved_path_even_with_escaped_windows_name() {
        let logs = vec![r#"2026 RECEIVED "report \"final\".pdf" (42 bytes) saved to "C:\\TapSend\\incoming-123-report \"final\".pdf""#.into()];
        assert_eq!(
            received_path(&logs, "report \"final\".pdf"),
            Some(PathBuf::from(
                "C:\\TapSend\\incoming-123-report \"final\".pdf"
            ))
        );
        assert_eq!(received_path(&logs, "other.pdf"), None);
    }
    #[cfg(windows)]
    #[test]
    fn camera_link_accepts_only_phone_share_on_lan() {
        let token = "a".repeat(48);
        assert!(phone_share_url(&format!(
            "http://192.168.1.5:8080/s/{token}/"
        )));
        assert!(!phone_share_url(&format!(
            "https://192.168.1.5:8080/s/{token}/"
        )));
        assert!(!phone_share_url(&format!("http://8.8.8.8:8080/s/{token}/")));
    }
    #[cfg(windows)]
    #[test]
    fn recipient_picker_selects_by_keyboard_and_by_scrolled_row() {
        let mut app = App::new().unwrap();
        let devices: Vec<_> = (0..12)
            .map(|index| crate::server::RecipientDevice {
                id: format!("id-{index}"),
                name: format!("Phone {index}"),
                pending: false,
            })
            .collect();
        app.overlay = Some(Overlay::Recipient {
            cursor: 0,
            files: Vec::new(),
            devices: Some(devices.clone()),
        });
        app.handle_modal(ModalKey::Char('V')).unwrap();
        assert!(app.modal_open());
        app.handle_modal(ModalKey::Enter).unwrap();
        assert_eq!(app.recipient.as_ref().unwrap().0, "id-1");
        assert!(!app.modal_open());

        app.overlay = Some(Overlay::Recipient {
            cursor: 11,
            files: Vec::new(),
            devices: Some(devices),
        });
        app.click_recipient_row(8, 20).unwrap();
        assert_eq!(app.recipient.as_ref().unwrap().0, "id-10");
        assert!(!app.modal_open());
    }

    #[cfg(windows)]
    #[test]
    fn file_search_marks_matching_files_and_keeps_them_in_the_send_queue() {
        let directory = env::temp_dir().join(format!(
            "tapsend-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let original = directory.join("Already marked.txt");
        let wanted = directory.join("Trip video.MP4");
        let subfolder = directory.join("Trip video folder");
        std::fs::write(&original, b"notes").unwrap();
        std::fs::write(&wanted, b"video").unwrap();
        std::fs::create_dir(&subfolder).unwrap();
        let mut app = App::new().unwrap();
        app.explorer = Explorer::new(directory.clone()).unwrap();
        app.explorer.toggle_entry(1);
        app.desktop_file_search().unwrap();
        for ch in "TRIP VIDEO".chars() {
            app.handle_modal(ModalKey::Char(ch)).unwrap();
        }
        let popup = app.render(92, 24, "").1.unwrap().3.join("\n");
        assert!(popup.contains("Trip video.MP4"));
        assert!(!popup.contains("Trip video folder"));
        app.handle_modal(ModalKey::Char('!')).unwrap();
        app.handle_modal(ModalKey::Enter).unwrap();
        assert_eq!(app.explorer.selected_count(), 1);
        app.handle_modal(ModalKey::Backspace).unwrap();
        app.handle_modal(ModalKey::Enter).unwrap();
        app.handle_modal(ModalKey::Escape).unwrap();
        assert!(!app.modal_open());
        assert_eq!(
            app.explorer.selected_files(),
            vec![original.clone(), wanted.clone()]
        );
        std::fs::remove_file(original).unwrap();
        std::fs::remove_file(wanted).unwrap();
        std::fs::remove_dir(subfolder).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn scratchpad_renders_and_selects_exact_text_for_copying() {
        let mut app = App::new().unwrap();
        app.overlay = Some(Overlay::Scratchpad {
            cursor: 0,
            entries: vec![
                crate::server::Snippet {
                    device: "Test phone".to_owned(),
                    from_phone: true,
                    text: "  first line\nsecond line\n".to_owned(),
                },
                crate::server::Snippet {
                    device: "Test phone".to_owned(),
                    from_phone: false,
                    text: "https://example.invalid".to_owned(),
                },
            ],
            status: String::new(),
        });
        assert!(
            app.render(92, 24, "")
                .1
                .unwrap()
                .3
                .join("\n")
                .contains("SCRATCHPAD")
        );
        assert_eq!(app.scratchpad_text(), Some("  first line\nsecond line\n"));
        app.handle_modal(ModalKey::Down).unwrap();
        assert_eq!(app.scratchpad_text(), Some("https://example.invalid"));
        app.handle_modal(ModalKey::Escape).unwrap();
        assert!(!app.scratchpad_open());
    }

    #[cfg(windows)]
    #[test]
    fn parses_phone_files_without_dropping_invalid_rows() {
        let list =
            parse_phone_list("0\t6E6F7465732E747874\t5\n1\t70686F746F2E706E67\t-1\n").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "notes.txt");
        assert_eq!(list[1].size, -1);
        assert!(parse_phone_list("0\t6E6F7465732E747874\t5\nbad row\n").is_err());
    }
}

enum Overlay {
    #[cfg(windows)]
    PasteStack {
        cursor: usize,
        status: String,
        waiting: Option<(String, &'static str)>,
    },
    #[cfg(windows)]
    Scratchpad {
        cursor: usize,
        entries: Vec<crate::server::Snippet>,
        status: String,
    },
    #[cfg(windows)]
    FileSearch {
        query: String,
        matches: Vec<usize>,
        cursor: usize,
    },
    #[cfg(windows)]
    Recipient {
        cursor: usize,
        files: Vec<PathBuf>,
        devices: Option<Vec<crate::server::RecipientDevice>>,
    },
    #[cfg(windows)]
    DeviceEdit {
        files: Vec<PathBuf>,
        task: DeviceTask,
    },
    #[cfg(windows)]
    DesktopShare {
        files: Vec<PathBuf>,
        scroll: usize,
    },
    #[cfg(windows)]
    PhoneDownload {
        name: String,
        index: usize,
        count: usize,
        done: u64,
        total: i64,
        existing: bool,
    },
    #[cfg(windows)]
    PhoneDownloadDone {
        lines: Vec<String>,
    },
    #[cfg(windows)]
    PhoneShare {
        url: String,
        files: Vec<PhoneFile>,
        cursor: usize,
        selected: Vec<bool>,
        stage: PhoneStage,
    },
    Help {
        scroll: usize,
    },
    Command {
        input: String,
        selected: usize,
    },
    Settings {
        values: [String; 9],
        selected: usize,
        edit: Option<String>,
        error: String,
        dirty: bool,
        confirm_exit: bool,
        first_run: bool,
    },
}

#[cfg(windows)]
enum DeviceTask {
    Rename {
        entry: crate::server::RecipientDevice,
        input: String,
    },
    Forget(crate::server::RecipientDevice),
    Clear(usize),
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum PhoneStage {
    Waiting,
    AskAll,
    Choose,
    Empty,
}

#[cfg(windows)]
#[derive(Clone)]
struct PhoneFile {
    id: usize,
    name: String,
    size: i64,
}

#[cfg(windows)]
fn parse_phone_list(text: &str) -> io::Result<Vec<PhoneFile>> {
    let mut files = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let id: usize = fields
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid phone file ID"))?;
        let name = fields
            .next()
            .and_then(hex_decode)
            .ok_or_else(|| io::Error::other("Invalid phone file name"))?;
        let size: i64 = fields
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid phone file size"))?;
        if id > 99999
            || size < -1
            || name.is_empty()
            || fields.next().is_some()
            || !ids.insert(id)
            || files.len() >= 1000
        {
            return Err(io::Error::other("Invalid phone file list"));
        }
        files.push(PhoneFile { id, name, size });
    }
    Ok(files)
}

const SETTING_KEYS: [&str; 9] = [
    "inbox",
    "start_folder",
    "show_hidden",
    "listen",
    "pairing_host",
    "direct_joins",
    "data_dir",
    "upload_limit_gib",
    "portal_enabled",
];

impl App {
    pub fn new() -> io::Result<Self> {
        let settings = Self::load_settings("show").ok();
        let start = settings
            .as_ref()
            .and_then(|values| (!values[1].is_empty()).then(|| PathBuf::from(&values[1])));
        let current = env::current_dir()?;
        let mut explorer = Explorer::new(start.unwrap_or_else(|| current.clone()))
            .or_else(|_| Explorer::new(current))?;
        if settings.as_ref().is_some_and(|values| values[2] == "true") {
            explorer.set_show_hidden(true)?;
        }
        Ok(Self {
            explorer,
            server: None,
            message: "Press S with no file to receive, or focus a file to share".into(),
            pairing_view: 0,
            auto_dashboard: false,
            device_view: false,
            page: 0,
            queue_cursor: 0,
            stopped: false,
            overlay: None,
            quit_requested: false,
            last_join_request: String::new(),
            #[cfg(windows)]
            phone_download: None,
            #[cfg(windows)]
            recipient: None,
            #[cfg(windows)]
            reminder_folder: None,
            #[cfg(windows)]
            relay_job: None,
            #[cfg(windows)]
            relay_pass: None,
            #[cfg(windows)]
            desktop_setup_blocked: false,
        })
    }

    pub fn run(mut self) -> io::Result<()> {
        let terminal = RawTerminal::enter()?;
        loop {
            if self.quit_requested {
                break;
            }
            self.tick();
            let overlay = self.overlay_view();
            ui::draw(
                &self.explorer,
                self.server.as_ref(),
                &self.message,
                self.pairing_view,
                self.stopped,
                self.device_view,
                self.page,
                self.queue_cursor,
                overlay,
            )?;
            if self.overlay.is_some() {
                self.handle_modal(terminal.read_modal_key()?)?;
                continue;
            }
            self.handle_key(terminal.read_key()?)?;
        }
        Ok(())
    }

    pub(crate) fn tick(&mut self) {
        #[cfg(windows)]
        if let Some(job) = &self.relay_job {
            match job.try_recv() {
                Ok(Ok((text, true))) => {
                    self.relay_pass = Some(text);
                    self.relay_job = None;
                }
                Ok(Ok((text, false))) => {
                    self.message = text;
                    self.relay_job = None;
                }
                Ok(Err(error)) => {
                    self.message = error;
                    self.relay_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.message = "Relay could not finish".into();
                    self.relay_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        #[cfg(windows)]
        if let Some(server) = self.server.as_mut() {
            let folder =
                (self.page == 1 && self.pairing_view == 0).then(|| self.explorer.directory());
            if folder != self.reminder_folder.as_deref() {
                if let Some(path) = folder {
                    let _ = server.reminder("paste-folder-opened", "", &path.to_string_lossy());
                }
                self.reminder_folder = folder.map(PathBuf::from);
            }
        }
        #[cfg(windows)]
        if let Some(server) = &self.server
            && let Some((name, pending)) = server.take_pair_notice()
        {
            if self.pairing_view != 0 {
                self.message = if pending {
                    format!("{name} needs approval · press V, select it, then A")
                } else {
                    format!("{name} connected · press V to select it")
                };
            }
            if let Some(Overlay::Recipient { devices, .. }) = &mut self.overlay {
                *devices = None;
            }
        }
        #[cfg(windows)]
        if let Some(server) = &self.server
            && let Some((action, outcome)) = server.take_device_result()
        {
            match outcome {
                Ok(()) => {
                    if let Some((id, _)) = &self.recipient
                        && let Some(devices) = server.recipients()
                    {
                        self.recipient = devices
                            .iter()
                            .find(|entry| &entry.id == id)
                            .map(|entry| (entry.id.clone(), entry.name.clone()));
                    }
                    self.message = match action.as_str() {
                        "rename" => "Device renamed",
                        "revoke" => "Device forgotten",
                        "clear-devices" => "All paired devices forgotten",
                        "approve" => "Phone approved",
                        "deny" => "Connection denied",
                        "folder-drop" => {
                            "Folder drop open for 10 minutes · confirm destination on phone"
                        }
                        "paste-hold-folder" | "hold-file-folder" => {
                            "Held until you return to this folder"
                        }
                        "paste-hold-device" | "hold-file-device" => {
                            "Held until the selected phone reconnects"
                        }
                        "paste-ready" => "Held items are ready · Shift+V opens Paste Stack",
                        _ => "Devices updated",
                    }
                    .into();
                }
                Err(error) => self.message = error,
            }
            if matches!(
                action.as_str(),
                "paste-hold-folder" | "paste-hold-device" | "paste-ready"
            ) && let Some(Overlay::PasteStack { cursor, status, .. }) = &mut self.overlay
            {
                *status = self.message.clone();
                *cursor = (*cursor).min(server.pastes().len().saturating_sub(1));
            }
        }
        #[cfg(windows)]
        if let Some(server) = &self.server
            && let Some(result) = server.take_paste_result()
            && let Some(Overlay::PasteStack {
                cursor,
                status,
                waiting,
            }) = &mut self.overlay
            && let Some((id, action)) = waiting.as_ref()
            && result.id == *id
            && result.action == *action
        {
            let copied = *action == "paste-copied";
            let id = result.id;
            *waiting = None;
            match result.outcome {
                Ok(()) => {
                    let items = server.pastes();
                    *cursor = if copied {
                        (*cursor).min(items.len().saturating_sub(1))
                    } else {
                        items.iter().position(|item| item.id == id).unwrap_or(0)
                    };
                    *status = if copied {
                        "Copied · Ctrl+V to paste".into()
                    } else {
                        "Order saved".into()
                    };
                }
                Err(error) => {
                    *status = if copied {
                        format!("On clipboard · queue update failed: {error}")
                    } else {
                        error
                    }
                }
            }
        }
        #[cfg(windows)]
        if let Some(server) = &self.server
            && let Some(result) = server.take_snippet_result()
        {
            let success = result.is_ok();
            let message = result
                .map(|()| "Snippet shared".to_owned())
                .unwrap_or_else(|error| format!("Could not share snippet: {error}"));
            if let Some(Overlay::Scratchpad {
                cursor,
                entries,
                status,
            }) = &mut self.overlay
            {
                *status = message;
                if success {
                    *entries = server.snippets();
                    *cursor = 0;
                }
            } else {
                self.message = message;
            }
        }
        #[cfg(windows)]
        if let Some(Overlay::Recipient {
            cursor, devices, ..
        }) = &mut self.overlay
            && devices.is_none()
            && let Some(snapshot) = self.server.as_ref().and_then(ServerProcess::recipients)
        {
            *cursor = self
                .recipient
                .as_ref()
                .and_then(|(selected, _)| snapshot.iter().position(|entry| &entry.id == selected))
                .unwrap_or((*cursor).min(snapshot.len().saturating_sub(1)));
            *devices = Some(snapshot);
        }
        #[cfg(windows)]
        if let Some(download) = &self.phone_download {
            loop {
                match download.try_recv() {
                    Ok((overlay, message)) => {
                        self.overlay = Some(overlay);
                        self.message = message;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.phone_download = None;
                        break;
                    }
                }
            }
        }
        if let Some(request) = self.server.as_ref().and_then(|server| {
            server
                .logs()
                .into_iter()
                .rev()
                .find(|line| line.contains("JOIN REQUEST "))
        }) {
            if request != self.last_join_request {
                self.last_join_request = request;
                self.message = if cfg!(windows) {
                    "Phone needs approval · press V, select it, then A"
                } else {
                    "New phone waiting. Open Devices, then ; approve ID or ; deny ID"
                }
                .into();
            }
        }
        if self.auto_dashboard
            && self.server.as_ref().is_some_and(|server| {
                server
                    .logs()
                    .iter()
                    .any(|line| line.contains("CONNECTED authenticated device"))
            })
        {
            self.pairing_view = 0;
            self.auto_dashboard = false;
            self.page = 0;
            self.message = "Phone connected. Browse and press A to add another file".into();
        }
    }

    fn overlay_view(&self) -> Option<ui::OverlayView<'_>> {
        match &self.overlay {
            #[cfg(windows)]
            Some(Overlay::PasteStack { cursor, status, .. }) => Some(ui::OverlayView::PasteStack {
                cursor: *cursor,
                items: self
                    .server
                    .as_ref()
                    .map(ServerProcess::pastes)
                    .unwrap_or_default(),
                status,
            }),
            #[cfg(windows)]
            Some(Overlay::Scratchpad {
                cursor,
                entries,
                status,
            }) => Some(ui::OverlayView::Scratchpad {
                cursor: *cursor,
                entries,
                status,
            }),
            #[cfg(windows)]
            Some(Overlay::FileSearch {
                query,
                matches,
                cursor,
            }) => Some(ui::OverlayView::FileSearch {
                explorer: &self.explorer,
                query,
                matches,
                cursor: *cursor,
            }),
            #[cfg(windows)]
            Some(Overlay::Recipient {
                cursor,
                files,
                devices,
            }) => Some(ui::OverlayView::Recipient {
                devices: devices.as_deref(),
                cursor: *cursor,
                sending: !files.is_empty(),
                status: &self.message,
            }),
            #[cfg(windows)]
            Some(Overlay::DeviceEdit { task, .. }) => {
                let (title, subtitle, input) = match task {
                    DeviceTask::Rename { entry, input } => (
                        "RENAME DEVICE",
                        format!("Current name · {}", entry.name),
                        Some(input.as_str()),
                    ),
                    DeviceTask::Forget(entry) => (
                        "FORGET DEVICE",
                        format!("{} will need to pair again.", entry.name),
                        None,
                    ),
                    DeviceTask::Clear(count) => (
                        "FORGET ALL DEVICES",
                        format!("{count} paired device(s) will need to pair again."),
                        None,
                    ),
                };
                Some(ui::OverlayView::DeviceEdit {
                    title,
                    subtitle,
                    input,
                })
            }
            #[cfg(windows)]
            Some(Overlay::DesktopShare { files, scroll }) => Some(ui::OverlayView::DesktopShare {
                files,
                scroll: *scroll,
            }),
            #[cfg(windows)]
            Some(Overlay::PhoneDownload {
                name,
                index,
                count,
                done,
                total,
                existing,
            }) => Some(ui::OverlayView::PhoneDownload {
                name,
                index: *index,
                count: *count,
                done: *done,
                total: *total,
                existing: *existing,
            }),
            #[cfg(windows)]
            Some(Overlay::PhoneDownloadDone { lines }) => {
                Some(ui::OverlayView::PhoneDownloadDone { lines })
            }
            #[cfg(windows)]
            Some(Overlay::PhoneShare {
                files,
                cursor,
                selected,
                stage,
                ..
            }) => Some(ui::OverlayView::PhoneShare {
                files: files
                    .iter()
                    .enumerate()
                    .map(|(i, file)| {
                        (
                            file.name.clone(),
                            file.size,
                            selected.get(i).copied().unwrap_or(false),
                        )
                    })
                    .collect(),
                cursor: *cursor,
                stage: match stage {
                    PhoneStage::Waiting => 0,
                    PhoneStage::AskAll => 1,
                    PhoneStage::Choose => 2,
                    PhoneStage::Empty => 3,
                },
            }),
            Some(Overlay::Help { scroll }) => Some(ui::OverlayView::Help { scroll: *scroll }),
            Some(Overlay::Command { input, selected }) => Some(ui::OverlayView::Command {
                input,
                selected: *selected,
                suggestions: ui::command_suggestions(input, self.server.is_some()),
            }),
            Some(Overlay::Settings {
                values,
                selected,
                edit,
                error,
                dirty,
                confirm_exit,
                first_run,
            }) => Some(ui::OverlayView::Settings {
                values,
                selected: *selected,
                edit: edit.as_deref(),
                error,
                dirty: *dirty,
                confirm_exit: *confirm_exit,
                first_run: *first_run,
            }),
            None => None,
        }
    }

    #[cfg(windows)]
    pub(crate) fn render(
        &self,
        width: usize,
        height: usize,
        desktop_status: &str,
    ) -> (Vec<String>, Option<(usize, usize, usize, Vec<String>)>) {
        let rows = ui::desktop_workspace(
            &self.explorer,
            self.server.as_ref(),
            if desktop_status.is_empty() {
                &self.message
            } else {
                desktop_status
            },
            self.pairing_view,
            self.stopped,
            self.page,
            self.queue_cursor,
            width,
            height,
            self.recipient.as_ref().map(|(_, name)| name.as_str()),
        );
        let overlay = self
            .overlay_view()
            .map(|view| ui::overlay_popup(view, width, height))
            .or_else(|| {
                (self.message == "Starting server…")
                    .then(|| ui::desktop_startup_popup(width, height))
            });
        (rows, overlay)
    }

    pub(crate) fn modal_open(&self) -> bool {
        self.overlay.is_some()
    }
    pub(crate) fn quitting(&self) -> bool {
        self.quit_requested
    }

    #[cfg(windows)]
    pub(crate) fn desktop_initialize(&mut self) {
        match Self::settings_command()
            .args(["settings", "state"])
            .output()
        {
            Ok(result) if result.status.success() && result.stdout.starts_with(b"configured") => {
                self.desktop_receive();
            }
            Ok(result) if result.status.success() && result.stdout.starts_with(b"missing") => {
                self.desktop_setup_blocked = true;
                self.open_settings_panel(true);
            }
            Ok(result) => {
                self.desktop_setup_blocked = true;
                let detail = String::from_utf8_lossy(&result.stderr);
                self.message = format!(
                    "Could not check setup: {} · Use the complete TapSend bundle",
                    detail.trim()
                );
            }
            Err(error) => {
                self.desktop_setup_blocked = true;
                self.message = format!(
                    "Could not check setup: {error} · Keep tapsend.exe beside TapSend Desktop"
                );
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_receive(&mut self) {
        if self.server.is_none() && !self.desktop_setup_blocked {
            self.message = "Starting server…".into();
            self.pairing_view = 0;
            self.auto_dashboard = false;
            self.stopped = false;
            match ServerProcess::start(&[]) {
                Ok(server) => self.server = Some(server),
                Err(error) => self.message = format!("Could not start server: {error}  [R] Retry"),
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_receive_key(&mut self) -> io::Result<()> {
        if self.server.is_none() {
            self.desktop_receive();
            Ok(())
        } else {
            self.handle_key(Key::Receive)
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_share_selected(&mut self) {
        if self.page != 1 || self.pairing_view != 0 {
            if self.server.is_none() {
                self.desktop_receive();
            }
            return;
        }
        let files = self.explorer.selected_files();
        if files.is_empty() {
            self.message = "Mark files with Space, then press S to share".into();
        } else {
            self.share_desktop_files(&files);
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_file_search(&mut self) -> io::Result<()> {
        self.explorer.refresh()?;
        self.page = 1;
        self.pairing_view = 0;
        self.overlay = Some(Overlay::FileSearch {
            query: String::new(),
            matches: self.explorer.file_matches(""),
            cursor: 0,
        });
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn file_search_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::FileSearch { .. }))
    }

    #[cfg(windows)]
    pub(crate) fn desktop_paste_stack(&mut self) {
        self.overlay = Some(Overlay::PasteStack {
            cursor: 0,
            status: String::new(),
            waiting: None,
        });
    }

    #[cfg(windows)]
    pub(crate) fn desktop_folder_drop(&mut self) -> io::Result<()> {
        if self.page != 1 || self.pairing_view != 0 {
            return Err(io::Error::other(
                "Open Files and navigate to the drop folder first",
            ));
        }
        let (peer, _) = self.recipient.as_ref().ok_or_else(|| {
            io::Error::other("Press V and choose the phone allowed to use this folder")
        })?;
        let server = self
            .server
            .as_mut()
            .ok_or_else(|| io::Error::other("Server is offline"))?;
        server.set_folder_drop(peer, &self.explorer.directory().to_string_lossy())?;
        self.message = format!(
            "Opening drop folder: {}",
            self.explorer.directory().display()
        );
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn desktop_hold_file(&mut self) -> io::Result<()> {
        if self.page != 1 || self.pairing_view != 0 {
            return Err(io::Error::other("Open Files and focus a file to hold"));
        }
        let file = self
            .explorer
            .entries()
            .get(self.explorer.cursor())
            .filter(|item| !item.directory)
            .ok_or_else(|| io::Error::other("Focus a file to hold"))?;
        let server = self
            .server
            .as_mut()
            .ok_or_else(|| io::Error::other("Server is offline"))?;
        server.reminder(
            "hold-file-folder",
            &self.explorer.directory().to_string_lossy(),
            &file.path.to_string_lossy(),
        )
    }

    #[cfg(windows)]
    pub(crate) fn connect_relay_pass(&mut self, pass: String) -> io::Result<()> {
        self.start_relay_job(vec!["connect".into()], Some(pass))
    }

    #[cfg(windows)]
    pub(crate) fn take_relay_pass(&mut self) -> Option<String> {
        self.relay_pass.take()
    }

    #[cfg(windows)]
    fn start_relay_job(&mut self, args: Vec<String>, pass: Option<String>) -> io::Result<()> {
        use std::{io::Write, process::Stdio};
        if self.relay_job.is_some() {
            return Err(io::Error::other("A relay operation is still running"));
        }
        let command_name = args[0].clone();
        let (done, status) = mpsc::channel();
        self.relay_job = Some(status);
        self.message = if command_name == "send" {
            "Encrypting and staging the file on the relay…".into()
        } else {
            "Checking relay…".into()
        };
        std::thread::spawn(move || {
            let result = (|| -> Result<(String, bool), String> {
                let mut child = Self::settings_command()
                    .arg("relay")
                    .args(args)
                    .stdin(if pass.is_some() {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    })
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|error| error.to_string())?;
                if let Some(pass) = pass {
                    let mut stdin = child.stdin.take().ok_or("Could not open relay input")?;
                    stdin
                        .write_all(pass.trim().as_bytes())
                        .map_err(|error| error.to_string())?;
                }
                let output = child
                    .wait_with_output()
                    .map_err(|error| error.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
                }
                Ok((
                    String::from_utf8_lossy(&output.stdout).trim().to_owned(),
                    command_name == "create",
                ))
            })();
            let _ = done.send(result);
        });
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn request_screen(&mut self) -> io::Result<()> {
        let (peer, _) = self.recipient.as_ref().ok_or_else(|| {
            io::Error::other("Press V and select the phone to view, then press L")
        })?;
        self.server
            .as_mut()
            .filter(|server| server.ready())
            .ok_or_else(|| io::Error::other("TapSend needs to be online before opening a viewer"))?
            .request_screen(peer)
    }

    #[cfg(windows)]
    pub(crate) fn has_recipient(&self) -> bool {
        self.recipient.is_some()
    }

    #[cfg(windows)]
    pub(crate) fn pick_screen_phone(&mut self) -> bool {
        self.pick_recipient(Vec::new());
        if self.recipient_picker_open() {
            self.message = "Choose the phone whose screen you want to view.".into();
            true
        } else {
            false
        }
    }

    #[cfg(windows)]
    pub(crate) fn take_screen_ticket(&self) -> Option<Result<crate::server::ScreenTicket, String>> {
        self.server.as_ref()?.take_screen_ticket()
    }

    #[cfg(windows)]
    pub(crate) fn close_screen(&mut self) {
        if let Some(server) = self.server.as_mut() {
            server.close_screen();
        }
    }

    #[cfg(windows)]
    pub(crate) fn paste_stack_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::PasteStack { .. }))
    }

    #[cfg(windows)]
    pub(crate) fn selected_paste(&self) -> Option<crate::server::PasteItem> {
        let Some(Overlay::PasteStack {
            cursor,
            waiting: None,
            ..
        }) = &self.overlay
        else {
            return None;
        };
        self.server.as_ref()?.pastes().get(*cursor).cloned()
    }

    #[cfg(windows)]
    pub(crate) fn paste_stack_error(&mut self, error: String) {
        if let Some(Overlay::PasteStack { status, .. }) = &mut self.overlay {
            *status = error;
        }
    }

    #[cfg(windows)]
    pub(crate) fn finish_paste_copy(&mut self, id: String) {
        let Some(server) = self.server.as_mut() else {
            return;
        };
        match server.change_paste(&id, "paste-copied") {
            Ok(()) => {
                if let Some(Overlay::PasteStack {
                    waiting, status, ..
                }) = &mut self.overlay
                {
                    *waiting = Some((id, "paste-copied"));
                    *status = "Copied · Ctrl+V to paste".into();
                }
            }
            Err(error) => {
                self.paste_stack_error(format!("On clipboard · queue update failed: {error}"))
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_scratchpad(&mut self) {
        let entries = self
            .server
            .as_ref()
            .map(ServerProcess::snippets)
            .unwrap_or_default();
        let status = self
            .recipient
            .as_ref()
            .map(|(_, name)| format!("Clipboard target · {name}"))
            .unwrap_or_else(|| "Choose a device with V before sharing clipboard text.".into());
        self.overlay = Some(Overlay::Scratchpad {
            cursor: 0,
            entries,
            status,
        });
    }

    #[cfg(windows)]
    pub(crate) fn scratchpad_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Scratchpad { .. }))
    }

    #[cfg(windows)]
    pub(crate) fn scratchpad_text(&self) -> Option<&str> {
        let Some(Overlay::Scratchpad {
            cursor, entries, ..
        }) = &self.overlay
        else {
            return None;
        };
        entries.get(*cursor).map(|entry| entry.text.as_str())
    }

    #[cfg(windows)]
    pub(crate) fn scratchpad_status(&mut self, value: String) {
        if let Some(Overlay::Scratchpad { status, .. }) = &mut self.overlay {
            *status = value;
        }
    }

    #[cfg(windows)]
    pub(crate) fn share_clipboard(&mut self, text: &str) -> io::Result<()> {
        let (id, name) = self
            .recipient
            .as_ref()
            .ok_or_else(|| io::Error::other("Press Esc, then V to choose a device first"))?;
        if text.trim().is_empty() || text.len() > 2000 {
            return Err(io::Error::other("Clipboard needs 1–2000 bytes of text"));
        }
        let server = self
            .server
            .as_mut()
            .filter(|server| server.ready())
            .ok_or_else(|| io::Error::other("TapSend needs to be online before sharing text"))?;
        server.send_snippet(id, text)?;
        self.scratchpad_status(format!("Sharing with {name}…"));
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn desktop_devices(&mut self) -> io::Result<()> {
        self.pick_recipient(Vec::new());
        Ok(())
    }

    #[cfg(windows)]
    fn pick_recipient(&mut self, files: Vec<PathBuf>) {
        self.desktop_receive();
        if let Some(server) = self.server.as_mut() {
            if let Err(error) = server.request_recipients() {
                self.message = format!("Could not load devices: {error}");
            } else {
                self.overlay = Some(Overlay::Recipient {
                    cursor: 0,
                    files,
                    devices: None,
                });
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn click_recipient_row(&mut self, row: usize, height: usize) -> io::Result<()> {
        let Some(Overlay::Recipient {
            cursor,
            devices: Some(devices),
            ..
        }) = &mut self.overlay
        else {
            return Ok(());
        };
        let Some(index) = ui::recipient_rows(devices.len(), *cursor, height).nth(row) else {
            return Ok(());
        };
        *cursor = index;
        self.handle_modal(ModalKey::Enter)
    }

    #[cfg(windows)]
    pub(crate) fn recipient_picker_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Recipient { .. }))
    }

    #[cfg(windows)]
    pub(crate) fn desktop_pair(&mut self) -> io::Result<()> {
        if self.pairing_view != 0 {
            self.pairing_view = 0;
            self.message = "Pairing closed".into();
            return Ok(());
        }
        self.desktop_receive();
        self.desktop_refresh_pairing();
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn desktop_refresh_pairing(&mut self) {
        if let Some(server) = self.server.as_mut() {
            self.pairing_view = 1;
            self.auto_dashboard = false;
            self.message = match server.refresh_pairing_pass() {
                Ok(()) => "On Android, open TapSend → Tools → Connect a PC".into(),
                Err(error) => error.to_string(),
            };
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_pairing_details(&mut self) -> bool {
        if self.pairing_view == 0 {
            return false;
        }
        self.pairing_view = if self.pairing_view == 1 { 2 } else { 1 };
        true
    }

    #[cfg(windows)]
    pub(crate) fn desktop_pairing_pass(&self) -> Option<io::Result<String>> {
        if self.pairing_view == 0 {
            return None;
        }
        Some(
            self.server
                .as_ref()
                .and_then(ServerProcess::pairing_pass)
                .ok_or_else(|| {
                    io::Error::other(
                        "Pairing pass unavailable. Press U to refresh, or scan the QR.",
                    )
                }),
        )
    }

    #[cfg(windows)]
    fn update_device(
        &mut self,
        action: &str,
        id: &str,
        name: &str,
        files: Vec<PathBuf>,
        cursor: usize,
    ) {
        if let Some(server) = self.server.as_mut() {
            self.message = match server.update_device(action, id, name) {
                Ok(()) => "Updating paired devices…".into(),
                Err(error) => error.to_string(),
            };
            self.overlay = Some(Overlay::Recipient {
                cursor,
                files,
                devices: None,
            });
        }
    }

    #[cfg(windows)]
    pub(crate) fn desktop_notices(&mut self) -> Vec<(String, String)> {
        let Some(server) = self.server.as_mut() else {
            return Vec::new();
        };
        let published = server.acknowledge_shares();
        if !published.is_empty() && self.message.starts_with("Publishing ") {
            self.message = "Offer queued · it appears when the phone connects".into();
        }
        for path in published {
            self.explorer.unselect(&path);
        }
        if server.ready() && self.message == "Starting server…" {
            self.message = "Ready to receive".into();
        }
        let mut notices: Vec<_> = server
            .drain_notices()
            .into_iter()
            .filter_map(|line| {
                if let Some(body) = line.strip_prefix("RECEIVED ") {
                    let (name, _) = go_quoted(body)?;
                    return Some(("File received".into(), name));
                }
                if line.starts_with("TUI_JOB ") {
                    let (_, name) = line.split_once(" name=")?;
                    return Some(("Receiving file".into(), hex_decode(name)?));
                }
                line.strip_prefix("JOIN REQUEST ")?;
                Some((
                    "Device requests access".into(),
                    "Open TapSend, press V, select the phone, then A.".into(),
                ))
            })
            .collect();
        if server.exited() {
            let logs = server.logs();
            let error = logs
                .last()
                .and_then(|line| line.splitn(3, ' ').nth(2))
                .unwrap_or("Receive session stopped");
            self.message = format!("[R] Retry  {error}");
            self.server = None;
            self.pairing_view = 0;
            self.auto_dashboard = false;
            notices.push((
                "Receive session stopped".into(),
                "Open TapSend to check the receive session.".into(),
            ));
        }
        notices
    }

    #[cfg(windows)]
    pub(crate) fn can_review_desktop_files(&self) -> bool {
        matches!(self.overlay, None | Some(Overlay::DesktopShare { .. }))
    }

    #[cfg(windows)]
    pub(crate) fn updates_allowed(&self) -> bool {
        !self.desktop_setup_blocked
    }

    #[cfg(windows)]
    pub(crate) fn review_desktop_files(&mut self, files: Vec<PathBuf>) {
        let files: Vec<_> = files.into_iter().filter(|path| path.is_file()).collect();
        if files.is_empty() {
            self.message = "Select one or more files in File Explorer".into();
        } else if let Some(Overlay::DesktopShare { files: pending, .. }) = self.overlay.as_mut() {
            for path in files {
                if !pending.contains(&path) {
                    pending.push(path);
                }
            }
        } else {
            self.overlay = Some(Overlay::DesktopShare { files, scroll: 0 });
        }
    }

    #[cfg(windows)]
    fn share_desktop_files(&mut self, files: &[PathBuf]) {
        let Some((id, name)) = self.recipient.clone() else {
            self.message = "Choose a device with V before sending".into();
            self.pick_recipient(Vec::new());
            return;
        };
        if self.server.is_none() {
            self.desktop_receive();
        }
        let Some(server) = self.server.as_mut() else {
            return;
        };
        if !server.ready() {
            if self.message != "Starting server…" {
                self.message = "Server is starting. Press S when ready to share.".into();
            }
            return;
        }
        if server
            .recipients()
            .is_some_and(|devices| !devices.iter().any(|entry| entry.id == id))
        {
            self.recipient = None;
            self.message = "Device is no longer paired. Press V to choose another.".into();
            return;
        }
        self.message = match server.offer(files, &id, &name) {
            Ok(0) => format!("These files are already offered to {name}"),
            Ok(count) => format!("Publishing {count} file(s) to {name}"),
            Err(error) => format!("Could not send to {name}: {error}"),
        };
        self.page = 1;
        self.pairing_view = 0;
    }

    pub(crate) fn handle_key(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::View(page) => {
                self.page = page;
                self.device_view = page == 2;
                self.pairing_view = 0;
                if page == 2 {
                    if let Some(server) = self.server.as_mut() {
                        let _ = server.control("devices");
                    }
                }
            }
            Key::NextView => {
                self.page = (self.page + 1) % 5;
                self.device_view = self.page == 2;
                self.pairing_view = 0;
                if self.page == 2 {
                    if let Some(server) = self.server.as_mut() {
                        let _ = server.control("devices");
                    }
                }
            }
            Key::Receive => {
                if self.server.is_none() {
                    self.start_receiving();
                } else {
                    self.page = 3;
                    self.pairing_view = 0;
                }
            }
            Key::Compose => {
                self.page = 4;
                self.pairing_view = 0;
                self.overlay = Some(Overlay::Command {
                    input: "message ".into(),
                    selected: 0,
                });
            }
            Key::Start if self.server.is_none() && self.page != 1 => self.start_receiving(),
            Key::Quit => self.quit_requested = true,
            #[cfg(windows)]
            Key::ScanCamera => self.scan_phone_qr(),
            Key::Up if self.page == 1 => self.explorer.up(),
            Key::Down if self.page == 1 => self.explorer.down(),
            Key::Up if self.page == 0 && self.pairing_view == 0 => {
                self.queue_cursor = self.queue_cursor.saturating_sub(1);
            }
            Key::Down if self.page == 0 && self.pairing_view == 0 => {
                let count = self
                    .server
                    .as_ref()
                    .map(|s| ui::transfer_count(&s.logs()))
                    .unwrap_or(0);
                self.queue_cursor = (self.queue_cursor + 1).min(count.saturating_sub(1));
            }
            Key::Space if self.page == 0 && self.pairing_view == 0 => self.open_received(),
            Key::Enter if self.page == 1 => {
                let result = self.explorer.open();
                self.update(result);
            }
            Key::Back if self.page == 1 => {
                let result = self.explorer.parent();
                self.update(result);
            }
            Key::Space if self.page == 1 => self.explorer.toggle(),
            Key::Hidden if self.page == 1 => {
                let result = self.explorer.toggle_hidden();
                self.update(result);
            }
            Key::Start if self.server.is_none() && !self.device_view => self.start_server(),
            Key::Add if self.server.is_some() && self.page == 1 => self.add_files(),
            Key::ToggleTransfers if self.server.is_some() => self.toggle_transfers(),
            Key::RefreshPairing if self.server.is_some() => {
                if let Some(server) = self.server.as_mut() {
                    self.message = match server.control("pairing") {
                        Ok(()) => "New pairing code requested; old code is now invalid".into(),
                        Err(error) => error.to_string(),
                    };
                    self.pairing_view = 1;
                }
            }
            Key::PairInfo if self.server.is_some() => {
                self.pairing_view = (self.pairing_view + 1) % 3;
                self.auto_dashboard = false;
            }
            Key::Files => {
                self.device_view = false;
                self.page = 1;
                self.pairing_view = 0;
            }
            Key::Devices => {
                self.page = 2;
                self.pairing_view = 0;
                self.device_view = true;
                if let Some(server) = self.server.as_mut() {
                    if let Err(error) = server.control("devices") {
                        self.message = error.to_string();
                    }
                }
            }
            Key::Command => {
                self.overlay = Some(Overlay::Command {
                    input: String::new(),
                    selected: 0,
                })
            }
            Key::Help => self.overlay = Some(Overlay::Help { scroll: 0 }),
            Key::Settings => self.open_settings(),
            _ => {}
        }
        Ok(())
    }

    fn open_received(&mut self) {
        let Some(server) = &self.server else {
            self.message = "No received files yet".into();
            return;
        };
        let logs = server.logs();
        let Some(name) = ui::selected_received_name(&logs, self.queue_cursor) else {
            self.message = "Select a completed received file (RX), then press Space".into();
            return;
        };
        let Some(path) = received_path(&logs, &name) else {
            self.message = "Saved path is no longer in the recent logs; check your inbox".into();
            return;
        };
        if !path.is_file() {
            self.message = format!("Received file is no longer at {}", path.display());
            return;
        }
        self.message = match open_local_file(&path) {
            Ok(()) => format!("Opened {}", name),
            Err(error) => format!("Could not open {}: {error}", name),
        };
    }

    #[cfg(windows)]
    fn scan_phone_qr(&mut self) {
        let scanner = env::current_exe()
            .ok()
            .map(|p| p.with_file_name("TapSend.CameraScanner.exe"));
        let Some(scanner) = scanner.filter(|p| p.is_file()) else {
            self.message = "Camera scanner missing; keep the extracted files together".into();
            return;
        };
        self.message = "Camera open; point at a TapSend phone-share QR (Esc cancels)".into();
        match Command::new(scanner).creation_flags(0x08000000).output() {
            Ok(output) if output.status.success() => {
                let url = String::from_utf8_lossy(&output.stdout);
                if phone_share_url(url.trim()) {
                    self.load_phone_share(url.trim().into());
                } else {
                    self.message = "QR is not a TapSend phone-share link".into();
                }
            }
            Ok(output) if output.status.code() == Some(1) => {
                self.message = "Camera scan cancelled".into()
            }
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr);
                self.message = if detail.trim().is_empty() {
                    "Camera unavailable or QR not recognized".into()
                } else {
                    detail.trim().chars().take(120).collect()
                };
            }
            Err(error) => self.message = format!("Could not start camera: {error}"),
        }
    }

    #[cfg(windows)]
    pub(crate) fn load_phone_share(&mut self, url: String) {
        let Some(scanner) = env::current_exe()
            .ok()
            .map(|p| p.with_file_name("TapSend.CameraScanner.exe"))
        else {
            return;
        };
        match Command::new(scanner)
            .creation_flags(0x08000000)
            .args(["--list", &url])
            .output()
        {
            Ok(output) if output.status.success() => {
                let files = match parse_phone_list(&String::from_utf8_lossy(&output.stdout)) {
                    Ok(files) => files,
                    Err(error) => {
                        self.message = error.to_string();
                        return;
                    }
                };
                match files.len() {
                    0 => {
                        self.overlay = Some(Overlay::PhoneShare {
                            url,
                            files,
                            selected: vec![],
                            cursor: 0,
                            stage: PhoneStage::Empty,
                        });
                        self.message =
                            "No files yet. Add files on the phone, then press R to refresh".into();
                    }
                    1 => self.download_phone_files(&url, &files),
                    _ => {
                        self.overlay = Some(Overlay::PhoneShare {
                            url,
                            selected: vec![false; files.len()],
                            files,
                            cursor: 0,
                            stage: PhoneStage::AskAll,
                        })
                    }
                }
            }
            Ok(output) if output.status.code() == Some(3) => {
                self.overlay = Some(Overlay::PhoneShare {
                    url,
                    files: vec![],
                    cursor: 0,
                    selected: vec![],
                    stage: PhoneStage::Waiting,
                });
                self.message =
                    "Waiting for approval on the phone; press R to retry without rescanning".into();
            }
            Ok(output) => {
                self.message = format!(
                    "Couldn't reach the phone share. Use the same network and scan a fresh phone QR. {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )
            }
            Err(error) => self.message = format!("Could not contact phone: {error}"),
        }
    }

    #[cfg(windows)]
    fn download_phone_files(&mut self, url: &str, files: &[PhoneFile]) {
        let Some(first) = files.first() else { return };
        let (sender, receiver) = mpsc::channel();
        let (overlay, message) = Self::phone_progress(first, 1, files.len(), 0, false);
        self.overlay = Some(overlay);
        self.message = message;
        self.phone_download = Some(receiver);
        let url = url.to_owned();
        let files = files.to_vec();
        std::thread::spawn(move || {
            Self::run_phone_download(&url, &files, |update| sender.send(update).is_ok());
        });
    }

    #[cfg(windows)]
    fn run_phone_download(
        url: &str,
        files: &[PhoneFile],
        report: impl Fn((Overlay, String)) -> bool,
    ) {
        let inbox = Self::load_settings("show")
            .ok()
            .map(|values| values[0].clone())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| {
                env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("TapSend Downloads")
                    .to_string_lossy()
                    .into_owned()
            });
        let Some(scanner) = env::current_exe()
            .ok()
            .map(|p| p.with_file_name("TapSend.CameraScanner.exe"))
        else {
            report(Self::phone_download_result(
                &[],
                &[],
                files.len(),
                Some("Camera scanner missing"),
            ));
            return;
        };
        let mut saved = Vec::new();
        let mut duplicates = Vec::new();
        for (index, file) in files.iter().enumerate() {
            if !report(Self::phone_progress(file, index + 1, files.len(), 0, false)) {
                return;
            }
            let child = Command::new(&scanner)
                .creation_flags(0x08000000)
                .args([
                    "--download",
                    url,
                    &file.id.to_string(),
                    &hex_encode(&file.name),
                    &file.size.to_string(),
                    &inbox,
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();
            let mut child = match child {
                Ok(child) => child,
                Err(error) => {
                    report(Self::phone_download_result(
                        &saved,
                        &duplicates,
                        files.len(),
                        Some(&error.to_string()),
                    ));
                    return;
                }
            };
            let mut existing = false;
            let mut renamed_from = None;
            let mut error_lines = Vec::new();
            if let Some(stderr) = child.stderr.take() {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    if line.starts_with("EXISTS\t") {
                        existing = true;
                        report(Self::phone_progress(file, index + 1, files.len(), 0, true));
                    } else if let Some(name) = line.strip_prefix("RENAMED\t") {
                        renamed_from = Some(name.to_owned());
                    } else if let Some(fields) = line.strip_prefix("PROGRESS\t") {
                        let done = fields
                            .split('\t')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        report(Self::phone_progress(
                            file,
                            index + 1,
                            files.len(),
                            done,
                            existing,
                        ));
                    } else if !line.trim().is_empty() {
                        error_lines.push(line);
                    }
                }
            }
            let mut path = String::new();
            if let Some(mut stdout) = child.stdout.take() {
                let _ = stdout.read_to_string(&mut path);
            }
            let status = child.wait();
            match status {
                Ok(result) if result.success() && !path.trim().is_empty() => {
                    if let Some(name) = renamed_from {
                        let path = PathBuf::from(path.trim());
                        let saved_name = path.file_name().unwrap_or_default().to_string_lossy();
                        duplicates.push(format!(
                            "Name already exists. Saved as {saved_name} (original: {name})"
                        ));
                    }
                    saved.push(path.trim().to_string());
                }
                Ok(result) if result.code() == Some(3) => {
                    report((
                        Overlay::PhoneShare {
                            url: url.into(),
                            files: vec![],
                            selected: vec![],
                            cursor: 0,
                            stage: PhoneStage::Waiting,
                        },
                        format!(
                            "Saved {} of {} files. Approve the computer on the phone, then press R",
                            saved.len(),
                            files.len()
                        ),
                    ));
                    return;
                }
                Ok(_) => {
                    let error = error_lines.join(" ");
                    report(Self::phone_download_result(
                        &saved,
                        &duplicates,
                        files.len(),
                        Some(if error.is_empty() {
                            "Download failed"
                        } else {
                            &error
                        }),
                    ));
                    return;
                }
                Err(error) => {
                    report(Self::phone_download_result(
                        &saved,
                        &duplicates,
                        files.len(),
                        Some(&error.to_string()),
                    ));
                    return;
                }
            }
        }
        report(Self::phone_download_result(
            &saved,
            &duplicates,
            files.len(),
            None,
        ));
    }

    #[cfg(windows)]
    fn phone_progress(
        file: &PhoneFile,
        index: usize,
        count: usize,
        done: u64,
        existing: bool,
    ) -> (Overlay, String) {
        (
            Overlay::PhoneDownload {
                name: file.name.clone(),
                index,
                count,
                done,
                total: file.size,
                existing,
            },
            format!("Receiving {index}/{count}: {} ({done} bytes)", file.name),
        )
    }

    #[cfg(windows)]
    fn phone_download_result(
        saved: &[String],
        duplicates: &[String],
        total: usize,
        failure: Option<&str>,
    ) -> (Overlay, String) {
        let mut lines = vec![format!("Saved {} of {total} files", saved.len())];
        if let Some(failure) = failure {
            lines.push(format!("Download failed: {failure}"));
        }
        lines.extend(duplicates.iter().take(5).cloned());
        if duplicates.len() > 5 {
            lines.push(format!(
                "{} more files saved with new names",
                duplicates.len() - 5
            ));
        }
        lines.extend(saved.iter().take(9).map(|path| format!("Saved: {path}")));
        if saved.len() > 9 {
            lines.push(format!("... and {} more", saved.len() - 9));
        }
        let message = if let Some(failure) = failure {
            format!("Saved {} of {total} files. {failure}", saved.len())
        } else if !duplicates.is_empty() {
            format!(
                "Saved {} of {total} files. {} saved with new names",
                saved.len(),
                duplicates.len()
            )
        } else {
            format!("Downloaded {}/{} file(s) from phone", saved.len(), total)
        };
        (Overlay::PhoneDownloadDone { lines }, message)
    }

    fn start_server(&mut self) {
        let mut files = self.explorer.selected_files();
        if files.is_empty() {
            if let Some(path) = self.explorer.focused_file() {
                files.push(path);
            }
        }
        self.start_with_files(&files);
    }

    fn start_receiving(&mut self) {
        self.start_with_files(&[]);
    }

    fn start_with_files(&mut self, files: &[PathBuf]) {
        match ServerProcess::start(files) {
            Ok(server) => {
                self.server = Some(server);
                if !files.is_empty() {
                    self.explorer.clear_selection();
                }
                self.pairing_view = 1;
                self.auto_dashboard = true;
                self.message = if files.is_empty() {
                    "Receiving is live. Open TapSend on your phone and send files to this computer"
                        .into()
                } else {
                    "Sharing is live. A saved phone can reconnect without pairing again".into()
                };
            }
            Err(error) => self.message = error.to_string(),
        }
    }

    fn add_files(&mut self) {
        let mut files = self.explorer.selected_files();
        if files.is_empty() {
            if let Some(path) = self.explorer.focused_file() {
                files.push(path);
            }
        }
        if files.is_empty() {
            self.message = "Select files or focus a file, then press a".into();
            return;
        }
        let mut added = 0;
        if let Some(server) = self.server.as_mut() {
            for path in files {
                match server.add_file(&path) {
                    Ok(()) => {
                        added += 1;
                        self.explorer.unselect(&path);
                    }
                    Err(error) => {
                        self.message = format!("Could not add {}: {error}", path.display());
                        return;
                    }
                }
            }
        }
        self.message =
            format!("{added} file(s) added. Connected phones will see them automatically");
    }

    fn toggle_transfers(&mut self) {
        let action = if self.stopped { "resume" } else { "stop" };
        if let Some(server) = self.server.as_mut() {
            self.message = match server.control(action) {
                Ok(()) => {
                    self.stopped = !self.stopped;
                    format!("Downloads {action} requested")
                }
                Err(error) => error.to_string(),
            };
        }
    }

    pub(crate) fn handle_modal(&mut self, key: ModalKey) -> io::Result<()> {
        let Some(overlay) = self.overlay.take() else {
            return Ok(());
        };
        match overlay {
            #[cfg(windows)]
            Overlay::PasteStack {
                mut cursor,
                mut status,
                mut waiting,
            } => {
                if matches!(key, ModalKey::Escape) {
                    return Ok(());
                }
                if waiting.is_none()
                    && let Some(server) = self.server.as_mut()
                {
                    let items = server.pastes();
                    match key {
                        ModalKey::Up => cursor = cursor.saturating_sub(1),
                        ModalKey::Down => cursor = (cursor + 1).min(items.len().saturating_sub(1)),
                        ModalKey::Char('b' | 'B') => {
                            server.reminder("paste-release-all", "", "")?;
                            status = "Restoring held items…".into();
                        }
                        ModalKey::Char(ch @ ('h' | 'H' | 'j' | 'J')) => {
                            if let Some(item) = items.get(cursor) {
                                let target = if ch.eq_ignore_ascii_case(&'h') {
                                    Some((
                                        "paste-hold-folder",
                                        self.explorer.directory().to_string_lossy(),
                                    ))
                                } else {
                                    self.recipient
                                        .as_ref()
                                        .map(|(id, _)| ("paste-hold-device", id.as_str().into()))
                                };
                                if let Some((action, value)) = target {
                                    server.reminder(action, &item.id, &value)?;
                                } else {
                                    status = "Press V and select a phone first".into();
                                }
                            }
                        }
                        ModalKey::Char(ch @ ('u' | 'U' | 'd' | 'D' | 'x' | 'X')) => {
                            if let Some(item) = items.get(cursor) {
                                let action = match ch.to_ascii_lowercase() {
                                    'u' => "paste-up",
                                    'd' => "paste-down",
                                    _ => "paste-skip",
                                };
                                match server.change_paste(&item.id, action) {
                                    Ok(()) => waiting = Some((item.id.clone(), action)),
                                    Err(error) => status = error.to_string(),
                                }
                            }
                        }
                        _ => {}
                    }
                }
                self.overlay = Some(Overlay::PasteStack {
                    cursor,
                    status,
                    waiting,
                });
            }
            #[cfg(windows)]
            Overlay::Scratchpad {
                mut cursor,
                entries,
                status,
            } => {
                match key {
                    ModalKey::Escape | ModalKey::Char('k' | 'K') => return Ok(()),
                    ModalKey::Char('r' | 'R') => {
                        self.desktop_scratchpad();
                        return Ok(());
                    }
                    ModalKey::Up => cursor = cursor.saturating_sub(1),
                    ModalKey::Down => cursor = (cursor + 1).min(entries.len().saturating_sub(1)),
                    _ => {}
                }
                self.overlay = Some(Overlay::Scratchpad {
                    cursor,
                    entries,
                    status,
                });
            }
            #[cfg(windows)]
            Overlay::FileSearch {
                mut query,
                mut matches,
                mut cursor,
            } => {
                match key {
                    ModalKey::Escape => return Ok(()),
                    ModalKey::Up => cursor = cursor.saturating_sub(1),
                    ModalKey::Down => cursor = (cursor + 1).min(matches.len().saturating_sub(1)),
                    ModalKey::Enter => {
                        if let Some(&index) = matches.get(cursor) {
                            self.explorer.toggle_entry(index);
                        }
                    }
                    ModalKey::Backspace | ModalKey::Char(_) => {
                        let changed = match key {
                            ModalKey::Backspace => query.pop().is_some(),
                            ModalKey::Char(ch)
                                if !ch.is_control() && query.len() + ch.len_utf8() <= 1024 =>
                            {
                                query.push(ch);
                                true
                            }
                            _ => false,
                        };
                        if changed {
                            matches = self.explorer.file_matches(&query);
                            cursor = 0;
                        }
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::FileSearch {
                    query,
                    matches,
                    cursor,
                });
            }
            #[cfg(windows)]
            Overlay::Recipient {
                mut cursor,
                files,
                devices,
            } => {
                let available = devices.as_deref().unwrap_or_default();
                match key {
                    ModalKey::Escape => return Ok(()),
                    ModalKey::Char('p' | 'P') => return self.desktop_pair(),
                    ModalKey::Char('m' | 'M') => return self.handle_key(Key::Devices),
                    ModalKey::Char('u' | 'U') => {
                        self.pick_recipient(files);
                        return Ok(());
                    }
                    ModalKey::Char('r' | 'R' | 'x' | 'X') if available.get(cursor).is_some() => {
                        let entry = available[cursor].clone();
                        let task = if matches!(key, ModalKey::Char('r' | 'R')) {
                            DeviceTask::Rename {
                                entry,
                                input: String::new(),
                            }
                        } else {
                            DeviceTask::Forget(entry)
                        };
                        self.overlay = Some(Overlay::DeviceEdit { files, task });
                        return Ok(());
                    }
                    ModalKey::Char('c' | 'C') if !available.is_empty() => {
                        self.overlay = Some(Overlay::DeviceEdit {
                            files,
                            task: DeviceTask::Clear(available.len()),
                        });
                        return Ok(());
                    }
                    ModalKey::Char('a' | 'A')
                        if available.get(cursor).is_some_and(|entry| entry.pending) =>
                    {
                        self.update_device("approve", &available[cursor].id, "", files, cursor);
                        return Ok(());
                    }
                    ModalKey::Up => cursor = cursor.saturating_sub(1),
                    ModalKey::Down => cursor = (cursor + 1).min(available.len().saturating_sub(1)),
                    ModalKey::Tab | ModalKey::Char('v' | 'V') if !available.is_empty() => {
                        cursor = (cursor + 1) % available.len();
                    }
                    ModalKey::Enter => {
                        if let Some(entry) = available.get(cursor) {
                            if entry.pending {
                                self.message = "Press A to approve this phone first.".into();
                                self.overlay = Some(Overlay::Recipient {
                                    cursor,
                                    files,
                                    devices,
                                });
                                return Ok(());
                            }
                            self.recipient = Some((entry.id.clone(), entry.name.clone()));
                            self.message =
                                format!("To {} · press S to send marked files", entry.name);
                            self.page = 1;
                            self.pairing_view = 0;
                            if !files.is_empty() {
                                self.share_desktop_files(&files);
                            }
                            return Ok(());
                        }
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Recipient {
                    cursor,
                    files,
                    devices,
                });
            }
            #[cfg(windows)]
            Overlay::DeviceEdit { files, mut task } => {
                if matches!(key, ModalKey::Escape) {
                    self.pick_recipient(files);
                    return Ok(());
                }
                let request = match &mut task {
                    DeviceTask::Rename { entry, input } => match key {
                        ModalKey::Backspace => {
                            input.pop();
                            None
                        }
                        ModalKey::Enter if !input.trim().is_empty() => {
                            Some(("rename", entry.id.clone(), input.trim().to_owned()))
                        }
                        ModalKey::Char(ch)
                            if !ch.is_control() && input.len() + ch.len_utf8() <= 80 =>
                        {
                            input.push(ch);
                            None
                        }
                        _ => None,
                    },
                    DeviceTask::Forget(entry) if matches!(key, ModalKey::Char('y' | 'Y')) => {
                        Some(("revoke", entry.id.clone(), String::new()))
                    }
                    DeviceTask::Clear(_) if matches!(key, ModalKey::Char('y' | 'Y')) => {
                        Some(("clear-devices", String::new(), String::new()))
                    }
                    _ => None,
                };
                if let Some((action, id, name)) = request {
                    self.update_device(action, &id, &name, files, 0);
                } else {
                    self.overlay = Some(Overlay::DeviceEdit { files, task });
                }
            }
            #[cfg(windows)]
            Overlay::DesktopShare { files, mut scroll } => match key {
                ModalKey::Enter if self.server.as_ref().is_some_and(ServerProcess::ready) => {
                    self.pick_recipient(files)
                }
                ModalKey::Enter => {
                    self.desktop_receive();
                    self.overlay = Some(Overlay::DesktopShare { files, scroll });
                }
                ModalKey::Escape => self.message = "File Explorer share cancelled".into(),
                _ => {
                    match key {
                        ModalKey::Down => scroll = (scroll + 1).min(files.len().saturating_sub(1)),
                        ModalKey::Up => scroll = scroll.saturating_sub(1),
                        _ => {}
                    }
                    self.overlay = Some(Overlay::DesktopShare { files, scroll });
                }
            },
            #[cfg(windows)]
            Overlay::PhoneDownloadDone { lines } => {
                if !matches!(
                    key,
                    ModalKey::Escape | ModalKey::Enter | ModalKey::Backspace
                ) {
                    self.overlay = Some(Overlay::PhoneDownloadDone { lines });
                }
            }
            #[cfg(windows)]
            progress @ Overlay::PhoneDownload { .. } => self.overlay = Some(progress),
            #[cfg(windows)]
            Overlay::PhoneShare {
                url,
                files,
                mut cursor,
                mut selected,
                mut stage,
            } => {
                match (stage, key) {
                    (_, ModalKey::Escape) => return Ok(()),
                    (
                        PhoneStage::Waiting | PhoneStage::Empty,
                        ModalKey::Char('r' | 'R') | ModalKey::Enter,
                    ) => {
                        self.load_phone_share(url);
                        return Ok(());
                    }
                    (PhoneStage::AskAll, ModalKey::Char('y' | 'Y')) => {
                        self.download_phone_files(&url, &files);
                        return Ok(());
                    }
                    (PhoneStage::AskAll, ModalKey::Char('n' | 'N')) => stage = PhoneStage::Choose,
                    (PhoneStage::Choose, ModalKey::Up | ModalKey::Char('k' | 'K')) => {
                        cursor = cursor.saturating_sub(1)
                    }
                    (PhoneStage::Choose, ModalKey::Down | ModalKey::Char('j' | 'J')) => {
                        cursor = (cursor + 1).min(files.len().saturating_sub(1))
                    }
                    (PhoneStage::Choose, ModalKey::Char(' ')) => {
                        if let Some(item) = selected.get_mut(cursor) {
                            *item = !*item;
                        }
                    }
                    (PhoneStage::Choose, ModalKey::Enter) => {
                        let chosen: Vec<PhoneFile> = files
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, file)| {
                                (selected.get(i).copied().unwrap_or(false)
                                    || i == cursor && !selected.iter().any(|selected| *selected))
                                .then_some(file)
                            })
                            .collect();
                        self.download_phone_files(&url, &chosen);
                        return Ok(());
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::PhoneShare {
                    url,
                    files,
                    cursor,
                    selected,
                    stage,
                });
            }
            Overlay::Help { mut scroll } => {
                match key {
                    ModalKey::Escape | ModalKey::Char('?') | ModalKey::Backspace => return Ok(()),
                    ModalKey::Up | ModalKey::Char('k') | ModalKey::Char('K') => {
                        scroll = scroll.saturating_sub(1)
                    }
                    ModalKey::Down | ModalKey::Char('j') | ModalKey::Char('J') => {
                        scroll = (scroll + 1).min(ui::help_line_count().saturating_sub(1))
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Help { scroll });
            }
            Overlay::Command {
                mut input,
                mut selected,
            } => {
                let suggestions = ui::command_suggestions(&input, self.server.is_some());
                match key {
                    ModalKey::Escape => return Ok(()),
                    ModalKey::Backspace => {
                        input.pop();
                        selected = 0;
                    }
                    ModalKey::Char(ch) => {
                        if input.len() < 2000 {
                            input.push(ch);
                            selected = 0;
                        }
                    }
                    ModalKey::Up => selected = selected.saturating_sub(1),
                    ModalKey::Down => {
                        selected = (selected + 1).min(suggestions.len().saturating_sub(1))
                    }
                    ModalKey::Tab => {
                        if let Some(choice) = suggestions.get(selected) {
                            input = format!(
                                "{}{}",
                                choice.name,
                                if choice.argument { " " } else { "" }
                            );
                            selected = 0;
                        }
                    }
                    ModalKey::Enter => {
                        let typed = input.trim();
                        let exact = suggestions.iter().find(|choice| choice.name == typed);
                        let chosen =
                            if typed.is_empty() || (!typed.contains(' ') && exact.is_none()) {
                                suggestions
                                    .get(selected)
                                    .map(|choice| choice.name)
                                    .unwrap_or(typed)
                            } else {
                                typed
                            };
                        let needs_argument = ui::command_suggestions(chosen, self.server.is_some())
                            .iter()
                            .any(|choice| choice.name == chosen && choice.argument);
                        if needs_argument && !chosen.contains(' ') {
                            self.overlay = Some(Overlay::Command {
                                input: format!("{chosen} "),
                                selected: 0,
                            });
                            return Ok(());
                        }
                        self.execute_command(chosen)?;
                        return Ok(());
                    }
                    ModalKey::Other => {}
                }
                self.overlay = Some(Overlay::Command { input, selected });
            }
            Overlay::Settings {
                mut values,
                mut selected,
                mut edit,
                mut error,
                mut dirty,
                mut confirm_exit,
                first_run,
            } => {
                if let Some(value) = edit.as_mut() {
                    match key {
                        ModalKey::Escape => edit = None,
                        ModalKey::Backspace => {
                            value.pop();
                        }
                        ModalKey::Char(ch) if value.len() < 1024 => value.push(ch),
                        ModalKey::Enter => {
                            values[selected] = value.trim().to_string();
                            edit = None;
                            dirty = true;
                            error.clear();
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        ModalKey::Escape if first_run => {
                            error = "Finish setup or close TapSend to set it up later.".into();
                        }
                        ModalKey::Escape if !dirty || confirm_exit => return Ok(()),
                        ModalKey::Escape => {
                            confirm_exit = true;
                            error = "Unsaved changes. Press Esc again to discard.".into();
                        }
                        ModalKey::Up | ModalKey::Down | ModalKey::Tab => {
                            let fields = ui::settings_fields(first_run);
                            let row = fields
                                .iter()
                                .position(|index| *index == selected)
                                .unwrap_or(0);
                            selected = fields[if matches!(key, ModalKey::Up) {
                                row.saturating_sub(1)
                            } else {
                                (row + 1) % fields.len()
                            }];
                        }
                        ModalKey::Enter => {
                            if selected == 2 || selected == 8 {
                                values[selected] = if values[selected] == "true" {
                                    "false"
                                } else {
                                    "true"
                                }
                                .into();
                                dirty = true;
                            } else {
                                edit = Some(values[selected].clone());
                            }
                            error.clear();
                        }
                        ModalKey::Char('s') | ModalKey::Char('S') => {
                            match self.save_settings(&values, first_run) {
                                Ok(()) => {
                                    #[cfg(windows)]
                                    if first_run {
                                        self.desktop_setup_blocked = false;
                                        self.desktop_receive();
                                    }
                                    return Ok(());
                                }
                                Err(message) => error = message,
                            }
                        }
                        ModalKey::Char('d') | ModalKey::Char('D') => {
                            match Self::load_settings("defaults") {
                                Ok(defaults) => {
                                    values = defaults;
                                    dirty = true;
                                    error = "Defaults loaded. Press S to save.".into();
                                }
                                Err(message) => error = message,
                            }
                        }
                        _ => {}
                    }
                    if !matches!(key, ModalKey::Escape) {
                        confirm_exit = false;
                    }
                }
                self.overlay = Some(Overlay::Settings {
                    values,
                    selected,
                    edit,
                    error,
                    dirty,
                    confirm_exit,
                    first_run,
                });
            }
        }
        Ok(())
    }

    fn execute_command(&mut self, raw: &str) -> io::Result<()> {
        let raw = raw.trim();
        let (action, argument) = raw.split_once(' ').unwrap_or((raw, ""));
        let argument = argument.trim();
        match action {
            #[cfg(windows)]
            "relay" => {
                let (verb, value) = argument.split_once(' ').unwrap_or((argument, ""));
                let result = match verb {
                    "create" if !value.trim().is_empty() => self.start_relay_job(
                        vec!["create".into(), value.trim().trim_matches('"').into()],
                        None,
                    ),
                    "send" if value.is_empty() => {
                        let file = self
                            .explorer
                            .entries()
                            .get(self.explorer.cursor())
                            .filter(|entry| !entry.directory)
                            .map(|entry| entry.path.to_string_lossy().into_owned());
                        if let Some(file) = file {
                            self.start_relay_job(vec!["send".into(), file], None)
                        } else {
                            Err(io::Error::other("Focus a file first, then run relay send"))
                        }
                    }
                    "status" if value.is_empty() => {
                        self.start_relay_job(vec!["status".into()], None)
                    }
                    _ => Err(io::Error::other(
                        "Relay: create SHARED_FOLDER, send, status · Shift+J pairs",
                    )),
                };
                self.update(result);
            }
            "share" if self.server.is_none() => self.start_server(),
            "share" => self.message = "Sharing is already live; use add".into(),
            "receive" if self.server.is_none() => self.start_receiving(),
            "receive" => self.message = "TapSend is already running and can receive files".into(),
            "add" if self.server.is_some() && argument.is_empty() => self.add_files(),
            "add" if self.server.is_some() => {
                let path = PathBuf::from(argument);
                let path = if path.is_absolute() {
                    path
                } else {
                    self.explorer.directory().join(path)
                };
                self.message = match self.server.as_mut().unwrap().add_file(&path) {
                    Ok(()) => format!("Adding {} to the live share", path.display()),
                    Err(error) => error.to_string(),
                };
            }
            "files" => {
                self.device_view = false;
                self.page = 1;
                self.pairing_view = 0;
            }
            "devices" => {
                self.page = 2;
                self.pairing_view = 0;
                self.device_view = true;
                if let Some(server) = self.server.as_mut() {
                    self.message = match server.control("devices") {
                        Ok(()) => "Refreshing saved phones".into(),
                        Err(error) => error.to_string(),
                    };
                } else {
                    self.message = "Start sharing to see saved phones".into();
                }
            }
            "pairing" | "qr" if self.server.is_some() => {
                self.pairing_view = 1;
                self.auto_dashboard = false;
            }
            "new-code" if self.server.is_some() => {
                self.message = match self.server.as_mut().unwrap().control("pairing") {
                    Ok(()) => {
                        self.pairing_view = 1;
                        "New pairing code requested".into()
                    }
                    Err(error) => error.to_string(),
                };
            }
            "pause" | "stop" if self.server.is_some() => self.set_transfers(true),
            "resume" if self.server.is_some() => self.set_transfers(false),
            "message" | "rename" | "revoke" | "recipient" | "approve" | "deny"
                if self.server.is_some() =>
            {
                if argument.is_empty() {
                    self.message = format!("{action} needs an argument");
                } else if let Some(server) = self.server.as_mut() {
                    self.message = match server.control(raw) {
                        Ok(()) => {
                            if action == "rename"
                                || action == "revoke"
                                || action == "approve"
                                || action == "deny"
                            {
                                let _ = server.control("devices");
                            }
                            format!("{action} sent; see connection logs for the result")
                        }
                        Err(error) => error.to_string(),
                    };
                }
            }
            "hidden" => {
                let result = self.explorer.toggle_hidden();
                self.update(result);
            }
            "help" => self.overlay = Some(Overlay::Help { scroll: 0 }),
            "settings" => self.open_settings(),
            "quit" => self.quit_requested = true,
            "" => {}
            _ => self.message = format!("Unknown or unavailable command: {action}"),
        }
        Ok(())
    }

    fn set_transfers(&mut self, paused: bool) {
        if let Some(server) = self.server.as_mut() {
            let action = if paused { "stop" } else { "resume" };
            self.message = match server.control(action) {
                Ok(()) => {
                    self.stopped = paused;
                    if paused {
                        "Downloads paused".into()
                    } else {
                        "Downloads resumed".into()
                    }
                }
                Err(error) => error.to_string(),
            };
        }
    }

    fn update(&mut self, result: io::Result<()>) {
        if let Err(error) = result {
            self.message = error.to_string();
        }
    }

    fn launcher() -> PathBuf {
        let name = format!("tapsend{}", env::consts::EXE_SUFFIX);
        env::current_exe()
            .ok()
            .and_then(|path| {
                let sibling = path.with_file_name(&name);
                sibling.is_file().then_some(sibling)
            })
            .unwrap_or_else(|| PathBuf::from(name))
    }

    fn open_settings(&mut self) {
        #[cfg(windows)]
        let first_run = self.desktop_setup_blocked;
        #[cfg(not(windows))]
        let first_run = false;
        self.open_settings_panel(first_run);
    }

    fn open_settings_panel(&mut self, first_run: bool) {
        match Self::load_settings("show") {
            Ok(values) => {
                self.overlay = Some(Overlay::Settings {
                    values,
                    selected: 0,
                    edit: None,
                    error: String::new(),
                    dirty: false,
                    confirm_exit: false,
                    first_run,
                })
            }
            Err(error) => self.message = error,
        }
    }

    fn load_settings(mode: &str) -> Result<[String; 9], String> {
        let output = Self::settings_command()
            .args(["settings", mode])
            .output()
            .map_err(|error| {
                format!("Could not load settings: {error}. Keep tapsend.exe beside tapsend-tui.exe")
            })?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut values: [String; 9] = std::array::from_fn(|_| String::new());
        let mut found = [false; 9];
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('\t') {
                if let Some(index) = SETTING_KEYS.iter().position(|candidate| *candidate == key) {
                    values[index] = value.into();
                    found[index] = true;
                }
            }
        }
        if found.iter().any(|found| !found) {
            return Err("Settings response is incomplete. Rebuild both TapSend binaries.".into());
        }
        Ok(values)
    }

    fn save_settings(&mut self, values: &[String; 9], first_run: bool) -> Result<(), String> {
        let output = Self::settings_command()
            .args([
                "settings",
                if first_run { "setup" } else { "set" },
                "--inbox",
                &values[0],
                "--start-folder",
                &values[1],
                "--listen",
                &values[3],
                "--pairing-host",
                &values[4],
                "--direct-joins",
                &values[5],
                "--data-dir",
                &values[6],
                "--upload-limit-gib",
                &values[7],
            ])
            .arg(format!("--show-hidden={}", values[2]))
            .arg(format!("--portal-enabled={}", values[8]))
            .output()
            .map_err(|error| format!("Could not save settings: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        self.explorer
            .set_show_hidden(values[2] == "true")
            .map_err(|error| error.to_string())?;
        self.message =
            "Settings saved. Restart an active share to apply connection changes.".into();
        Ok(())
    }

    fn settings_command() -> Command {
        #[allow(unused_mut)]
        let mut command = Command::new(Self::launcher());
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        command
    }
}
