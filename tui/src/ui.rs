use crate::{explorer::Explorer, server::ServerProcess};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
    sync::Mutex,
};

const NORMAL: &str = "\x1b[40m\x1b[97m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const INVERSE: &str = "\x1b[47m\x1b[30m";
const RESET: &str = "\x1b[0m";

pub struct CommandSpec {
    pub name: &'static str,
    pub detail: &'static str,
    pub argument: bool,
    pub live_only: bool,
}

const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "settings",
        detail: "Change inbox and connection settings",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "share",
        detail: "Share focused or marked files",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "receive",
        detail: "Start phone uploads without sharing a file",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "add",
        detail: "Add focused files or enter a path",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "files",
        detail: "Show the file browser",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "devices",
        detail: "Show saved phones",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "pairing",
        detail: "Show QR and manual pairing",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "new-code",
        detail: "Generate a new pairing code",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "pause",
        detail: "Pause computer downloads",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "resume",
        detail: "Resume computer downloads",
        argument: false,
        live_only: true,
    },
    CommandSpec {
        name: "message",
        detail: "Send a message to the phone",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "rename",
        detail: "Rename a device: ID NAME",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "revoke",
        detail: "Forget a device: ID",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "recipient",
        detail: "Select a device: ID",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "approve",
        detail: "Allow a waiting device: ID",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "deny",
        detail: "Reject a waiting device: ID",
        argument: true,
        live_only: true,
    },
    CommandSpec {
        name: "hidden",
        detail: "Show or hide dotfiles",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "help",
        detail: "Open the key guide",
        argument: false,
        live_only: false,
    },
    CommandSpec {
        name: "quit",
        detail: "Close TapSend",
        argument: false,
        live_only: false,
    },
];

pub fn command_suggestions(query: &str, live: bool) -> Vec<&'static CommandSpec> {
    let word = query
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(|command| !command.live_only || live)
        .filter(|command| {
            word.is_empty()
                || command.name.contains(&word)
                || command.detail.to_ascii_lowercase().contains(&word)
        })
        .collect()
}

pub enum OverlayView<'a> {
    Help {
        scroll: usize,
    },
    Command {
        input: &'a str,
        selected: usize,
        suggestions: Vec<&'static CommandSpec>,
    },
    Settings {
        inbox: &'a str,
        listen: &'a str,
        selected: usize,
        edit: Option<&'a str>,
    },
}

const HELP_LINES: &[&str] = &[
    "FILES & NAVIGATION",
    " ↑ / K       Move up in the file browser",
    " ↓ / J       Move down in the file browser",
    " Enter / L   Open a folder",
    " Backspace   Go to parent folder",
    " Space       Mark or unmark a file",
    " F           Show files in the left column",
    " D           Show saved devices",
    " H           Show or hide dotfiles",
    " G           Open settings",
    "",
    "SHARING & PAIRING",
    " S           Share focused files or receive when none is focused",
    " ; receive   Start receive-only mode from any folder",
    " A           Add files while sharing",
    " P           QR → manual details → dashboard",
    " R           Create a new pairing code",
    " D           Show devices and join requests",
    " T           Pause or resume downloads",
    "",
    "COMMANDS & SESSION",
    " ; or :      Open the command popup",
    " ?           Open or close this guide",
    " ↑ / ↓       Select a command suggestion",
    " Tab         Complete a command",
    " Enter       Run the command",
    " Esc         Close a popup",
    " Q           Quit TapSend",
    "",
    "COMMAND EXAMPLES",
    " message Hello from the computer",
    " rename DEVICE_ID New phone name",
    " revoke DEVICE_ID",
    " approve DEVICE_ID / deny DEVICE_ID",
];

pub fn help_line_count() -> usize {
    HELP_LINES.len()
}

