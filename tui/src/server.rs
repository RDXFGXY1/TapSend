#[cfg(windows)]
use std::{collections::VecDeque, os::windows::process::CommandExt};
#[cfg(windows)]
#[path = "send_queue.rs"]
mod send_queue;
#[cfg(windows)]
pub(crate) use send_queue::{QueueRow, SendState};
#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct Snippet {
    pub device: String,
    pub from_phone: bool,
    pub text: String,
}
#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct PasteItem {
    pub id: String,
    pub image: bool,
    pub file: bool,
    pub name: String,
    pub device: String,
    pub preview: String,
    pub path: PathBuf,
}
#[cfg(windows)]
pub(crate) struct PasteResult {
    pub id: String,
    pub action: String,
    pub outcome: Result<(), String>,
}
#[cfg(windows)]
pub(crate) struct ScreenTicket {
    pub port: u16,
    pub key: String,
    pub pin: String,
}
#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct RecipientDevice {
    pub id: String,
    pub name: String,
    pub pending: bool,
}
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

fn control_quote(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .chars()
        .filter(|ch| !ch.is_control())
        .collect()
}

fn server_binary() -> OsString {
    std::env::var_os("TAPSEND_SERVER").unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|path| {
                let sibling =
                    path.with_file_name(format!("tapsend-server{}", std::env::consts::EXE_SUFFIX));
                sibling.is_file().then_some(sibling.into_os_string())
            })
            .unwrap_or_else(|| format!("tapsend-server{}", std::env::consts::EXE_SUFFIX).into())
    })
}

pub struct ServerProcess {
    child: Child,
    logs: Arc<Mutex<Vec<String>>>,
    shared: Vec<PathBuf>,
    ready: Arc<AtomicBool>,
    #[cfg(windows)]
    notices: Arc<Mutex<VecDeque<String>>>,
    #[cfg(windows)]
    outgoing: Arc<Mutex<send_queue::SendQueue>>,
    #[cfg(windows)]
    recipients: Arc<Mutex<Option<Vec<RecipientDevice>>>>,
    #[cfg(windows)]
    device_result: Arc<Mutex<Option<(String, Result<(), String>)>>>,
    #[cfg(windows)]
    pairing_pass: Arc<Mutex<Option<(String, String)>>>,
    #[cfg(windows)]
    pair_notice: Arc<Mutex<Option<(String, bool)>>>,
    #[cfg(windows)]
    snippets: Arc<Mutex<VecDeque<Snippet>>>,
    #[cfg(windows)]
    snippet_result: Arc<Mutex<Option<Result<(), String>>>>,
    #[cfg(windows)]
    pastes: Arc<Mutex<Vec<PasteItem>>>,
    #[cfg(windows)]
    paste_result: Arc<Mutex<Option<PasteResult>>>,
    #[cfg(windows)]
    screen_ticket: Arc<Mutex<Option<Result<ScreenTicket, String>>>>,
}

fn server_command() -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(server_binary());
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    command
}

