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
        inbox: String,
        listen: String,
        selected: usize,
        edit: Option<String>,
    },
}

impl App {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            explorer: Explorer::new(env::current_dir()?)?,
            server: None,
            message: "Focus a file and press S, or mark several with Space".into(),
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
                    inbox,
                    listen,
                    selected,
                    edit,
                }) => Some(ui::OverlayView::Settings {
                    inbox,
                    listen,
                    selected: *selected,
                    edit: edit.as_deref(),
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
        if files.is_empty() {
            self.message = "Focus a file or select files with Space".into();
            return;
        }
        match ServerProcess::start(&files) {
            Ok(server) => {
                self.server = Some(server);
                self.explorer.clear_selection();
                self.pairing_view = 1;
                self.auto_dashboard = true;
                self.message =
                    "Sharing is live. A saved phone can reconnect without pairing again".into();
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
                mut inbox,
                mut listen,
                mut selected,
                mut edit,
            } => {
                if let Some(value) = edit.as_mut() {
                    match key {
                        ModalKey::Escape => edit = None,
                        ModalKey::Backspace => {
                            value.pop();
                        }
                        ModalKey::Char(ch) if value.len() < 1024 => value.push(ch),
                        ModalKey::Enter => {
                            if selected == 0 {
                                inbox = value.trim().to_string();
                            } else {
                                listen = value.trim().to_string();
                            }
                            edit = None;
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        ModalKey::Escape => return Ok(()),
                        ModalKey::Up => selected = selected.saturating_sub(1),
                        ModalKey::Down => selected = (selected + 1).min(1),
                        ModalKey::Enter => {
                            edit = Some(if selected == 0 {
                                inbox.clone()
                            } else {
                                listen.clone()
                            })
                        }
                        ModalKey::Char('s') | ModalKey::Char('S') => {
                            self.save_settings(&inbox, &listen);
                            if self.message.starts_with("Settings saved") {
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                }
                self.overlay = Some(Overlay::Settings {
                    inbox,
                    listen,
                    selected,
                    edit,
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
        match Command::new(Self::launcher())
            .args(["settings", "show"])
            .output()
        {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let mut inbox = String::new();
                let mut listen = String::new();
                for line in text.lines() {
                    if let Some(value) = line.strip_prefix("inbox\t") {
                        inbox = value.into();
                    }
                    if let Some(value) = line.strip_prefix("listen\t") {
                        listen = value.into();
                    }
                }
                self.overlay = Some(Overlay::Settings {
                    inbox,
                    listen,
                    selected: 0,
                    edit: None,
                });
            }
            Ok(output) => self.message = String::from_utf8_lossy(&output.stderr).trim().into(),
            Err(error) => self.message = format!("Could not open settings: {error}"),
        }
    }

    fn save_settings(&mut self, inbox: &str, listen: &str) {
        match Command::new(Self::launcher())
            .args(["settings", "set", "--inbox", inbox, "--listen", listen])
            .output()
        {
            Ok(output) if output.status.success() => {
                self.message = "Settings saved. Restart the share to apply them.".into()
            }
            Ok(output) => self.message = String::from_utf8_lossy(&output.stderr).trim().into(),
            Err(error) => self.message = format!("Could not save settings: {error}"),
        }
    }
}