pub fn draw(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing_view: u8,
    stopped: bool,
    device_view: bool,
    overlay: Option<OverlayView<'_>>,
) -> io::Result<()> {
    let (columns, height) = terminal_size();
    let width = columns.saturating_sub(1).max(1);
    let height = height.max(1);
    if width >= 112 && height >= 34 {
        return draw_reference_layout(
            explorer,
            server,
            message,
            pairing_view,
            stopped,
            device_view,
            overlay,
            width,
            height,
        );
    }
    let live = server.is_some();
    let shared = server.map(|s| s.shared_files().len()).unwrap_or(0);
    let mut lines = Vec::with_capacity(height);
    lines.push(format!(
        "{INVERSE}{BOLD} TAPSEND {NORMAL}  {}  {DIM}{} shared / {} marked{NORMAL}",
        if stopped {
            "Ⅱ PAUSED"
        } else if live {
            "● LIVE"
        } else {
            "○ READY"
        },
        shared,
        explorer.selected_count()
    ));
    let compact_pairing = pairing_view == 1 && live && height <= 26;
    if !compact_pairing {
        lines.push(format!(
            "{BOLD}NETWORK{NORMAL}  {}    {BOLD}FLOW{NORMAL}  {}    {BOLD}ACTION{NORMAL}  {}",
            server
                .and_then(ServerProcess::pairing_url)
                .and_then(|url| url.strip_prefix("https://").map(|value| value
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .to_string()))
                .unwrap_or_else(|| "LOCAL / OFFLINE".into()),
            server
                .map(|s| transfer_summary(&s.logs()))
                .unwrap_or_else(|| "READY TO SHARE".into()),
            if live {
                "BROWSE + ADD"
            } else {
                "CHOOSE + START"
            }
        ));
        lines.push(format!(
            "{DIM}{} {NORMAL}{}",
            "─".repeat(3),
            explorer.directory().display()
        ));
    }

    let body_height = height.saturating_sub(if compact_pairing { 4 } else { 6 });
    if width >= 118 && pairing_view == 0 {
        let left_width = width * 38 / 100;
        let center_width = width * 32 / 100;
        let right_width = width.saturating_sub(left_width + center_width + 2);
        let left = panel(
            "01  LIBRARY / SPACE TO MARK",
            left_width,
            body_height,
            &if device_view {
                device_rows(
                    server,
                    left_width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            } else {
                file_rows(
                    explorer,
                    left_width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            },
        );
        let center_content = if let Some(server) = server {
            session_rows(
                server,
                stopped,
                center_width.saturating_sub(4),
                body_height.saturating_sub(2),
                false,
            )
        } else {
            ready_rows(
                explorer,
                center_width.saturating_sub(4),
                body_height.saturating_sub(2),
            )
        };
        let center = panel(
            if live {
                "02  TRANSFER DECK"
            } else {
                "02  QUICK START"
            },
            center_width,
            body_height,
            &center_content,
        );
        let right_content = if let Some(server) = server {
            activity_rows(
                server,
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
            )
        } else {
            inspector_rows(
                explorer,
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
            )
        };
        let right = panel(
            if live {
                "03  ACTIVITY"
            } else {
                "03  INSPECTOR"
            },
            right_width,
            body_height,
            &right_content,
        );
        for row in 0..body_height {
            lines.push(format!(
                "{} {} {}",
                left.get(row).map(String::as_str).unwrap_or(""),
                center.get(row).map(String::as_str).unwrap_or(""),
                right.get(row).map(String::as_str).unwrap_or("")
            ));
        }
    } else if width >= 78 {
        let left_width = width * 48 / 100;
        let right_width = width.saturating_sub(left_width + 1);
        let left = panel(
            "01  LIBRARY / SPACE TO MARK",
            left_width,
            body_height,
            &if device_view {
                device_rows(
                    server,
                    left_width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            } else {
                file_rows(
                    explorer,
                    left_width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            },
        );
        let right_content = if pairing_view != 0 && live {
            pairing_rows(
                server.unwrap(),
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
                pairing_view == 2,
            )
        } else if let Some(server) = server {
            session_rows(
                server,
                stopped,
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
                true,
            )
        } else {
            ready_rows(
                explorer,
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
            )
        };
        let right_title = if pairing_view != 0 && live {
            "02  PAIR / SCAN OR TYPE"
        } else if live {
            "02  TRANSFER DECK"
        } else {
            "02  QUICK START"
        };
        let right = panel(right_title, right_width, body_height, &right_content);
        for row in 0..body_height {
            lines.push(format!(
                "{} {}",
                left.get(row).map(String::as_str).unwrap_or(""),
                right.get(row).map(String::as_str).unwrap_or("")
            ));
        }
    } else {
        lines.extend(panel(
            "01  LIBRARY / WIDEN FOR DASHBOARD",
            width,
            body_height,
            &if device_view {
                device_rows(
                    server,
                    width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            } else {
                file_rows(
                    explorer,
                    width.saturating_sub(4),
                    body_height.saturating_sub(2),
                )
            },
        ));
    }

    lines.push(format!("{INVERSE} STATUS {NORMAL} {message}"));
    lines.push(if live {
        " [SPACE] mark   [A] add now   [P] pair / details / deck   [R] new code   [T] pause".into()
    } else {
        " [SPACE] mark   [S] share or receive   [ENTER] open   [BACKSPACE] up".into()
    });
    lines.push(" [↑↓/JK] browse   [G] settings   [;] commands   [?] help   [Q] quit".into());

    paint_frame(&lines, width, height, overlay)
}

fn paint_frame(
    lines: &[String],
    width: usize,
    height: usize,
    overlay: Option<OverlayView<'_>>,
) -> io::Result<()> {
    static LAST: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    let mut frame = Vec::new();
    for row in 0..height {
        let line = lines.get(row).map(String::as_str).unwrap_or("");
        write!(
            frame,
            "\x1b[{};1H{NORMAL}{}{RESET}\x1b[K",
            row + 1,
            padded(line, width)
        )?;
    }
    if let Some(overlay) = overlay {
        let (left, top, popup_width, popup) = overlay_popup(overlay, width, height);
        for (row, line) in popup.iter().enumerate() {
            write!(
                frame,
                "\x1b[{};{}H{NORMAL}{}{RESET}",
                top + row + 1,
                left + 1,
                padded(line, popup_width)
            )?;
        }
    }
    let mut last = LAST.lock().unwrap();
    if *last != frame {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&frame)?;
        stdout.flush()?;
        *last = frame;
    }
    Ok(())
}

fn draw_reference_layout(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing_view: u8,
    stopped: bool,
    device_view: bool,
    overlay: Option<OverlayView<'_>>,
    width: usize,
    height: usize,
) -> io::Result<()> {
    let live = server.is_some();
    let shared = server.map(|s| s.shared_files().len()).unwrap_or(0);
    let address = server
        .and_then(ServerProcess::pairing_url)
        .and_then(|url| {
            url.strip_prefix("https://")
                .map(|rest| rest.split('/').next().unwrap_or("").to_string())
        })
        .unwrap_or_else(|| "LOCAL / OFFLINE".into());
    let flow = server
        .map(|s| transfer_summary(&s.logs()))
        .unwrap_or_else(|| "READY TO SHARE".into());
    let mut lines = vec![
        format!(
            "{INVERSE}{BOLD} TAPSEND {NORMAL}  {}   {} shared / {} marked    {DIM}LOCAL NETWORK{NORMAL}",
            if stopped {
                "Ⅱ PAUSED"
            } else if live {
                "● LIVE"
            } else {
                "○ READY"
            },
            shared,
            explorer.selected_count()
        ),
        format!(
            "{BOLD}NETWORK{NORMAL}  {address}    {BOLD}FLOW{NORMAL}  {flow}    {BOLD}ACTION{NORMAL}  {}",
            if pairing_view != 0 {
                "PAIRING"
            } else if live {
                "BROWSE + ADD"
            } else {
                "CHOOSE + START"
            }
        ),
        format!("{DIM}───{NORMAL} {}", explorer.directory().display()),
    ];
    let body_height = height.saturating_sub(6);
    let left_width = width * 29 / 100;
    let right_width = width.saturating_sub(left_width + 1);
    let left_content = if device_view {
        device_rows(
            server,
            left_width.saturating_sub(4),
            body_height.saturating_sub(2),
        )
    } else {
        file_rows(
            explorer,
            left_width.saturating_sub(4),
            body_height.saturating_sub(2),
        )
    };
    let left = panel(
        if device_view {
            "FILES [F]  │  DEVICES [D] ◀"
        } else {
            "FILES [F] ◀  │  DEVICES [D]"
        },
        left_width,
        body_height,
        &left_content,
    );
    let right = if pairing_view != 0 {
        panel(
            "QR + PAIRING [P]  /  SCAN OR ENTER DETAILS",
            right_width,
            body_height,
            &pairing_rows(
                server.unwrap(),
                right_width.saturating_sub(4),
                body_height.saturating_sub(2),
                pairing_view == 2,
            ),
        )
    } else {
        let top_height = (body_height * 45 / 100).max(12);
        let middle_height = (body_height * 30 / 100).max(8);
        let log_height = body_height.saturating_sub(top_height + middle_height + 2);
        let mut sections = panel(
            if live {
                "DASHBOARD  /  STREAMS 1—4                          [P] QR + PAIRING"
            } else {
                "DASHBOARD  /  STREAMS 1—4                          START TO ENABLE PAIRING"
            },
            right_width,
            top_height,
            &dashboard_rows(
                server,
                stopped,
                right_width.saturating_sub(4),
                top_height.saturating_sub(2),
            ),
        );
        sections.push(" ".repeat(right_width));
        let shared_width = right_width * 43 / 100;
        let messages_width = right_width.saturating_sub(shared_width + 1);
        let shared_panel = panel(
            "SHARED FILES",
            shared_width,
            middle_height,
            &shared_rows(
                server,
                shared_width.saturating_sub(4),
                middle_height.saturating_sub(2),
            ),
        );
        let messages_panel = panel(
            "MESSAGES  /  : MESSAGE TEXT",
            messages_width,
            middle_height,
            &message_rows(
                server,
                messages_width.saturating_sub(4),
                middle_height.saturating_sub(2),
            ),
        );
        for row in 0..middle_height {
            sections.push(format!(
                "{} {}",
                shared_panel.get(row).map(String::as_str).unwrap_or(""),
                messages_panel.get(row).map(String::as_str).unwrap_or("")
            ));
        }
        sections.push(" ".repeat(right_width));
        sections.extend(panel(
            "CONNECTION LOGS",
            right_width,
            log_height,
            &connection_log_rows(server, log_height.saturating_sub(2)),
        ));
        sections
    };
    for row in 0..body_height {
        lines.push(format!(
            "{} {}",
            left.get(row).map(String::as_str).unwrap_or(""),
            right.get(row).map(String::as_str).unwrap_or("")
        ));
    }
    lines.push(format!("{INVERSE} STATUS {NORMAL} {message}"));
    lines.push(if live {
        " [F] files  [D] devices  [SPACE] mark  [A] add  [P] QR / details / dashboard  [R] new code  [T] pause".into()
    } else {
        " [F] files  [D] devices  [SPACE] mark  [S] share/receive  [ENTER] open  [BACKSPACE] up".into()
    });
    lines.push(" [↑↓/JK] browse  [G] settings  [;] commands  [?] help  [Q] quit".into());
    paint_frame(&lines, width, height, overlay)
}

fn overlay_popup(
    overlay: OverlayView<'_>,
    width: usize,
    height: usize,
) -> (usize, usize, usize, Vec<String>) {
    let popup_width = width.saturating_sub(4).min(72).max(4).min(width);
    let left = width.saturating_sub(popup_width) / 2;
    match overlay {
        OverlayView::Help { scroll } => {
            let popup_height = (HELP_LINES.len() + 3)
                .min(height.saturating_sub(4))
                .max(3)
                .min(height);
            let top = height.saturating_sub(popup_height) / 2;
            let visible = popup_height.saturating_sub(3);
            let max_scroll = HELP_LINES.len().saturating_sub(visible);
            let mut content = HELP_LINES
                .iter()
                .skip(scroll.min(max_scroll))
                .take(visible)
                .map(|line| {
                    if !line.starts_with(' ') && !line.is_empty() {
                        format!("{BOLD}{line}{NORMAL}")
                    } else {
                        (*line).to_string()
                    }
                })
                .collect::<Vec<_>>();
            content.resize(visible, String::new());
            content.push(format!(
                "{DIM}ESC close   ↑↓ / J K scroll   ? close{NORMAL}"
            ));
            (
                left,
                top,
                popup_width,
                panel("KEYBOARD MAP  /  ?", popup_width, popup_height, &content),
            )
        }
        OverlayView::Command {
            input,
            selected,
            suggestions,
        } => {
            let popup_height = (suggestions.len().min(8) + 6)
                .min(height.saturating_sub(4))
                .max(6)
                .min(height);
            let top = height.saturating_sub(popup_height + 2);
            let visible = popup_height.saturating_sub(6);
            let start = selected.saturating_sub(visible.saturating_sub(1));
            let mut content = vec![
                format!("{BOLD}❯{NORMAL} {input}▏"),
                String::new(),
                format!("{DIM}COMMANDS  /  {} matching{NORMAL}", suggestions.len()),
            ];
            if suggestions.is_empty() {
                content.push("No matching command".into());
            }
            for (index, item) in suggestions.iter().enumerate().skip(start).take(visible) {
                let line = format!(" {:<13}  {}", item.name, item.detail);
                content.push(if index == selected {
                    format!("{INVERSE}{line}{NORMAL}")
                } else {
                    line
                });
            }
            content.resize(popup_height.saturating_sub(3), String::new());
            content.push(format!(
                "{DIM}ESC close   ↑↓ select   TAB fill   ENTER run{NORMAL}"
            ));
            (
                left,
                top,
                popup_width,
                panel("CMDLINE  /  ;", popup_width, popup_height, &content),
            )
        }
        OverlayView::Settings {
            inbox,
            listen,
            selected,
            edit,
        } => {
            let popup_height = 10.min(height);
            let top = height.saturating_sub(popup_height) / 2;
            let row = |index: usize, label: &str, value: &str| {
                let shown = if selected == index {
                    edit.unwrap_or(value)
                } else {
                    value
                };
                let line = format!(
                    "{label:<15} {}",
                    crop(shown, popup_width.saturating_sub(22))
                );
                if selected == index {
                    format!("{INVERSE}{line}{NORMAL}")
                } else {
                    line
                }
            };
            let content = vec![
                "COMPUTER / TRANSFERS + CONNECTION".into(),
                String::new(),
                row(0, "Inbox folder", inbox),
                row(1, "Listen address", listen),
                String::new(),
                if edit.is_some() {
                    "ENTER accept value   ESC cancel edit".into()
                } else {
                    "↑↓ choose   ENTER edit   S save   ESC close".into()
                },
            ];
            (
                left,
                top,
                popup_width,
                panel("SETTINGS  /  G", popup_width, popup_height, &content),
            )
        }
    }
}

fn device_rows(server: Option<&ServerProcess>, _width: usize, height: usize) -> Vec<String> {
    let mut rows = vec![
        format!("{INVERSE}  DEVICES + JOIN REQUESTS  {NORMAL}"),
        String::new(),
    ];
    let Some(server) = server else {
        rows.push("Start sharing to view paired phones".into());
        rows.push("Press F to return to files".into());
        return rows;
    };
    let logs = server.logs();
    let pending = pending_from_logs(&logs);
    rows.push(format!(
        "{BOLD}WAITING FOR APPROVAL  /  {}{NORMAL}",
        pending.len()
    ));
    for (id, name) in pending.iter().take(3) {
        rows.push(format!("! {}  {}", crop(id, 8), crop(name, 22)));
    }
    rows.push("; approve ID  or  ; deny ID".into());
    rows.push(String::new());
    rows.push(format!("{BOLD}SAVED PHONES{NORMAL}"));
    let devices = devices_from_logs(&logs);
    if devices.is_empty() {
        rows.push("No saved phones yet".into());
    }
    for (id, name) in devices
        .into_iter()
        .take(height.saturating_sub(rows.len() + 3) / 3)
    {
        rows.push(format!("● {name}"));
        rows.push(format!("  ID {}", crop(&id, 12)));
        rows.push(String::new());
    }
    rows.push(format!("{DIM}Press D to refresh the list{NORMAL}"));
    rows.push("Use ; rename ID NAME or ; revoke ID".into());
    rows.push("Press F to browse files".into());
    rows
}

fn pending_from_logs(logs: &[String]) -> Vec<(String, String)> {
    let Some(end) = logs
        .iter()
        .rposition(|line| line.contains("CONTROL devices applied"))
    else {
        return Vec::new();
    };
    let start = logs[..end]
        .iter()
        .rposition(|line| line.contains("CONTROL devices applied"))
        .map(|index| index + 1)
        .unwrap_or(0);
    logs[start..end]
        .iter()
        .filter_map(|line| {
            let (_, value) = line.split_once("JOIN REQUEST ")?;
            let (id, rest) = value.split_once(' ')?;
            let name = rest.split('"').nth(1)?;
            Some((id.to_string(), name.to_string()))
        })
        .collect()
}

fn devices_from_logs(logs: &[String]) -> Vec<(String, String)> {
    let Some(end) = logs
        .iter()
        .rposition(|line| line.contains("CONTROL devices applied"))
    else {
        return Vec::new();
    };
    let start = logs[..end]
        .iter()
        .rposition(|line| line.contains("CONTROL devices applied"))
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut seen = HashSet::new();
    let mut devices = Vec::new();
    for line in logs[start..end].iter().rev() {
        if let Some(data) = line.split_once("DEVICE ").map(|(_, value)| value) {
            if let Some((id, name)) = data.split_once(' ') {
                if seen.insert(id.to_string()) {
                    devices.push((id.to_string(), name.trim_matches('"').to_string()));
                }
            }
        }
    }
    devices
}

fn dashboard_rows(
    server: Option<&ServerProcess>,
    stopped: bool,
    width: usize,
    height: usize,
) -> Vec<String> {
    let logs = server.map(ServerProcess::logs).unwrap_or_default();
    let mut rows = vec![format!(
        "{BOLD}STREAMS 01—04{NORMAL}  │  {}",
        if stopped {
            "Ⅱ TRANSFERS PAUSED"
        } else if server.is_some() {
            "● LOCAL HTTPS ACTIVE"
        } else {
            "○ START A SHARE TO GO LIVE"
        }
    )];
    let graph_width = width * 68 / 100;
    let stats_width = width.saturating_sub(graph_width + 3);
    rows.push(format!(
        "{}┬{}",
        "─".repeat(graph_width + 1),
        "─".repeat(stats_width + 1)
    ));
    rows.push(format!(
        "{BOLD}{:<graph_width$}{NORMAL} │ {BOLD}CONNECTION DATA{NORMAL}",
        "CONNECTION + TRANSFERS"
    ));
    let graph = graph_rows(&logs, graph_width, height.saturating_sub(3));
    let stats = connection_data_rows(&logs, server.map(|s| s.shared_files().len()).unwrap_or(0));
    for row in 0..height.saturating_sub(3) {
        rows.push(format!(
            "{} │ {}",
            padded(
                graph.get(row).map(String::as_str).unwrap_or(""),
                graph_width
            ),
            padded(
                stats.get(row).map(String::as_str).unwrap_or(""),
                stats_width
            )
        ));
    }
    rows
}

fn graph_rows(logs: &[String], width: usize, height: usize) -> Vec<String> {
    let total = send_samples(logs);
    let mut rows = vec![format!(
        "{BOLD}TOTAL{NORMAL}  {:>12}  {}",
        rate_text(*total.last().unwrap_or(&0.0)),
        sparkline(&total, width.saturating_sub(22))
    )];
    if height >= 8 {
        rows.push(format!("{DIM}{}{NORMAL}", "─".repeat(width)));
    }
    for lane in 1..=4 {
        let samples = lane_samples(logs, lane);
        rows.push(format!(
            "{BOLD}  0{lane}{NORMAL}   {:>12}  {}",
            rate_text(*samples.last().unwrap_or(&0.0)),
            sparkline(&samples, width.saturating_sub(22))
        ));
        if height >= 12 {
            rows.push(String::new());
        }
    }
    let latency = rtt_samples(logs);
    rows.push(format!(
        "{BOLD}PING{NORMAL}  {:>12}  {}",
        latency
            .last()
            .map(|value| format!("{value:.0} ms"))
            .unwrap_or_else(|| "—".into()),
        sparkline(&latency, width.saturating_sub(22))
    ));
    if total.is_empty() {
        rows.push(format!("{DIM}Waiting for the first transfer{NORMAL}"));
    }
    rows
}

fn lane_samples(logs: &[String], lane: usize) -> Vec<f64> {
    let marker = format!("STREAM lane={lane} ");
    logs.iter()
        .filter(|line| line.contains(&marker))
        .filter_map(|line| parse_metric(line, "speed="))
        .rev()
        .take(36)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn rtt_samples(logs: &[String]) -> Vec<f64> {
    logs.iter()
        .filter(|line| line.contains("HEALTH phone="))
        .filter_map(|line| parse_number_field(line, "rtt=", "ms"))
        .rev()
        .take(36)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn connection_data_rows(logs: &[String], shared: usize) -> Vec<String> {
    let authenticated = logs
        .iter()
        .any(|line| line.contains("CONNECTED authenticated device"));
    let heartbeat = logs
        .iter()
        .rev()
        .find(|line| line.contains("HEALTH phone="));
    let rtt = heartbeat.and_then(|line| parse_number_field(line, "rtt=", "ms"));
    let phone_rate = heartbeat.and_then(|line| parse_metric(line, "download="));
    let send_rate = send_samples(logs).last().copied();
    vec![
        format!(
            "{}",
            if authenticated {
                "● AUTHENTICATED"
            } else {
                "○ WAITING FOR PHONE"
            }
        ),
        String::new(),
        format!("LAST TX {}", rate_text(send_rate.unwrap_or(0.0))),
        format!(
            "LAST RX {}",
            phone_rate.map(rate_text).unwrap_or_else(|| "—".into())
        ),
        format!(
            "LATENCY {}",
            rtt.map(|n| format!("{n:.0} ms"))
                .unwrap_or_else(|| "—".into())
        ),
        format!("FILES  {shared}"),
    ]
}

fn parse_number_field(line: &str, field: &str, suffix: &str) -> Option<f64> {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(field))
        .and_then(|value| value.strip_suffix(suffix))
        .and_then(|value| value.parse::<f64>().ok())
}

fn shared_rows(server: Option<&ServerProcess>, width: usize, height: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let Some(server) = server else {
        rows.push("No files shared yet".into());
        rows.push("Focus a file and press S".into());
        return rows;
    };
    rows.push(format!(
        "{BOLD}{:02} AVAILABLE ON PHONE{NORMAL}",
        server.shared_files().len()
    ));
    for path in server
        .shared_files()
        .iter()
        .rev()
        .take(height.saturating_sub(2))
    {
        let name = crop(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            width.saturating_sub(14),
        );
        let size = fs::metadata(path)
            .map(|info| human_size(info.len()))
            .unwrap_or_else(|_| "?".into());
        rows.push(format!(
            "◆ {:<name_width$} {:>7}",
            name,
            crop(&size, 7),
            name_width = width.saturating_sub(12)
        ));
    }
    rows.push(format!("{DIM}Browse left + press A to add another{NORMAL}"));
    rows
}

fn message_rows(server: Option<&ServerProcess>, _width: usize, height: usize) -> Vec<String> {
    let mut rows = vec![format!("{DIM}Phone ⇄ computer{NORMAL}")];
    let Some(server) = server else {
        rows.push("Messages appear after sharing starts".into());
        return rows;
    };
    let mut found = 0;
    for line in server.logs().iter().rev() {
        let message = line
            .split_once("MESSAGE phone: ")
            .map(|(_, value)| ("PHONE", value))
            .or_else(|| {
                line.split_once("MESSAGE computer: ")
                    .map(|(_, value)| ("YOU", value))
            })
            .or_else(|| {
                line.split_once("CHAT phone: ")
                    .map(|(_, value)| ("PHONE", value))
            });
        if let Some((from, content)) = message {
            rows.push(format!("{BOLD}{from:>5}{NORMAL}  {content}"));
            found += 1;
            if rows.len() >= height {
                break;
            }
        }
    }
    if found == 0 {
        rows.push("No messages yet".into());
    }
    rows
}

fn connection_log_rows(server: Option<&ServerProcess>, height: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let Some(server) = server else {
        rows.push("Start sharing to open the local connection".into());
        return rows;
    };
    let logs = server.logs();
    let feed = events(&logs);
    if feed.is_empty() {
        rows.push("Listening for connection events…".into());
    } else {
        rows.extend(feed.into_iter().take(height));
    }
    rows
}

fn panel(title: &str, width: usize, height: usize, content: &[String]) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    if width < 4 {
        return vec![String::new(); height];
    }
    let title = crop(title, width.saturating_sub(5));
    let border_len = width.saturating_sub(title.chars().count() + 5);
    let mut rows = vec![format!(
        "┏━ {BOLD}{title}{NORMAL} {}┓",
        "━".repeat(border_len)
    )];
    for index in 0..height.saturating_sub(2) {
        let line = content.get(index).map(String::as_str).unwrap_or("");
        rows.push(format!("┃ {} ┃", padded(line, width.saturating_sub(4))));
    }
    if height >= 2 {
        rows.push(format!("┗{}┛", "━".repeat(width - 2)));
    }
    rows
}

fn file_rows(explorer: &Explorer, width: usize, height: usize) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    let mut rows = vec![format!(
        "{DIM}{}  FILES    {} MARKED{NORMAL}",
        explorer.entries().len(),
        explorer.selected_count()
    )];
    let list_height = height.saturating_sub(4);
    let start = explorer.cursor().saturating_sub(list_height / 2);
    let marked = explorer.selected_files();
    let folders = explorer
        .entries()
        .iter()
        .filter(|entry| entry.directory)
        .count();
    for index in start..start + list_height {
        let Some(entry) = explorer.entries().get(index) else {
            let past = index.saturating_sub(explorer.entries().len());
            rows.push(if past == 1 {
                format!(
                    "{DIM}── {} ──{NORMAL}",
                    if marked.is_empty() {
                        "FOLDER SUMMARY"
                    } else {
                        "MARKED FILES"
                    }
                )
            } else if past >= 2 {
                if marked.is_empty() {
                    if past == 2 {
                        format!(
                            " {folders} folders  /  {} files",
                            explorer.entries().len().saturating_sub(folders)
                        )
                    } else {
                        String::new()
                    }
                } else {
                    marked
                        .get(past - 2)
                        .map(|path| {
                            format!(
                                " ◆ {}",
                                crop(
                                    &path.file_name().unwrap_or_default().to_string_lossy(),
                                    width.saturating_sub(4)
                                )
                            )
                        })
                        .unwrap_or_default()
                }
            } else {
                String::new()
            });
            continue;
        };
        let symbol = if entry.directory {
            "▸"
        } else if explorer.selected(&entry.path) {
            "◆"
        } else {
            "·"
        };
        let size = if entry.directory {
            "DIR".into()
        } else {
            fs::metadata(&entry.path)
                .map(|info| human_size(info.len()))
                .unwrap_or_else(|_| "?".into())
        };
        let name_width = width.saturating_sub(14);
        let line = format!(
            " {symbol} {:<name_width$} {:>7}",
            crop(&entry.name, name_width),
            crop(&size, 7)
        );
        rows.push(if index == explorer.cursor() {
            format!("{INVERSE}{BOLD}{line}{NORMAL}")
        } else {
            line
        });
    }
    rows.push(format!("{DIM}{}{NORMAL}", "─".repeat(width.min(60))));
    rows.push(format!(
        "{BOLD}QUEUE  {:02}{NORMAL}  /  {}",
        explorer.selected_count(),
        human_size(explorer.selected_size())
    ));
    rows.push(if explorer.selected_count() == 0 {
        "Press S to receive, or focus a file to share".into()
    } else {
        "Press S to share · A to add while live".into()
    });
    rows
}

fn ready_rows(explorer: &Explorer, _width: usize, height: usize) -> Vec<String> {
    let mut rows = vec![
        format!("{INVERSE}{BOLD}  READY TO SEND  {NORMAL}"),
        "Computer  ━━━━━━━━━━━━━▶  Phone".into(),
        String::new(),
        format!("{BOLD}01 / CHOOSE{NORMAL}"),
        "Focus file or mark several [SPACE]".into(),
        String::new(),
        format!("{BOLD}02 / START{NORMAL}"),
        "Press S. With no file, receive only.".into(),
        String::new(),
        format!("{BOLD}03 / CONNECT{NORMAL}"),
        "Saved phones return automatically.".into(),
        "New phone? Scan QR or type code.".into(),
        String::new(),
        format!("{BOLD}MARKED FOR SHARING{NORMAL}"),
    ];
    let selected = explorer.selected_files();
    if selected.is_empty() {
        rows.push(
            explorer
                .focused_file()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| format!("◇ {}", crop(&name.to_string_lossy(), 80)))
                })
                .unwrap_or_else(|| "No PC file selected: phone uploads work".into()),
        );
    } else {
        for path in selected.iter().take(height.saturating_sub(rows.len())) {
            rows.push(format!(
                "◆ {}",
                crop(&path.file_name().unwrap_or_default().to_string_lossy(), 80)
            ));
        }
    }
    if height >= rows.len() + 7 {
        rows.push(String::new());
        rows.push(format!("{DIM}{}{NORMAL}", "─".repeat(28)));
        rows.push(format!("{BOLD}QUICK KEYS{NORMAL}"));
        rows.push("[↑↓] MOVE    [ENTER] OPEN".into());
        rows.push("[SPACE] MARK [S] START".into());
        rows.push("[H] HIDDEN   [Q] QUIT".into());
    }
    rows
}