impl ServerProcess {
    pub fn start(files: &[PathBuf]) -> io::Result<Self> {
        let mut command = server_command();
        for file in files {
            command.arg("--file").arg(file);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("server log unavailable"))?;
        let logs = Arc::new(Mutex::new(Vec::new()));
        let thread_logs = Arc::clone(&logs);
        let ready = Arc::new(AtomicBool::new(false));
        let thread_ready = Arc::clone(&ready);
        #[cfg(windows)]
        let notices = Arc::new(Mutex::new(VecDeque::new()));
        #[cfg(windows)]
        let thread_notices = Arc::clone(&notices);
        #[cfg(windows)]
        let outgoing = Arc::new(Mutex::new(send_queue::SendQueue::default()));
        #[cfg(windows)]
        let thread_outgoing = Arc::clone(&outgoing);
        #[cfg(windows)]
        let recipients = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_recipients = Arc::clone(&recipients);
        #[cfg(windows)]
        let device_result = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_device_result = Arc::clone(&device_result);
        #[cfg(windows)]
        let pairing_pass = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_pairing_pass = Arc::clone(&pairing_pass);
        #[cfg(windows)]
        let pair_notice = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_pair_notice = Arc::clone(&pair_notice);
        #[cfg(windows)]
        let snippets = Arc::new(Mutex::new(VecDeque::new()));
        #[cfg(windows)]
        let thread_snippets = Arc::clone(&snippets);
        #[cfg(windows)]
        let snippet_result = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_snippet_result = Arc::clone(&snippet_result);
        #[cfg(windows)]
        let pastes = Arc::new(Mutex::new(Vec::new()));
        #[cfg(windows)]
        let thread_pastes = Arc::clone(&pastes);
        #[cfg(windows)]
        let paste_result = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_paste_result = Arc::clone(&paste_result);
        #[cfg(windows)]
        let screen_ticket = Arc::new(Mutex::new(None));
        #[cfg(windows)]
        let thread_screen_ticket = Arc::clone(&screen_ticket);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some(body) = line.splitn(3, ' ').nth(2) {
                    if body == "TUI_READY" {
                        thread_ready.store(true, Ordering::Release);
                    }
                    #[cfg(windows)]
                    if let Ok(mut queue) = thread_outgoing.lock() {
                        queue.apply(body);
                    }
                    #[cfg(windows)]
                    if let Some(fields) = body.strip_prefix("TUI_SCREEN ")
                        && let Ok(mut ticket) = thread_screen_ticket.lock()
                    {
                        let mut words = fields.split_ascii_whitespace();
                        let parsed = words
                            .next()
                            .and_then(|word| word.strip_prefix("port="))
                            .and_then(|port| port.parse::<u16>().ok())
                            .zip(words.next().and_then(|word| word.strip_prefix("key=")))
                            .zip(words.next().and_then(|word| word.strip_prefix("pin=")));
                        *ticket = Some(
                            parsed
                                .filter(|((port, key), pin)| {
                                    *port != 0 && key.len() == 64 && pin.len() == 64
                                })
                                .map(|((port, key), pin)| ScreenTicket {
                                    port,
                                    key: key.into(),
                                    pin: pin.into(),
                                })
                                .ok_or_else(|| "Could not read the screen viewer address".into()),
                        );
                    }
                    #[cfg(windows)]
                    if let Some(error) = body.strip_prefix("TUI_SCREEN_ERROR ")
                        && let Ok(mut ticket) = thread_screen_ticket.lock()
                    {
                        *ticket = Some(Err(crate::app::hex_decode(error)
                            .unwrap_or_else(|| "Could not start screen sharing".into())));
                    }
                    // The viewer ticket is local IPC and must not appear in the TUI logs.
                    if body.starts_with("TUI_SCREEN ") || body.starts_with("TUI_SCREEN_ERROR ") {
                        continue;
                    }
                    #[cfg(windows)]
                    if let Some(fields) = body.strip_prefix("TUI_PASTE_RESULT ")
                        && let Some((id, fields)) = fields.split_once(' ')
                        && let Some((action, value)) = fields.split_once(' ')
                        && let Ok(mut result) = thread_paste_result.lock()
                    {
                        let outcome = if value == "ok" {
                            Ok(())
                        } else {
                            Err(crate::app::hex_decode(value)
                                .unwrap_or_else(|| "Paste Stack action failed".into()))
                        };
                        *result = Some(PasteResult {
                            id: id.into(),
                            action: action.into(),
                            outcome,
                        });
                    }
                    #[cfg(windows)]
                    if (body == "TUI_PASTE" || body.starts_with("TUI_PASTE "))
                        && let Ok(mut rows) = thread_pastes.lock()
                    {
                        *rows = body
                            .split_ascii_whitespace()
                            .skip(1)
                            .filter_map(|row| {
                                let mut fields = row.split(':');
                                let id = fields.next()?.to_owned();
                                let kind = fields.next()?;
                                let name = crate::app::hex_decode(fields.next()?)?;
                                let device = crate::app::hex_decode(fields.next()?)?;
                                let preview = crate::app::hex_decode(fields.next()?)?;
                                let path = crate::app::hex_decode(fields.next()?)?.into();
                                Some(PasteItem {
                                    id,
                                    image: kind == "image",
                                    file: kind == "file",
                                    name,
                                    device,
                                    preview,
                                    path,
                                })
                            })
                            .collect();
                    }
                    #[cfg(windows)]
                    if let Some(value) = body.strip_prefix("TUI_SNIPPET_RESULT ")
                        && let Ok(mut result) = thread_snippet_result.lock()
                    {
                        *result = Some(if value == "ok" {
                            Ok(())
                        } else {
                            Err(crate::app::hex_decode(value)
                                .unwrap_or_else(|| "Could not share snippet".into()))
                        });
                    }
                    #[cfg(windows)]
                    if let Some(entry) = body.strip_prefix("TUI_SNIPPET from=")
                        && let Some((from, entry)) = entry.split_once(" device=")
                        && let Some((device, text)) = entry.split_once(" text=")
                        && let (Some(device), Some(text)) =
                            (crate::app::hex_decode(device), crate::app::hex_decode(text))
                        && let Ok(mut snippets) = thread_snippets.lock()
                    {
                        snippets.push_front(Snippet {
                            device,
                            from_phone: from == "phone",
                            text,
                        });
                        snippets.truncate(20);
                    }
                    #[cfg(windows)]
                    if let Some(body) = body.strip_prefix("TUI_PAIR_PASS url=")
                        && let Some((url, pass)) = body.split_once(" pass=")
                        && let Some(url) = crate::app::hex_decode(url)
                        && let Ok(mut pairing) = thread_pairing_pass.lock()
                    {
                        *pairing = Some((url, pass.into()));
                    }
                    #[cfg(windows)]
                    if let Some(body) = body.strip_prefix("TUI_DEVICE_RESULT ")
                        && let Some((action, status)) = body.split_once(' ')
                        && let Ok(mut result) = thread_device_result.lock()
                    {
                        let status = if status == "ok" {
                            Ok(())
                        } else {
                            Err(crate::app::hex_decode(status)
                                .unwrap_or_else(|| "Device update failed".into()))
                        };
                        *result = Some((action.into(), status));
                    }
                    #[cfg(windows)]
                    if let Some(fields) = body.strip_prefix("TUI_PAIR_RESULT ")
                        && let Some((status, name)) = fields.split_once(' ')
                        && let Some(name) = crate::app::hex_decode(name)
                        && let Ok(mut notice) = thread_pair_notice.lock()
                    {
                        let name: String = name.chars().filter(|ch| !ch.is_control()).collect();
                        *notice = Some((name, status == "pending"));
                    }
                    if body.starts_with("TUI_PAIR_PASS ")
                        || body.starts_with("TUI_DEVICE_RESULT ")
                        || body.starts_with("TUI_PAIR_RESULT ")
                    {
                        continue;
                    }
                    #[cfg(windows)]
                    if let Some(snapshot) = body.strip_prefix("TUI_RECIPIENTS")
                        && let Ok(mut recipients) = thread_recipients.lock()
                    {
                        *recipients = Some(
                            snapshot
                                .split_whitespace()
                                .filter_map(|entry| {
                                    let (id, rest) = entry.split_once(':')?;
                                    let (name, state) =
                                        rest.split_once(':').unwrap_or((rest, "false"));
                                    let id = crate::app::hex_decode(id)?;
                                    if id.is_empty() || id.chars().any(char::is_control) {
                                        return None;
                                    }
                                    let name: String = crate::app::hex_decode(name)?
                                        .chars()
                                        .filter(|ch| !ch.is_control())
                                        .collect();
                                    let name = if name.trim().is_empty() {
                                        format!(
                                            "Unnamed device · {}",
                                            id.chars().take(8).collect::<String>()
                                        )
                                    } else {
                                        name
                                    };
                                    Some(RecipientDevice {
                                        id,
                                        name,
                                        pending: state == "true",
                                    })
                                })
                                .collect(),
                        );
                    }
                }
                #[cfg(windows)]
                if let Some(body) = line.splitn(3, ' ').nth(2)
                    && (body.starts_with("RECEIVED ")
                        || (body.starts_with("JOIN REQUEST ")
                            && body.ends_with("sender approval needed"))
                        || (body.starts_with("TUI_JOB ")
                            && body.contains(" dir=RX done=0 ")
                            && body.contains(" state=active ")))
                    && let Ok(mut pending) = thread_notices.lock()
                {
                    if pending.len() == 200 {
                        pending.pop_front();
                    }
                    pending.push_back(body.to_owned());
                }
                if let Ok(mut values) = thread_logs.lock() {
                    values.push(line);
                    if values.len() > 200 {
                        values.remove(2);
                    }
                }
            }
        });
        Ok(Self {
            child,
            logs,
            shared: files.to_vec(),
            ready,
            #[cfg(windows)]
            notices,
            #[cfg(windows)]
            outgoing,
            #[cfg(windows)]
            recipients,
            #[cfg(windows)]
            device_result,
            #[cfg(windows)]
            pairing_pass,
            #[cfg(windows)]
            pair_notice,
            #[cfg(windows)]
            snippets,
            #[cfg(windows)]
            snippet_result,
            #[cfg(windows)]
            pastes,
            #[cfg(windows)]
            paste_result,
            #[cfg(windows)]
            screen_ticket,
        })
    }
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    #[cfg(windows)]
    pub fn outgoing_rows(&self) -> Vec<QueueRow> {
        self.outgoing
            .lock()
            .map(|queue| queue.rows())
            .unwrap_or_default()
    }

    #[cfg(windows)]
    pub fn acknowledge_shares(&mut self) -> Vec<PathBuf> {
        let paths = self
            .outgoing
            .lock()
            .map(|mut queue| queue.take_published())
            .unwrap_or_default();
        for path in &paths {
            if !self.shared.contains(path) {
                self.shared.push(path.clone());
            }
        }
        paths
    }

    #[cfg(windows)]
    pub fn recipients(&self) -> Option<Vec<RecipientDevice>> {
        self.recipients.lock().ok()?.clone()
    }

    #[cfg(windows)]
    pub fn request_recipients(&mut self) -> io::Result<()> {
        *self
            .recipients
            .lock()
            .map_err(|_| io::Error::other("Device list unavailable"))? = None;
        self.control("devices")
    }

    #[cfg(windows)]
    pub fn update_device(&mut self, action: &str, id: &str, name: &str) -> io::Result<()> {
        *self
            .recipients
            .lock()
            .map_err(|_| io::Error::other("Device list unavailable"))? = None;
        self.write_control(action, id, name)
    }

    #[cfg(windows)]
    pub fn take_device_result(&self) -> Option<(String, Result<(), String>)> {
        self.device_result.lock().ok()?.take()
    }

    #[cfg(windows)]
    pub fn take_pair_notice(&self) -> Option<(String, bool)> {
        self.pair_notice.lock().ok()?.take()
    }

    #[cfg(windows)]
    pub fn refresh_pairing_pass(&mut self) -> io::Result<()> {
        *self
            .pairing_pass
            .lock()
            .map_err(|_| io::Error::other("Pairing pass unavailable"))? = None;
        self.control("pairing")
    }

    #[cfg(windows)]
    pub fn pairing_pass(&self) -> Option<String> {
        let latest = self.pairing_url()?;
        let pass = self.pairing_pass.lock().ok()?;
        let (url, pass) = pass.as_ref()?;
        (url == &latest).then(|| pass.clone())
    }

    #[cfg(windows)]
    pub fn snippets(&self) -> Vec<Snippet> {
        self.snippets
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    #[cfg(windows)]
    pub fn take_snippet_result(&self) -> Option<Result<(), String>> {
        self.snippet_result.lock().ok()?.take()
    }

    #[cfg(windows)]
    pub fn pastes(&self) -> Vec<PasteItem> {
        self.pastes
            .lock()
            .map(|items| items.clone())
            .unwrap_or_default()
    }

    #[cfg(windows)]
    pub fn take_paste_result(&self) -> Option<PasteResult> {
        self.paste_result.lock().ok()?.take()
    }

    #[cfg(windows)]
    pub fn change_paste(&mut self, id: &str, action: &str) -> io::Result<()> {
        self.write_control(action, id, "")
    }

    #[cfg(windows)]
    pub fn request_screen(&mut self, peer: &str) -> io::Result<()> {
        self.screen_ticket
            .lock()
            .ok()
            .map(|mut ticket| ticket.take());
        self.write_control("screen-open", peer, "")
    }

    #[cfg(windows)]
    pub fn set_folder_drop(&mut self, peer: &str, folder: &str) -> io::Result<()> {
        self.write_control("folder-drop", peer, folder)
    }

    #[cfg(windows)]
    pub fn reminder(&mut self, action: &str, item: &str, trigger: &str) -> io::Result<()> {
        self.write_control(action, item, trigger)
    }

    #[cfg(windows)]
    pub fn take_screen_ticket(&self) -> Option<Result<ScreenTicket, String>> {
        self.screen_ticket.lock().ok()?.take()
    }

    #[cfg(windows)]
    pub fn close_screen(&mut self) {
        let _ = self.write_control("screen-close", "", "");
    }

    #[cfg(windows)]
    pub fn send_snippet(&mut self, id: &str, text: &str) -> io::Result<()> {
        use std::fmt::Write;
        let mut escaped = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\0'..='\u{001f}' => {
                    let _ = write!(escaped, "\\u{:04x}", ch as u32);
                }
                _ => escaped.push(ch),
            }
        }
        self.write_line(&format!(
            "{{\"Action\":\"snippet\",\"ID\":\"{}\",\"Text\":\"{escaped}\"}}",
            control_quote(id)
        ))
    }

    #[cfg(windows)]
    pub fn offer(&mut self, files: &[PathBuf], id: &str, name: &str) -> io::Result<usize> {
        let files = files
            .iter()
            .map(|path| {
                let value = path
                    .to_str()
                    .filter(|value| !value.chars().any(char::is_control))
                    .ok_or_else(|| io::Error::other("File name contains unsupported characters"))?;
                let metadata = path.metadata()?;
                if !metadata.is_file() {
                    return Err(io::Error::other("Choose a regular file"));
                }
                Ok((path, value, metadata.len()))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let mut queue = self
            .outgoing
            .lock()
            .map_err(|_| io::Error::other("Send queue unavailable"))?;
        let requests: Vec<_> = files
            .into_iter()
            .filter_map(|(path, value, size)| {
                queue
                    .stage_to(path, size, Some((id, name)))
                    .map(|request| (request, value))
            })
            .collect();
        drop(queue);
        if requests.is_empty() {
            return Ok(0);
        }
        let entries = requests
            .iter()
            .map(|(request, value)| {
                format!(
                    "{{\"Request\":{request},\"Path\":\"{}\"}}",
                    control_quote(value)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let command = format!(
            "{{\"Action\":\"offer\",\"ID\":\"{}\",\"Files\":[{entries}]}}",
            control_quote(id)
        );
        let result = if command.len() >= 4 * 1024 * 1024 - 1 {
            Err(io::Error::other("Select fewer files for one offer"))
        } else {
            self.write_line(&command)
        };
        if let Err(error) = result {
            if let Ok(mut queue) = self.outgoing.lock() {
                for (request, _) in &requests {
                    queue.fail(*request, error.to_string());
                }
            }
            return Err(error);
        }
        Ok(requests.len())
    }
    #[cfg(windows)]
    pub fn drain_notices(&self) -> Vec<String> {
        self.notices
            .lock()
            .map(|mut pending| pending.drain(..).collect())
            .unwrap_or_default()
    }
    #[cfg(windows)]
    pub fn exited(&mut self) -> bool {
        self.child.try_wait().is_ok_and(|status| status.is_some())
    }
    pub fn logs(&self) -> Vec<String> {
        self.logs
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
    pub fn control(&mut self, line: &str) -> io::Result<()> {
        let mut parts = line.splitn(3, ' ');
        let action = parts.next().unwrap_or("");
        if ![
            "devices",
            "rename",
            "recipient",
            "approve",
            "deny",
            "revoke",
            "stop",
            "resume",
            "message",
            "pairing",
            "add",
        ]
        .contains(&action)
        {
            return Err(io::Error::other(
                "Commands: pairing | add PATH | message TEXT | stop | resume | devices | approve ID | deny ID | rename ID NAME | recipient ID | revoke ID",
            ));
        }
        let id = parts.next().unwrap_or("");
        let name = if action == "message" || action == "add" {
            line.strip_prefix(&format!("{action} ")).unwrap_or("")
        } else {
            parts.next().unwrap_or("")
        };
        self.write_control(action, id, name)
    }

    fn write_control(&mut self, action: &str, id: &str, name: &str) -> io::Result<()> {
        self.write_line(&format!(
            "{{\"Action\":\"{}\",\"ID\":\"{}\",\"Name\":\"{}\"}}",
            control_quote(action),
            control_quote(id),
            control_quote(name)
        ))
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let input = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::other("Server input closed"))?;
        writeln!(input, "{line}")
    }
    pub fn add_file(&mut self, path: &Path) -> io::Result<()> {
        let value = path
            .to_str()
            .ok_or_else(|| io::Error::other("File name is not valid UTF-8"))?;
        if value.chars().any(char::is_control) {
            return Err(io::Error::other("File name contains a control character"));
        }
        self.control(&format!("add {value}"))?;
        if !self.shared.iter().any(|existing| existing == path) {
            self.shared.push(path.to_path_buf());
        }
        Ok(())
    }
    pub fn shared_files(&self) -> &[PathBuf] {
        &self.shared
    }
    pub fn pairing_url(&self) -> Option<String> {
        self.logs().into_iter().rev().find_map(|line| {
            line.split_once("TapSend pairing URL: ")
                .map(|(_, value)| value.trim().to_string())
        })
    }
    pub fn qr_code(&self) -> Vec<String> {
        static QR_CACHE: Mutex<(String, Vec<String>)> = Mutex::new((String::new(), Vec::new()));
        let Some(url) = self.pairing_url() else {
            return Vec::new();
        };
        let mut cache = QR_CACHE.lock().unwrap();
        if cache.0 == url {
            return cache.1.clone();
        }
        let rows: Vec<String> = server_command()
            .args(["--qr-utf8", &url])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        *cache = (url, rows.clone());
        rows
    }
}
impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
