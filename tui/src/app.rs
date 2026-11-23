use std::{env, io, path::PathBuf, process::Command};

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
    stopped: bool,
    overlay: Option<Overlay>,
    quit_requested: bool,
    last_join_request: String,
}

enum Overlay {
    Help {
        scroll: usize,
    },
    Command {
        input: String,
        selected: usize,
    },
    Settings {
        values: [String; 6],
        selected: usize,
        edit: Option<String>,
        error: String,
        dirty: bool,
        confirm_exit: bool,
    },
}

const SETTING_KEYS: [&str; 6] = [
    "inbox",
    "start_folder",
    "show_hidden",
    "listen",
    "pairing_host",
    "direct_joins",
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
            stopped: false,
            overlay: None,
            quit_requested: false,
            last_join_request: String::new(),
        })
    }

    pub fn run(mut self) -> io::Result<()> {
        let terminal = RawTerminal::enter()?;
        loop {
            if self.quit_requested {
                break;
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
                    self.message =
                        "New phone waiting: press D, then ; approve ID or ; deny ID".into();
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
                self.message = "Phone connected. Browse and press A to add another file".into();
            }
            let overlay = match &self.overlay {
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
                }) => Some(ui::OverlayView::Settings {
                    values,
                    selected: *selected,
                    edit: edit.as_deref(),
                    error,
                    dirty: *dirty,
                    confirm_exit: *confirm_exit,
                }),
                None => None,
            };
            ui::draw(
                &self.explorer,
                self.server.as_ref(),
                &self.message,
                self.pairing_view,
                self.stopped,
                self.device_view,
                overlay,
            )?;
            if self.overlay.is_some() {
                self.handle_modal(terminal.read_modal_key()?)?;
                continue;
            }
            match terminal.read_key()? {
                Key::Quit => break,
                Key::Up if !self.device_view => self.explorer.up(),
                Key::Down if !self.device_view => self.explorer.down(),
                Key::Enter if !self.device_view => {
                    let result = self.explorer.open();
                    self.update(result);
                }
                Key::Back if !self.device_view => {
                    let result = self.explorer.parent();
                    self.update(result);
                }
                Key::Space if !self.device_view => self.explorer.toggle(),
                Key::Hidden if !self.device_view => {
                    let result = self.explorer.toggle_hidden();
                    self.update(result);
                }
                Key::Start if self.server.is_none() && !self.device_view => self.start_server(),
                Key::Add if self.server.is_some() && !self.device_view => self.add_files(),
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
                Key::Files => self.device_view = false,
                Key::Devices => {
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
        }
        Ok(())
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

    fn handle_modal(&mut self, key: ModalKey) -> io::Result<()> {
        let Some(overlay) = self.overlay.take() else {
            return Ok(());
        };
        match overlay {
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
                        ModalKey::Escape if !dirty || confirm_exit => return Ok(()),
                        ModalKey::Escape => {
                            confirm_exit = true;
                            error = "Unsaved changes. Press Esc again to discard.".into();
                        }
                        ModalKey::Up => selected = selected.saturating_sub(1),
                        ModalKey::Down | ModalKey::Tab => selected = (selected + 1) % values.len(),
                        ModalKey::Enter => {
                            if selected == 2 {
                                values[2] =
                                    if values[2] == "true" { "false" } else { "true" }.into();
                                dirty = true;
                            } else {
                                edit = Some(values[selected].clone());
                            }
                            error.clear();
                        }
                        ModalKey::Char('s') | ModalKey::Char('S') => {
                            match self.save_settings(&values) {
                                Ok(()) => return Ok(()),
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
            "files" => self.device_view = false,
            "devices" => {
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
        match Self::load_settings("show") {
            Ok(values) => {
                self.overlay = Some(Overlay::Settings {
                    values,
                    selected: 0,
                    edit: None,
                    error: String::new(),
                    dirty: false,
                    confirm_exit: false,
                })
            }
            Err(error) => self.message = error,
        }
    }

    fn load_settings(mode: &str) -> Result<[String; 6], String> {
        let output = Command::new(Self::launcher())
            .args(["settings", mode])
            .output()
            .map_err(|error| format!("Could not load settings: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut values: [String; 6] = std::array::from_fn(|_| String::new());
        let mut found = [false; 6];
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

    fn save_settings(&mut self, values: &[String; 6]) -> Result<(), String> {
        let output = Command::new(Self::launcher())
            .args([
                "settings",
                "set",
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
            ])
            .arg(format!("--show-hidden={}", values[2]))
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
}