fn session_rows(
    server: &ServerProcess,
    stopped: bool,
    width: usize,
    height: usize,
    include_events: bool,
) -> Vec<String> {
    let logs = server.logs();
    let connected = logs
        .iter()
        .any(|line| line.contains("CONNECTED authenticated device"));
    let rates = send_samples(&logs);
    let latest = rates.last().copied().unwrap_or(0.0);
    let mut rows = vec![
        format!(
            "{INVERSE}  {}  {NORMAL}  {}",
            if stopped { "PAUSED" } else { "ON AIR" },
            if connected {
                "PHONE AUTHENTICATED"
            } else {
                "AWAITING PHONE"
            }
        ),
        format!(
            "{}  •  {} shared",
            if stopped {
                "Transfers paused"
            } else {
                "Local HTTPS active"
            },
            server.shared_files().len()
        ),
        String::new(),
        format!("{BOLD}LAST SEND RATE{NORMAL}    {}", rate_text(latest)),
        format!(
            "{DIM}{} {NORMAL}",
            sparkline(&rates, width.saturating_sub(2).min(36))
        ),
        String::new(),
        format!("{BOLD}NEXT FILE{NORMAL}"),
        "Browse left, mark with Space, press A".into(),
        "Your phone stays paired.".into(),
        String::new(),
        format!(
            "{BOLD}SHARED LIBRARY  /  {:02}{NORMAL}",
            server.shared_files().len()
        ),
    ];
    let room_for_files = height.saturating_sub(rows.len() + if include_events { 5 } else { 0 });
    for path in server
        .shared_files()
        .iter()
        .rev()
        .take(room_for_files.min(6))
    {
        rows.push(format!(
            "◆ {}",
            crop(
                &path.file_name().unwrap_or_default().to_string_lossy(),
                width.saturating_sub(4)
            )
        ));
    }
    if include_events {
        rows.push(String::new());
        rows.push(format!("{BOLD}ACTIVITY{NORMAL}"));
        let room = height.saturating_sub(rows.len());
        for event in events(&logs).into_iter().take(room) {
            rows.push(event);
        }
    }
    rows
}

fn activity_rows(server: &ServerProcess, _width: usize, height: usize) -> Vec<String> {
    let logs = server.logs();
    let feed = events(&logs);
    let mut rows = vec![
        format!("{INVERSE}  EVENT FEED  {NORMAL}  {} recent", feed.len()),
        format!("{DIM}Newest event first{NORMAL}"),
        String::new(),
    ];
    if feed.is_empty() {
        rows.push("Waiting for activity…".into());
    }
    for event in feed.into_iter().take(height.saturating_sub(rows.len())) {
        rows.push(event);
    }
    rows
}

fn inspector_rows(explorer: &Explorer, width: usize, height: usize) -> Vec<String> {
    let mut rows = vec![format!("{INVERSE}  FOCUSED ITEM  {NORMAL}"), String::new()];
    let Some(entry) = explorer.entries().get(explorer.cursor()) else {
        rows.push("Nothing in this folder".into());
        return rows;
    };
    rows.push(format!("{BOLD}{}{NORMAL}", crop(&entry.name, width)));
    rows.push(if entry.directory {
        "DIRECTORY  /  ENTER TO OPEN".into()
    } else {
        "FILE  /  SPACE TO MARK".into()
    });
    if let Ok(info) = fs::metadata(&entry.path) {
        rows.push(format!(
            "SIZE  {}",
            if entry.directory {
                "—".into()
            } else {
                human_size(info.len())
            }
        ));
    }
    rows.push(String::new());
    rows.push(format!("{DIM}{}{NORMAL}", "─".repeat(width.min(40))));
    rows.push(format!("{BOLD}PREVIEW{NORMAL}"));
    let remaining = height.saturating_sub(rows.len());
    if entry.directory {
        let mut names = fs::read_dir(&entry.path)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|item| item.file_name().into_string().ok())
            .collect::<Vec<_>>();
        names.sort();
        rows.push(format!("{} item(s) inside", names.len()));
        for name in names.into_iter().take(remaining.saturating_sub(1).min(12)) {
            rows.push(format!("  ▸ {}", crop(&name, width.saturating_sub(4))));
        }
    } else {
        let preview = preview(explorer, width, remaining);
        if !preview.is_empty() {
            rows.extend(preview);
        } else if let Ok(mut file) = fs::File::open(&entry.path) {
            let mut bytes = Vec::new();
            if Read::by_ref(&mut file)
                .take(4096)
                .read_to_end(&mut bytes)
                .is_ok()
            {
                if let Ok(text) = String::from_utf8(bytes) {
                    for (index, line) in text.lines().take(remaining).enumerate() {
                        let clean: String = line
                            .chars()
                            .map(|ch| if ch.is_control() { ' ' } else { ch })
                            .collect();
                        rows.push(format!("{DIM}{:>3} │{NORMAL} {}", index + 1, clean));
                    }
                } else {
                    rows.push("Binary file · no text preview".into());
                }
            }
        } else {
            rows.push("Preview unavailable".into());
        }
    }
    rows
}

fn parse_metric(line: &str, field: &str) -> Option<f64> {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(field))
        .and_then(|value| value.strip_suffix("B/s"))
        .and_then(|value| value.parse::<f64>().ok())
}

fn send_samples(logs: &[String]) -> Vec<f64> {
    logs.iter()
        .filter(|line| line.contains("TRANSFER session-sent="))
        .filter_map(|line| parse_metric(line, "upload="))
        .rev()
        .take(36)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn rate_text(bytes: f64) -> String {
    if bytes >= 1048576.0 {
        format!("{:.2} MiB/s", bytes / 1048576.0)
    } else if bytes >= 1024.0 {
        format!("{:.1} KiB/s", bytes / 1024.0)
    } else {
        format!("{bytes:.0} B/s")
    }
}

fn transfer_summary(logs: &[String]) -> String {
    let latest = send_samples(logs).last().copied().unwrap_or(0.0);
    format!("LAST SEND {}", rate_text(latest))
}

fn sparkline(samples: &[f64], width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut values = samples
        .iter()
        .rev()
        .take(width)
        .copied()
        .collect::<Vec<_>>();
    values.reverse();
    let peak = values.iter().copied().fold(0.0_f64, f64::max);
    let mut out = String::new();
    out.push_str(&"·".repeat(width.saturating_sub(values.len())));
    for value in values {
        if peak <= 0.0 || value <= 0.0 {
            out.push('·');
        } else {
            let index = ((value / peak * 7.0).ceil() as usize).min(7);
            out.push(['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][index]);
        }
    }
    out
}

fn pairing_rows(
    server: &ServerProcess,
    width: usize,
    height: usize,
    manual_only: bool,
) -> Vec<String> {
    let mut rows = Vec::new();
    let qr = server.qr_code();
    let address = server
        .pairing_url()
        .and_then(|url| {
            let (base, fragment) = url.split_once('#')?;
            fragment.split_once('~')?;
            Some(base.to_string())
        })
        .unwrap_or_default();
    let (code, key) = server
        .logs()
        .iter()
        .rev()
        .find_map(|line| {
            let (_, details) = line.split_once("TapSend manual details: ")?;
            let (code, key) = details.trim().split_once(' ')?;
            Some((code.to_string(), key.to_string()))
        })
        .unwrap_or_else(|| ("Preparing…".into(), "Preparing…".into()));
    if !manual_only {
        if qr.is_empty() {
            rows.push("Preparing QR; check qrencode installation".into());
        } else if qr.iter().all(|line| line.chars().count() <= width) {
            if qr.len() + 6 <= height {
                rows.push(format!("{BOLD}SCAN WITH THE ANDROID APP{NORMAL}"));
                rows.extend(qr);
            } else if qr.len() <= height {
                rows.extend(qr);
                return rows;
            } else {
                rows.push("Enlarge terminal to show the full QR.".into());
            }
        } else {
            rows.push("Widen terminal to show the full QR.".into());
        }
    }
    rows.push(format!("{BOLD}TYPE MANUALLY IN APP{NORMAL}"));
    rows.push(format!("ADDRESS  {address}"));
    rows.push(format!("CODE     {code}"));
    rows.push(format!("KEY      {key}"));
    rows.push("Code lasts 5 min • r creates a new one".into());
    rows
}

fn events(logs: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut tls_reported = false;
    for line in logs.iter().rev() {
        let body = if line.len() > 20 && line.as_bytes().get(4) == Some(&b'/') {
            &line[20..]
        } else {
            line
        };
        if body.starts_with("HEALTH ")
            || body.starts_with("STREAM ")
            || body.starts_with("TRANSFER session-sent=")
            || body.contains("TapSend pairing URL:")
            || body.contains("TapSend manual details:")
            || body.contains("CA certificate")
        {
            continue;
        }
        if body.contains("TLS handshake error") {
            if !tls_reported {
                result.push("! Phone closed TLS setup; check trust".into());
                tls_reported = true;
            }
            continue;
        }
        let symbol = if body.starts_with("SENDING ") {
            "↑"
        } else if body.starts_with("RECEIVED ") {
            "↓"
        } else if body.starts_with("CONNECTED ") || body.starts_with("PAIRED ") {
            "●"
        } else if body.starts_with("MESSAGE ") || body.starts_with("CHAT ") {
            "✉"
        } else if body.starts_with("FILE ADDED ") {
            "◆"
        } else {
            "·"
        };
        let time = if line.len() >= 19 && line.as_bytes().get(4) == Some(&b'/') {
            &line[11..19]
        } else {
            "--:--:--"
        };
        let decorated = format!("{time} {symbol} {body}");
        if result.last().map(String::as_str) != Some(decorated.as_str()) {
            result.push(decorated);
        }
    }
    result
}

fn preview(explorer: &Explorer, width: usize, height: usize) -> Vec<String> {
    static CACHE: Mutex<(String, Vec<String>)> = Mutex::new((String::new(), Vec::new()));
    let Some(entry) = explorer.entries().get(explorer.cursor()) else {
        return Vec::new();
    };
    if entry.directory || width < 12 || height < 4 {
        return Vec::new();
    }
    let ext = entry
        .path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let video = ["mp4", "mkv", "webm", "mov", "avi"].contains(&ext.as_str());
    if !video && !["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&ext.as_str()) {
        return Vec::new();
    }
    let w = width.min(60);
    let h = height.saturating_sub(1).min(12);
    let key = format!("{}:{w}:{h}", entry.path.display());
    let mut cache = CACHE.lock().unwrap();
    if cache.0 == key {
        return cache.1.clone();
    }
    let filter = format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"
    );
    let mut command = Command::new("ffmpeg");
    command.args(["-v", "error", "-nostdin"]);
    if video {
        let duration = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(&entry.path)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        command.args(["-ss", &(duration / 2.0).to_string()]);
    }
    let output = command
        .arg("-i")
        .arg(&entry.path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let mut result = vec![format!("{BOLD}MONO PREVIEW{NORMAL}")];
    if let Ok(output) = output {
        if output.status.success() && output.stdout.len() >= w * h * 3 {
            let shades = [' ', '·', ':', '-', '=', '+', '*', '#', '%', '█'];
            for y in 0..h {
                let mut row = String::new();
                for x in 0..w {
                    let index = (y * w + x) * 3;
                    let light = (u32::from(output.stdout[index]) * 30
                        + u32::from(output.stdout[index + 1]) * 59
                        + u32::from(output.stdout[index + 2]) * 11)
                        / 100;
                    row.push(shades[(light as usize * (shades.len() - 1)) / 255]);
                }
                result.push(row);
            }
        } else {
            result.push("Preview unavailable".into());
        }
    } else {
        result.push("Install ffmpeg for previews".into());
    }
    *cache = (key, result.clone());
    result
}

fn human_size(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.1} GiB", n as f64 / (1u64 << 30) as f64)
    } else if n >= 1 << 20 {
        format!("{:.1} MiB", n as f64 / (1u64 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

fn crop(s: &str, width: usize) -> String {
    s.chars()
        .take(width)
        .map(|ch| if ch.is_control() { '�' } else { ch })
        .collect()
}

fn padded(s: &str, width: usize) -> String {
    let mut result = String::new();
    let mut escape = false;
    let mut count = 0;
    for c in s.chars() {
        if c == '\x1b' {
            escape = true;
        }
        if escape {
            result.push(c);
            if c.is_ascii_alphabetic() {
                escape = false;
            }
            continue;
        }
        let cells = cell_width(c);
        if count + cells > width {
            break;
        }
        if c.is_control() {
            continue;
        }
        result.push(c);
        count += cells;
    }
    result.push_str(NORMAL);
    result.push_str(&" ".repeat(width.saturating_sub(count)));
    result
}

fn cell_width(c: char) -> usize {
    let n = c as u32;
    if c.is_control()
        || matches!(n,0x0300..=0x036f|0x1ab0..=0x1aff|0x1dc0..=0x1dff|0x200b..=0x200f|0x202a..=0x202e|0x2060..=0x206f|0x20d0..=0x20ff|0xfe00..=0xfe0f|0xfe20..=0xfe2f|0xe0100..=0xe01ef)
    {
        0
    } else if matches!(n,0x1100..=0x115f|0x2329..=0x232a|0x2600..=0x27bf|0x2e80..=0xa4cf|0xac00..=0xd7a3|0xf900..=0xfaff|0xfe10..=0xfe19|0xfe30..=0xfe6f|0xff01..=0xff60|0xffe0..=0xffe6|0x1f000..=0x1faff|0x20000..=0x3fffd)
    {
        2
    } else {
        1
    }
}

#[cfg(not(windows))]
fn terminal_size() -> (usize, usize) {
    Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let mut parts = s.split_whitespace();
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
        })
        .map(|(height, width)| (width, height))
        .filter(|(width, height)| *width > 0 && *height > 0)
        .unwrap_or((110, 32))
}

#[cfg(windows)]
fn terminal_size() -> (usize, usize) {
    crate::windows_console::terminal_size().unwrap_or((110, 32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_clipping_keeps_cell_width() {
        assert_eq!(padded("😂😂😂", 5), format!("😂😂{NORMAL} "));
        assert_eq!(padded("你好", 3), format!("你{NORMAL} "));
        assert_eq!(padded("a\u{301}", 2), format!("a\u{301}{NORMAL} "));
    }

    #[test]
    fn panel_has_requested_height() {
        assert_eq!(panel("FILES", 40, 12, &["one".into()]).len(), 12);
    }

    #[test]
    fn pending_devices_only_use_latest_snapshot() {
        let logs = vec![
            "JOIN REQUEST phone-cccc — sender approval needed".into(),
            "JOIN REQUEST phone-cccc \"Phone C\" — approve phone-cccc or deny phone-cccc".into(),
            "CONTROL devices applied".into(),
        ];
        assert_eq!(
            pending_from_logs(&logs),
            vec![("phone-cccc".into(), "Phone C".into())]
        );
        let mut after_approval = logs;
        after_approval.push("JOIN APPROVED phone-cccc".into());
        after_approval.push("CONTROL devices applied".into());
        assert!(pending_from_logs(&after_approval).is_empty());
    }

    #[test]
    fn transfer_history_reads_server_rates_in_order() {
        let logs = vec![
            "2026/11/22 20:50:01 TRANSFER session-sent=1.00MiB upload=1048576B/s".into(),
            "2026/11/22 20:50:02 HEALTH phone=192.168.2.60 rtt=23ms download=123B/s".into(),
            "2026/11/22 20:50:02 STREAM lane=1 speed=512B/s bytes=1024".into(),
            "2026/11/22 20:50:02 STREAM lane=2 speed=256B/s bytes=512".into(),
            "2026/11/22 20:50:03 TRANSFER session-sent=3.00MiB upload=2097152B/s".into(),
        ];
        assert_eq!(send_samples(&logs), vec![1048576.0, 2097152.0]);
        assert_eq!(lane_samples(&logs, 1), vec![512.0]);
        assert_eq!(lane_samples(&logs, 2), vec![256.0]);
        assert_eq!(rtt_samples(&logs), vec![23.0]);
        assert_eq!(parse_number_field(&logs[1], "rtt=", "ms"), Some(23.0));
        assert_eq!(sparkline(&send_samples(&logs), 2).chars().count(), 2);
    }

    #[test]
    fn saved_devices_use_latest_completed_snapshot() {
        let logs = vec![
            "DEVICE old-phone \"Old\"".into(),
            "CONTROL devices applied".into(),
            "CONTROL revoke applied".into(),
            "DEVICE new-phone \"New\"".into(),
            "CONTROL devices applied".into(),
        ];
        assert_eq!(
            devices_from_logs(&logs),
            vec![("new-phone".into(), "New".into())]
        );
    }

    #[test]
    fn command_popup_filters_by_session_and_search() {
        let ready = command_suggestions("", false);
        assert!(ready.iter().any(|command| command.name == "share"));
        assert!(!ready.iter().any(|command| command.name == "revoke"));
        let found = command_suggestions("mess", true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "message");
    }
}
