#[cfg(windows)]
use crate::server::SendState;
use crate::{explorer::Explorer, server::ServerProcess};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    process::{Command, Stdio},
    sync::Mutex,
};

const NORMAL: &str = "\x1b[0m\x1b[40m\x1b[97m";
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
    #[cfg(windows)]
    CommandSpec {
        name: "relay",
        detail: "create SHARED_FOLDER · send · status",
        argument: true,
        live_only: false,
    },
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
    #[cfg(windows)]
    PasteStack {
        cursor: usize,
        items: Vec<crate::server::PasteItem>,
        status: &'a str,
    },
    #[cfg(windows)]
    Scratchpad {
        cursor: usize,
        entries: &'a [crate::server::Snippet],
        status: &'a str,
    },
    #[cfg(windows)]
    FileSearch {
        explorer: &'a Explorer,
        query: &'a str,
        matches: &'a [usize],
        cursor: usize,
    },
    #[cfg(windows)]
    Recipient {
        devices: Option<&'a [crate::server::RecipientDevice]>,
        cursor: usize,
        sending: bool,
        status: &'a str,
    },
    #[cfg(windows)]
    DeviceEdit {
        title: &'static str,
        subtitle: String,
        input: Option<&'a str>,
    },
    #[cfg(windows)]
    DesktopShare {
        files: &'a [std::path::PathBuf],
        scroll: usize,
    },
    #[cfg(windows)]
    PhoneDownload {
        name: &'a str,
        index: usize,
        count: usize,
        done: u64,
        total: i64,
        existing: bool,
    },
    #[cfg(windows)]
    PhoneDownloadDone {
        lines: &'a [String],
    },
    #[cfg(windows)]
    PhoneShare {
        files: Vec<(String, i64, bool)>,
        cursor: usize,
        stage: u8,
    },
    Help {
        scroll: usize,
    },
    Command {
        input: &'a str,
        selected: usize,
        suggestions: Vec<&'static CommandSpec>,
    },
    Settings {
        values: &'a [String; 9],
        selected: usize,
        edit: Option<&'a str>,
        error: &'a str,
        dirty: bool,
        confirm_exit: bool,
        first_run: bool,
    },
}

pub(crate) fn settings_fields(first_run: bool) -> &'static [usize] {
    if first_run {
        &[0, 6, 7]
    } else {
        &[0, 1, 2, 3, 4, 5, 6, 7, 8]
    }
}

const HELP_LINES: &[&str] = &[
    "WORKSPACE",
    " 1-5 / Tab   Dashboard, Files, Devices, Transfers, Messages",
    " M           Compose message",
    "",
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
    #[cfg(windows)]
    " C           Scan a phone-share QR with the laptop camera",
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
    _device_view: bool,
    page: usize,
    queue_cursor: usize,
    overlay: Option<OverlayView<'_>>,
) -> io::Result<()> {
    let (columns, height) = terminal_size();
    let width = columns.saturating_sub(1).max(1);
    let lines = workspace(
        explorer,
        server,
        message,
        pairing_view,
        stopped,
        page,
        queue_cursor,
        width,
        height,
    );
    paint_frame(&lines, width, height, overlay)
}

#[derive(Debug)]
struct Transfer {
    id: u64,
    direction: String,
    name: String,
    done: f64,
    total: f64,
    speed: f64,
    state: String,
}
fn transfers(logs: &[String]) -> Vec<Transfer> {
    let mut jobs = std::collections::BTreeMap::new();
    for line in logs {
        let Some((_, body)) = line.split_once("TUI_JOB ") else {
            continue;
        };
        let fields: std::collections::HashMap<_, _> = body
            .split_whitespace()
            .filter_map(|s| s.split_once('='))
            .collect();
        let Some(id) = fields.get("id").and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        let number = |key| {
            fields
                .get(key)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
        };
        let hex = fields.get("name").copied().unwrap_or("");
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .filter_map(|pair| {
                std::str::from_utf8(pair)
                    .ok()
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
            })
            .collect();
        jobs.insert(
            id,
            Transfer {
                id,
                direction: fields.get("dir").unwrap_or(&"--").to_string(),
                name: String::from_utf8_lossy(&bytes).into_owned(),
                done: number("done"),
                total: number("total"),
                speed: number("speed").max(0.0),
                state: fields.get("state").unwrap_or(&"unknown").to_string(),
            },
        );
    }
    let mut jobs: Vec<_> = jobs.into_values().collect();
    jobs.sort_by_key(|job| (job.state != "active", std::cmp::Reverse(job.id)));
    jobs
}
pub(crate) fn transfer_count(logs: &[String]) -> usize {
    transfers(logs).len()
}
pub(crate) fn selected_received_name(logs: &[String], index: usize) -> Option<String> {
    let job = transfers(logs).into_iter().nth(index)?;
    (job.direction == "RX" && job.state == "saved").then_some(job.name)
}
fn direction_samples(logs: &[String], direction: &str) -> Vec<f64> {
    let mut rates = std::collections::HashMap::new();
    let mut samples = Vec::new();
    for line in logs {
        if !line.contains("TUI_JOB ") {
            continue;
        }
        let fields: std::collections::HashMap<_, _> = line
            .split_whitespace()
            .filter_map(|s| s.split_once('='))
            .collect();
        if fields.get("dir") != Some(&direction) {
            continue;
        }
        let Some(id) = fields.get("id") else { continue };
        let speed = if fields.get("state") == Some(&"active") {
            fields
                .get("speed")
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|s| s.is_finite())
                .unwrap_or(0.0)
                .max(0.0)
        } else {
            0.0
        };
        rates.insert(id.to_string(), speed);
        samples.push(rates.values().sum());
    }
    samples
}

fn transfer_rows(
    logs: &[String],
    width: usize,
    height: usize,
    selected: Option<usize>,
    desktop: bool,
) -> Vec<String> {
    let jobs = transfers(logs);
    if width >= 78 {
        let name_width = width.saturating_sub(66).max(12);
        let mut rows = vec![
            format!(
                "#   {} {} {} {} {}",
                "DIR",
                text_cell("NAME", name_width),
                text_cell("SIZE", 9),
                text_cell("PROGRESS", 18),
                "SPEED / ETA / STATE"
            ),
            "─".repeat(width),
        ];
        if jobs.is_empty() {
            rows.extend([
                String::new(),
                "No active transfers".into(),
                if desktop {
                    "[F] Choose files to send    [R] Receive from phone"
                } else {
                    "[2] Choose files to send    [6] Receive from phone"
                }
                .into(),
            ]);
        }
        for (index, job) in jobs.iter().take(height.saturating_sub(2)).enumerate() {
            let fraction = if job.total > 0.0 {
                (job.done / job.total).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let blocks = (fraction * 10.0) as usize;
            let bar = if job.total > 0.0 {
                format!(
                    "[{}{}] {:3.0}%",
                    "█".repeat(blocks),
                    "░".repeat(10 - blocks),
                    fraction * 100.0
                )
            } else {
                "[░░░░░░░░░░]  --%".into()
            };
            let eta = if job.state == "active" && job.speed > 0.0 && job.total > 0.0 {
                format!("{:.0}s", ((job.total - job.done) / job.speed).max(0.0))
            } else {
                "--".into()
            };
            let size = if job.total >= 0.0 {
                human_size(job.total as u64)
            } else {
                "--".into()
            };
            let line = format!(
                "{:02}  {} {} {} {} {} {} {}",
                job.id,
                if job.direction == "TX" { "↑" } else { "↓" },
                text_cell(&job.name, name_width),
                text_cell(&size, 9),
                text_cell(&bar, 18),
                text_cell(&rate_text(job.speed), 11),
                text_cell(&eta, 5),
                job.state
            );
            rows.push(if selected == Some(index) {
                format!("{INVERSE}{}{NORMAL}", text_cell(&line, width))
            } else {
                line
            });
        }
        return rows;
    }
    if jobs.is_empty() {
        return vec!["No transfers yet. [F] choose files / [S] receive.".into()];
    }
    let mut rows = vec!["HTTP REQUESTS / parallel file ranges shown separately".into()];
    for (index, job) in jobs.iter().take(height.saturating_sub(1) / 2).enumerate() {
        let fraction = if job.total > 0.0 {
            (job.done / job.total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let blocks = (fraction * 10.0) as usize;
        let progress = if job.total > 0.0 {
            format!(
                "[{}{}] {:3.0}%",
                "#".repeat(blocks),
                ".".repeat(10 - blocks),
                fraction * 100.0
            )
        } else {
            "[..........]  --%".into()
        };
        let eta = if job.state == "active" && job.speed > 0.0 && job.total > 0.0 {
            format!("{:.0}s", ((job.total - job.done) / job.speed).max(0.0))
        } else {
            "--".into()
        };
        let name = format!(
            "{BOLD}{} #{:02} {}{NORMAL}",
            job.direction,
            job.id,
            crop(&job.name, width.saturating_sub(12))
        );
        rows.push(if selected == Some(index) {
            format!("{INVERSE}{name}{NORMAL}")
        } else {
            name
        });
        rows.push(format!(
            "{progress} {} ETA {eta} {}",
            rate_text(job.speed),
            job.state
        ));
    }
    rows
}

fn join_panels(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.into_iter()
        .zip(right)
        .map(|(a, b)| format!("{a} {b}"))
        .collect()
}

fn text_cell(value: &str, width: usize) -> String {
    let clipped = crop(value, width);
    let cells: usize = clipped.chars().map(cell_width).sum();
    format!("{clipped}{}", " ".repeat(width.saturating_sub(cells)))
}

fn crop_ellipsis(value: &str, width: usize) -> String {
    if value.chars().map(cell_width).sum::<usize>() <= width {
        return value.to_string();
    }
    format!("{}…", crop(value, width.saturating_sub(1)))
}

fn bar_chart(logs: &[String], width: usize, height: usize) -> Vec<String> {
    let tx = direction_samples(logs, "TX");
    let rx = direction_samples(logs, "RX");
    let mut rows = vec![format!(
        "↑ TX {}  ↓ RX {}",
        rate_text(*tx.last().unwrap_or(&0.0)),
        rate_text(*rx.last().unwrap_or(&0.0))
    )];
    let chart_height = height.saturating_sub(3).max(1);
    let plot_width = width.saturating_sub(9);
    let mut samples = Vec::new();
    for i in 0..tx.len().max(rx.len()) {
        samples.push(tx.get(i).copied().unwrap_or(0.0) + rx.get(i).copied().unwrap_or(0.0));
    }
    let peak = samples.iter().copied().fold(0.0, f64::max).max(1.0);
    let tail = &samples[samples.len().saturating_sub(plot_width)..];
    for y in (0..chart_height).rev() {
        let threshold = peak * (y + 1) as f64 / chart_height as f64;
        let mut bars = " ".repeat(plot_width.saturating_sub(tail.len()));
        for value in tail {
            bars.push(if *value >= threshold {
                '█'
            } else if y == 0 && *value > 0.0 {
                '▂'
            } else {
                ' '
            });
        }
        let label = if y == chart_height - 1 {
            format!("{:.1}M", peak / 1048576.0)
        } else if y == 0 {
            "0".into()
        } else {
            String::new()
        };
        rows.push(format!("{:>6} │{bars}", label));
    }
    rows.push(format!("       └{}", "─".repeat(plot_width)));
    rows.push(if samples.is_empty() {
        "Waiting for network activity".into()
    } else {
        "TX + RX / recent reports / auto scale".into()
    });
    rows
}

fn device_table(
    server: Option<&ServerProcess>,
    width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let logs = server.map(ServerProcess::logs).unwrap_or_default();
    let devices = devices_from_logs(&logs);
    let mut rows = vec![
        format!("{:<12} {}", "ID", "DEVICE / TRUST"),
        "─".repeat(width),
    ];
    if devices.is_empty() {
        rows.push("No device snapshot yet.".into());
        rows.push(
            if desktop {
                "[V] Refresh devices   [P] Pair"
            } else {
                "[D] Refresh devices   [P] Pair"
            }
            .into(),
        );
    }
    for (id, name) in devices.iter().take(height.saturating_sub(2)) {
        rows.push(format!(
            "{} {}  saved",
            text_cell(id, 10),
            crop(name, width.saturating_sub(20))
        ));
    }
    rows
}

fn dashboard_v2(
    server: Option<&ServerProcess>,
    stopped: bool,
    queue_cursor: usize,
    width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let logs = server.map(ServerProcess::logs).unwrap_or_default();
    let tx = *direction_samples(&logs, "TX").last().unwrap_or(&0.0);
    let rx = *direction_samples(&logs, "RX").last().unwrap_or(&0.0);
    let devices = devices_from_logs(&logs);
    let rtt = rtt_samples(&logs)
        .last()
        .map(|n| format!("{n:.0} ms"))
        .unwrap_or_else(|| "--".into());
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let address = server
        .and_then(ServerProcess::pairing_url)
        .map(|u| {
            u.trim_start_matches("https://")
                .split('/')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_else(|| "--".into());
    let status = if stopped {
        "Downloads stopped"
    } else if server.is_some() {
        "Listening"
    } else {
        "Offline"
    };
    let mut out = panel(
        "// DASHBOARD / CONNECTION INFO",
        width,
        4,
        &[
            format!("{BOLD}Overview of transfers, devices and network{NORMAL}  │  {status}"),
            format!(
                "LAN {address}  │  TLS {}  │  {:02}:{:02}:{:02} UTC",
                if server.is_some() { "enabled" } else { "idle" },
                clock / 3600 % 24,
                clock / 60 % 60,
                clock % 60
            ),
        ],
    );
    let metrics = [
        ("↑ TX / Upload", rate_text(tx)),
        ("↓ RX / Download", rate_text(rx)),
        ("↔ Total", rate_text(tx + rx)),
        ("▣ Devices", format!("{} saved", devices.len())),
        ("◷ Latency", rtt),
    ];
    let base = (width - 4) / 5;
    let mut strip = vec![String::new(); 5];
    for (i, (title, value)) in metrics.iter().enumerate() {
        let w = if i == 4 { width - 4 - base * 4 } else { base };
        let card = panel(
            title,
            w,
            5,
            &[String::new(), format!("{BOLD}{value}{NORMAL}")],
        );
        for row in 0..5 {
            if i > 0 {
                strip[row].push(' ');
            }
            strip[row].push_str(&card[row]);
        }
    }
    out.extend(strip);
    let log_height = (height / 5).clamp(6, 10);
    let center_height = height - 9 - log_height;
    let queue_height = (center_height * 48 / 100).clamp(7, center_height.saturating_sub(7).max(7));
    let lower_height = center_height - queue_height;
    out.extend(panel(
        if desktop {
            "⇅ TRANSFER QUEUE   [Up/Down] Select  [Space] Open RX  [T] Details"
        } else {
            "⇅ TRANSFER QUEUE   [Up/Down] Select  [Space] Open RX  [4] Details"
        },
        width,
        queue_height,
        &transfer_rows(
            &logs,
            width - 4,
            queue_height.saturating_sub(2),
            Some(queue_cursor),
            desktop,
        ),
    ));
    let nw = width * 40 / 100;
    let dw = (width - nw - 2) / 2;
    let sw = width - nw - dw - 2;
    let lower = join_panels(
        panel(
            if desktop {
                "▣ DEVICES   [V] Open"
            } else {
                "▣ DEVICES   [D] Refresh"
            },
            dw,
            lower_height,
            &device_table(server, dw - 4, lower_height.saturating_sub(2), desktop),
        ),
        panel(
            "▤ SHARED FILES   [F] Browse",
            sw,
            lower_height,
            &shared_rows(server, sw - 4, lower_height.saturating_sub(2)),
        ),
    );
    out.extend(join_panels(
        panel(
            "▥ NETWORK USAGE / TX + RX",
            nw,
            lower_height,
            &bar_chart(&logs, nw - 4, lower_height.saturating_sub(2)),
        ),
        lower,
    ));
    out.extend(panel(
        "▤ CONNECTION LOGS / latest events",
        width,
        log_height,
        &connection_log_rows(server, log_height - 2),
    ));
    out
}

fn pairing_workspace(
    server: &ServerProcess,
    manual_only: bool,
    width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let qr = server.qr_code();
    let qr_width = qr
        .iter()
        .map(|row| row.chars().map(cell_width).sum::<usize>())
        .max()
        .unwrap_or(42);
    let left_width = (qr_width + 4).clamp(42, width.saturating_sub(48));
    let right_width = width - left_width - 1;
    let manual = pairing_rows(
        server,
        right_width.saturating_sub(4),
        height.saturating_sub(2),
        true,
        desktop,
    );
    let mut left = vec![format!("{BOLD}01 / SCAN TO CONNECT{NORMAL}"), String::new()];
    if qr.is_empty() {
        left.push("Waiting for a pairing QR…".into());
    } else if qr.len() <= height.saturating_sub(6) && qr_width <= left_width.saturating_sub(4) {
        left.extend(qr);
    } else {
        left.push("QR needs more terminal space; use manual pairing →".into());
    }
    left.push(String::new());
    left.push(
        if desktop {
            "[U] Generate a new code"
        } else {
            "[R] Generate a new code"
        }
        .into(),
    );
    let mut details = vec![
        format!("{BOLD}02 / ENTER MANUALLY{NORMAL}"),
        "Use these details in TapSend Android.".into(),
        String::new(),
    ];
    details.extend(manual.into_iter().skip(1));
    details.extend([
        String::new(),
        "─".repeat(right_width.saturating_sub(4)),
        String::new(),
        format!("{BOLD}WHY WON'T IT CONNECT?{NORMAL}"),
        "1. Connect both devices to the same network.".into(),
        "2. Windows Firewall: allow TapSend on this network.".into(),
        "3. PC address changed? Restart TapSend and rescan.".into(),
        if desktop {
            "4. Press [V], then [M] to approve devices."
        } else {
            "4. Approve new devices from [3] Devices."
        }
        .into(),
        String::new(),
        if desktop {
            "[P] Switch QR / manual view     [D] Dashboard"
        } else {
            "[P] Switch QR / manual view     [1] Dashboard"
        }
        .into(),
    ]);
    let guide = [
        format!("{BOLD}CONNECT WITHOUT A CAMERA{NORMAL}"),
        String::new(),
        "1. Open Connect on your phone.".into(),
        "2. Choose manual pairing.".into(),
        "3. Enter the address, code and key →".into(),
        "4. Approve the request in Devices.".into(),
        String::new(),
        "[P] Return to dashboard".into(),
    ];
    join_panels(
        panel(
            if manual_only {
                "ⓘ PAIRING GUIDE"
            } else {
                "⇄ PAIRING QR / Active"
            },
            left_width,
            height,
            if manual_only { &guide } else { &left },
        ),
        panel(
            if manual_only {
                "⌨ MANUAL CONNECTION / Active"
            } else {
                "⌨ MANUAL CONNECTION"
            },
            right_width,
            height,
            &details,
        ),
    )
}

fn wide_workspace(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing: u8,
    stopped: bool,
    page: usize,
    queue_cursor: usize,
    width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let sidebar = if desktop { 0 } else { 30 };
    let right_width = if desktop { width } else { width - sidebar - 1 };
    let body_height = height - if desktop { 3 } else { 5 };
    let logs = server.map(ServerProcess::logs).unwrap_or_default();
    let address = server
        .and_then(ServerProcess::pairing_url)
        .map(|u| {
            u.trim_start_matches("https://")
                .split('/')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_else(|| "--".into());
    let state = if stopped {
        "PAUSED"
    } else if server.is_some() {
        "ONLINE"
    } else {
        "OFFLINE"
    };
    let nav = if desktop {
        Vec::new()
    } else {
        let (host, port) = address.rsplit_once(':').unwrap_or((&address, "--"));
        let mut nav = vec![
            format!(
                "{BOLD}{} {state} (LAN){NORMAL}",
                if server.is_some() { "●" } else { "○" }
            ),
            String::new(),
            format!("IP      : {}", crop(host, sidebar - 14)),
            format!("Port    : {port}"),
            format!("Devices : {} saved", devices_from_logs(&logs).len()),
            "Mode    : Local Network".into(),
            "─".repeat(sidebar - 4),
            format!("{DIM}NAVIGATION{NORMAL}"),
        ];
        let items = [
            ("⌂", "Dashboard", "1", 0),
            ("↑", "Send Files", "2", 1),
            ("↓", "Receive Files", "6", 9),
            ("⇄", "Connect / Pair", "P", 8),
            ("▤", "Messages", "5", 4),
            ("▣", "Devices", "3", 2),
            ("⇅", "Transfers", "4", 3),
            ("▦", "Settings", "G", 7),
            ("ⓘ", "About / Help", "?", 6),
        ];
        for (icon, label, key, view) in items {
            let line = format!("{icon}  {:<16} [{key}]", label);
            nav.push(
                if (pairing > 0 && view == 8) || (pairing == 0 && page == view) {
                    format!("{INVERSE}{BOLD}{}{NORMAL}", text_cell(&line, sidebar - 4))
                } else {
                    line
                },
            );
            if body_height >= 40 {
                nav.push(String::new());
            }
        }
        while nav.len() + 6 < body_height - 2 {
            nav.push(String::new());
        }
        nav.extend([
            format!("{BOLD}  ╔╦╗┌─┐┌─┐╔═╗┌─┐┌┐┌┌┬┐{NORMAL}"),
            "   ║ ├─┤├─┘╚═╗├┤ │││ ││".into(),
            "   ╩ ┴ ┴┴  ╚═╝└─┘┘└┘─┴┘".into(),
            String::new(),
            " [ Fast · Local · Secure ]".into(),
        ]);
        nav
    };
    let content = if pairing > 0 && server.is_some() {
        pairing_workspace(
            server.unwrap(),
            pairing == 2,
            right_width,
            body_height,
            desktop,
        )
    } else if page == 0 {
        dashboard_v2(
            server,
            stopped,
            queue_cursor,
            right_width,
            body_height,
            desktop,
        )
    } else if page == 1 {
        let lw = right_width * 65 / 100;
        let rw = right_width - lw - 1;
        let mut files = vec![crop(&explorer.directory().display().to_string(), lw - 4)];
        files.extend(file_rows(explorer, lw - 4, body_height - 3));
        join_panels(
            panel("↑ SEND FILES / Space: select", lw, body_height, &files),
            panel(
                "▤ FILE INSPECTOR",
                rw,
                body_height,
                &inspector_rows(explorer, rw - 4, body_height - 2),
            ),
        )
    } else if page == 4 {
        let history_height = (body_height * 58 / 100).clamp(14, body_height - 12);
        let log_height = body_height - history_height - 7;
        let activity_width = (right_width / 3).max(30);
        let conversation_width = right_width - activity_width - 1;
        let mut activity_rows = vec![
            format!("{BOLD}SESSION{NORMAL}"),
            if server.is_some() {
                "● Sharing is live"
            } else {
                "○ Waiting for a share"
            }
            .into(),
            String::new(),
            format!("{BOLD}HOW TO MESSAGE{NORMAL}"),
            if desktop {
                "[E]  Write a message"
            } else {
                "[M]  Write a message"
            }
            .into(),
            "[Enter]  Send to the phone".into(),
            "[P]  Show pairing code".into(),
            String::new(),
            format!("{BOLD}RECENT ACTIVITY{NORMAL}"),
        ];
        activity_rows.extend(connection_log_rows(
            server,
            history_height.saturating_sub(12),
        ));
        let mut chat = join_panels(
            panel(
                "▤ CONVERSATION / Phone ↔ Computer",
                conversation_width,
                history_height,
                &message_rows(server, conversation_width - 4, history_height - 2, desktop),
            ),
            panel(
                "◷ SESSION / Activity",
                activity_width,
                history_height,
                &activity_rows,
            ),
        );
        let mut session_log = vec![format!("{BOLD}CONNECTION EVENTS{NORMAL}")];
        session_log.extend(connection_log_rows(server, log_height.saturating_sub(3)));
        chat.extend(panel(
            "◷ SESSION LOG / Recent events",
            right_width,
            log_height,
            &session_log,
        ));
        chat.extend(panel(
            if desktop {
                "⌨ COMPOSE   [E] Write message"
            } else {
                "⌨ COMPOSE   [M] Write message"
            },
            right_width,
            7,
            &[
                String::new(),
                format!(
                    "{INVERSE} >_  Press {} to type a message{NORMAL}",
                    if desktop { "E" } else { "M" }
                ),
                String::new(),
                "Messages are delivered to paired phones while sharing is live.".into(),
            ],
        ));
        chat
    } else if page == 2 {
        let upper = (body_height * 55 / 100).clamp(13, body_height - 8);
        let saved_width = right_width * 60 / 100;
        let pending_width = right_width - saved_width - 1;
        let pending = pending_from_logs(&logs);
        let mut waiting = vec![
            format!("{BOLD}{} WAITING FOR APPROVAL{NORMAL}", pending.len()),
            "─".repeat(pending_width.saturating_sub(4)),
        ];
        if pending.is_empty() {
            waiting.push("No requests waiting".into());
        }
        for (id, name) in pending.iter().take(upper.saturating_sub(9)) {
            waiting.push(format!(
                "● {}  {}",
                crop(id, 10),
                crop(name, pending_width.saturating_sub(20))
            ));
        }
        waiting.extend([
            String::new(),
            "[;] approve ID".into(),
            "[;] deny ID".into(),
            "[P] Pair another device".into(),
            #[cfg(windows)]
            "[C] Scan phone share QR with camera".into(),
        ]);
        let mut devices = join_panels(
            panel(
                if desktop {
                    "▣ SAVED DEVICES  [V] Refresh"
                } else {
                    "▣ SAVED DEVICES  [D] Refresh"
                },
                saved_width,
                upper,
                &device_table(server, saved_width - 4, upper - 2, desktop),
            ),
            panel("◉ JOIN REQUESTS", pending_width, upper, &waiting),
        );
        let mut activity = vec![
            format!(
                "[;] rename ID NAME     [;] revoke ID     [{}] refresh list",
                if desktop { "V" } else { "D" }
            ),
            "Saved devices are trusted identities; live connection status is shown in logs.".into(),
            String::new(),
            format!("{BOLD}RECENT CONNECTION EVENTS{NORMAL}"),
        ];
        activity.extend(connection_log_rows(server, body_height - upper - 6));
        devices.extend(panel(
            "ⓘ DEVICE ACTIVITY / Actions",
            right_width,
            body_height - upper,
            &activity,
        ));
        devices
    } else {
        panel(
            if desktop {
                "⇅ TRANSFER MANAGER   [X] Stop / Resume downloads"
            } else {
                "⇅ TRANSFER MANAGER   [T] Stop / Resume downloads"
            },
            right_width,
            body_height,
            &transfer_rows(&logs, right_width - 4, body_height - 2, None, desktop),
        )
    };
    let mut out = vec![
        format!(
            "{BOLD}[>_] TapSend v{}  -  Terminal File Transfer{NORMAL}",
            env!("CARGO_PKG_VERSION")
        ),
        "─".repeat(width),
    ];
    if desktop {
        out.extend(content);
        out.push(desktop_status(
            desktop_lan_state(server, stopped),
            &address,
            message,
            width,
        ));
        return out;
    }
    out.extend(join_panels(panel("", sidebar, body_height, &nav), content));
    #[cfg(windows)]
    let keys = "[1] Dashboard  [2] Send  [6] Receive  [3] Devices  [4] Transfers  [5] Messages  [P] Pair  [C] Camera  [G] Settings  [Q] Quit";
    #[cfg(not(windows))]
    let keys = "[1] Dashboard  [2] Send  [6] Receive  [3] Devices  [4] Transfers  [5] Messages  [P] Pair  [G] Settings  [Q] Quit";
    let available = width.saturating_sub(7);
    let footer = if available >= keys.len() + 5 {
        format!("{}  │  {}", keys, crop(message, available - keys.len() - 5))
    } else {
        crop(keys, available)
    };
    out.extend(panel("COMMANDS", width, 3, &[footer]));
    out
}

pub(crate) fn workspace(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing: u8,
    stopped: bool,
    page: usize,
    queue_cursor: usize,
    width: usize,
    height: usize,
) -> Vec<String> {
    workspace_mode(
        explorer,
        server,
        message,
        pairing,
        stopped,
        page,
        queue_cursor,
        width,
        height,
        false,
    )
}

#[cfg(windows)]
pub(crate) fn desktop_workspace(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing: u8,
    stopped: bool,
    page: usize,
    queue_cursor: usize,
    width: usize,
    height: usize,
    recipient: Option<&str>,
) -> Vec<String> {
    if pairing != 0
        && let Some(server) = server
        && width >= 36
        && height >= 10
    {
        let mut rows = vec!["TapSend Desktop".into(), "─".repeat(width)];
        let body = height - 3;
        let mut steps = vec![
            "1. Put your PC and phone on the same network.".into(),
            "2. On Android, open Tools → Connect a PC.".into(),
            "3. Tap Scan QR and point at this screen.".into(),
            String::new(),
            "[Y] Copy a pairing pass for pasting on Android.".into(),
            "[U] New pass   [M] More options   [P] Back".into(),
            "The QR and pass expire after 5 minutes.".into(),
        ];
        if pairing == 2 {
            steps.push(String::new());
            steps.extend(pairing_rows(server, width, body, true, true));
            steps.push("Pending approval? Press V, select the phone, then A.".into());
            rows.extend(panel("CONNECT A PHONE / MORE OPTIONS", width, body, &steps));
        } else {
            let qr = server.qr_code();
            let qr_width = qr
                .iter()
                .map(|line| line.chars().map(cell_width).sum::<usize>())
                .max()
                .unwrap_or(0);
            if !qr.is_empty() && qr.len() <= body.saturating_sub(2) && qr_width + 48 <= width {
                let left = (qr_width + 6).max(36);
                rows.extend(join_panels(
                    panel("SCAN THIS QR", left, body, &qr),
                    panel("CONNECT A PHONE", width - left - 1, body, &steps),
                ));
            } else {
                if !qr.is_empty()
                    && qr_width <= width - 4
                    && qr.len() + steps.len() <= body.saturating_sub(2)
                {
                    steps.extend(qr);
                } else {
                    steps.push("QR out of view? Press Y to copy the pass.".into());
                }
                rows.extend(panel("CONNECT A PHONE", width, body, &steps));
            }
        }
        rows.push(desktop_status(
            desktop_lan_state(Some(server), stopped),
            "",
            message,
            width,
        ));
        return rows;
    }
    if page == 1 && pairing == 0 && width >= 48 && height >= 12 {
        let body = height - 3;
        let mut rows = vec!["TapSend Desktop".into(), "─".repeat(width)];
        let browse = |width: usize, height: usize| {
            let mut rows = vec![crop(&explorer.directory().display().to_string(), width - 4)];
            rows.extend(file_rows_mode(
                explorer,
                width - 4,
                height.saturating_sub(3),
                true,
            ));
            panel("↑ FILES / Shift+S: search", width, height, &rows)
        };
        if width >= 86 {
            let left = width * 55 / 100;
            let right = width - left - 1;
            rows.extend(join_panels(
                browse(left, body),
                outgoing_panel(explorer, server, right, body, recipient),
            ));
        } else {
            let browser_height = body / 2;
            rows.extend(browse(width, browser_height));
            rows.extend(outgoing_panel(
                explorer,
                server,
                width,
                body - browser_height,
                recipient,
            ));
        }
        let address = server
            .and_then(ServerProcess::pairing_url)
            .unwrap_or_default();
        let address = address
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("");
        rows.push(desktop_status(
            desktop_lan_state(server, stopped),
            address,
            message,
            width,
        ));
        return rows;
    }
    workspace_mode(
        explorer,
        server,
        message,
        pairing,
        stopped,
        page,
        queue_cursor,
        width,
        height,
        true,
    )
}

fn desktop_lan_state(server: Option<&ServerProcess>, stopped: bool) -> &'static str {
    match server {
        None => "OFFLINE",
        Some(server) if !server.ready() => "STARTING",
        Some(_) if stopped => "PAUSED",
        Some(_) => "ONLINE",
    }
}

#[cfg(windows)]
fn outgoing_panel(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    width: usize,
    height: usize,
    recipient: Option<&str>,
) -> Vec<String> {
    let mut shares = server.map(ServerProcess::outgoing_rows).unwrap_or_default();
    shares.sort_by_key(|row| match row.state {
        SendState::Downloading(_) | SendState::Publishing | SendState::AwaitingReceipt => 0,
        SendState::Available
        | SendState::WaitingForPhone
        | SendState::WaitingForChoice
        | SendState::Interrupted
        | SendState::PublishFailed(_) => 1,
        SendState::Saved | SendState::Unconfirmed | SendState::Declined | SendState::Revoked => 2,
    });
    let selected: Vec<_> = explorer
        .selected_files()
        .into_iter()
        .filter(|path| {
            !shares
                .iter()
                .any(|row| row.path == *path && row.state == SendState::Publishing)
        })
        .collect();
    let marked = explorer.selected_count();
    let action = if marked == 0 {
        "Mark files with Space to queue them".into()
    } else if recipient.is_none() {
        "[V] Choose who gets the files".into()
    } else if server.is_some_and(ServerProcess::ready) {
        format!("[S] Send {marked} file(s)")
    } else {
        "Files marked · waiting for server".into()
    };
    let row = |path: &std::path::Path, bytes: u64, state: &str, to: &str| {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut name: String = name
            .chars()
            .map(|ch| if ch.is_control() { '�' } else { ch })
            .collect();
        if !to.is_empty() {
            name.push_str(" → ");
            name.push_str(to);
        }
        let size = human_size(bytes);
        let inner = width.saturating_sub(4);
        let state_width = (inner * 56 / 100).min(34);
        let file_width = inner.saturating_sub(state_width + size.len() + 3);
        format!(
            "{} {}  {}",
            text_cell(&name, file_width),
            size,
            crop(state, state_width)
        )
    };
    let mut visible: Vec<String> = selected
        .iter()
        .map(|path| {
            row(
                path,
                path.metadata().map(|meta| meta.len()).unwrap_or(0),
                "Selected",
                "",
            )
        })
        .collect();
    visible.extend(shares.iter().map(|entry| {
        let state = match &entry.state {
            SendState::Publishing => "Sending offer…".into(),
            SendState::WaitingForPhone => "Waiting for phone".into(),
            SendState::WaitingForChoice => "Waiting for response".into(),
            SendState::Declined => "Declined".into(),
            SendState::Revoked => "Device unpaired".into(),
            SendState::Available => "Accepted · waiting for download".into(),
            SendState::Downloading(percent) => {
                let filled = usize::from(*percent / 10);
                format!(
                    "[{}{}] {percent:>3}%",
                    "█".repeat(filled),
                    "░".repeat(10 - filled)
                )
            }
            SendState::AwaitingReceipt => "Waiting for save confirmation".into(),
            SendState::Saved => "✓ Saved on phone".into(),
            SendState::Unconfirmed => "Browser save unconfirmed".into(),
            SendState::Interrupted => "Interrupted · retry on phone".into(),
            SendState::PublishFailed(error) => format!("Share failed: {error}"),
        };
        row(&entry.path, entry.bytes, &state, &entry.recipient)
    }));
    let available = height.saturating_sub(4);
    let mut rows = vec![
        format!(
            "{BOLD}To{}  [V] Device{NORMAL}",
            recipient
                .map(|name| format!(" {name}"))
                .unwrap_or_else(|| " —".into())
        ),
        format!("{BOLD}{action}{NORMAL}"),
    ];
    if visible.len() > available && available > 0 {
        let hidden = visible.len() - (available - 1);
        visible.truncate(available - 1);
        visible.push(format!("{DIM}+ {hidden} more file(s){NORMAL}"));
    }
    rows.extend(visible.into_iter().take(available));
    panel("SEND QUEUE", width, height, &rows)
}

#[cfg(windows)]
pub(crate) fn desktop_startup_popup(
    width: usize,
    height: usize,
) -> (usize, usize, usize, Vec<String>) {
    let popup_width = width.min(42);
    let popup_height = height.min(4);
    (
        width.saturating_sub(popup_width) / 2,
        height.saturating_sub(popup_height) / 2,
        popup_width,
        panel(
            "TAPSEND",
            popup_width,
            popup_height,
            &["Starting server…".into()],
        ),
    )
}

fn desktop_status(state: &str, address: &str, message: &str, width: usize) -> String {
    let status = format!(" {state} / LAN ");
    let address = if address.is_empty() || address == "--" || address == "Not started" {
        String::new()
    } else {
        format!("  {address}")
    };
    let available = width.saturating_sub(status.len() + address.len() + 4);
    format!(
        "{INVERSE}{status}{NORMAL}  {}{DIM}{address}{NORMAL}",
        crop(message, available)
    )
}

#[cfg(windows)]
pub(crate) const DESKTOP_HELP_KEYS: [[(char, &str); 2]; 5] = [
    [('d', "Dashboard"), ('r', "Receive files")],
    [('f', "Files"), ('p', "Connect / pair")],
    [('v', "Choose device"), ('c', "Scan phone QR")],
    [('t', "Transfers"), ('g', "Settings")],
    [('m', "Messages"), ('w', "Windows settings")],
];

#[cfg(windows)]
pub(crate) fn desktop_help_popup(
    width: usize,
    height: usize,
) -> (usize, usize, usize, Vec<String>) {
    let popup_width = width.saturating_sub(4).clamp(4, 62).min(width);
    let popup_height = 13.min(height.saturating_sub(1)).min(height);
    let left = width.saturating_sub(popup_width) / 2;
    let top = (height.saturating_sub(popup_height) / 2).max(2);
    let column = popup_width.saturating_sub(4) / 2;
    let mut rows = vec![format!(
        "{DIM}{}ACTIONS{NORMAL}",
        text_cell("VIEWS", column)
    )];
    for [(left_key, left_name), (right_key, right_name)] in DESKTOP_HELP_KEYS {
        rows.push(format!(
            "{}[{}] {right_name}",
            text_cell(
                &format!("[{}] {left_name}", left_key.to_ascii_uppercase()),
                column
            ),
            right_key.to_ascii_uppercase()
        ));
    }
    rows.push("[Shift+S] Search   [Shift+V] Paste Stack".into());
    rows.push("[L] Screen share   [Shift+R] Drop into this folder".into());
    rows.push("[Shift+H] Hold file   [I] Updates".into());
    rows.push("[: relay] File Relay   [Shift+J] Pair from clipboard".into());
    rows.truncate(popup_height.saturating_sub(3));
    rows.push(format!(
        "{DIM}[N / Esc] Close   [K] Scratchpad   [Q] Quit{NORMAL}"
    ));
    (
        left,
        top,
        popup_width,
        panel("KEYBOARD SHORTCUTS", popup_width, popup_height, &rows),
    )
}

#[cfg(windows)]
pub(crate) fn desktop_update_popup(
    width: usize,
    height: usize,
    details: (String, String, &'static str),
) -> (usize, usize, usize, Vec<String>) {
    let popup_width = width.saturating_sub(4).min(64).max(4).min(width);
    let popup_height = 7.min(height);
    let content = [
        format!("{BOLD}{}{NORMAL}", details.0),
        details.1,
        String::new(),
        details.2.into(),
    ];
    (
        width.saturating_sub(popup_width) / 2,
        height.saturating_sub(popup_height) / 2,
        popup_width,
        panel(" GITHUB RELEASES ", popup_width, popup_height, &content),
    )
}

fn workspace_mode(
    explorer: &Explorer,
    server: Option<&ServerProcess>,
    message: &str,
    pairing: u8,
    stopped: bool,
    page: usize,
    queue_cursor: usize,
    width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    if width >= 130 && height >= 34 {
        return wide_workspace(
            explorer,
            server,
            message,
            pairing,
            stopped,
            page,
            queue_cursor,
            width,
            height,
            desktop,
        );
    }
    let names = ["DASHBOARD", "FILES", "DEVICES", "TRANSFERS", "MESSAGES"];
    let page = page.min(4);
    let mut lines = vec![format!(
        "{INVERSE}{BOLD} >_ TAPSEND {NORMAL} v{}  /  TERMINAL FILE TRANSFER",
        env!("CARGO_PKG_VERSION")
    )];
    if desktop {
        lines.push("─".repeat(width));
    }
    if width < 48 || height < 12 {
        lines.push(
            if desktop {
                "Enlarge the TapSend window"
            } else {
                "Enlarge terminal to at least 48 x 12"
            }
            .into(),
        );
        lines.push("[Q] quit".into());
        return lines;
    }
    let sidebar = if !desktop && width >= 100 { 24 } else { 0 };
    let content_width = width.saturating_sub(if sidebar > 0 { sidebar + 1 } else { 0 });
    let body_height = height.saturating_sub(if desktop { 3 } else { 4 });
    let logs = server.map(ServerProcess::logs).unwrap_or_default();
    let address = server
        .and_then(ServerProcess::pairing_url)
        .map(|url| {
            url.trim_start_matches("https://")
                .split('/')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_else(|| "Not started".into());
    let state = if stopped {
        "DOWNLOADS STOPPED"
    } else if server.is_some() {
        "LIVE / LAN"
    } else {
        "OFFLINE / LAN"
    };
    let content = if pairing != 0 && server.is_some() {
        panel(
            "CONNECT / P: QR - DETAILS - BACK",
            content_width,
            body_height,
            &pairing_rows(
                server.unwrap(),
                content_width.saturating_sub(4),
                body_height.saturating_sub(2),
                pairing == 2,
                desktop,
            ),
        )
    } else if page == 1 && content_width >= 95 {
        let list_width = content_width * 65 / 100;
        let detail_width = content_width - list_width - 1;
        let mut files = vec![crop(
            &explorer.directory().display().to_string(),
            list_width - 4,
        )];
        files.extend(file_rows(explorer, list_width - 4, body_height - 3));
        join_panels(
            panel("FILES / SPACE: MARK", list_width, body_height, &files),
            panel(
                "INSPECTOR",
                detail_width,
                body_height,
                &inspector_rows(explorer, detail_width - 4, body_height - 2),
            ),
        )
    } else if page == 0 {
        let top = 5.min(body_height);
        let queue_height = if body_height >= 22 { 7 } else { 0 };
        let middle = body_height.saturating_sub(top + queue_height + 6);
        let mut dashboard = panel(
            "NETWORK / LAST REPORTED SAMPLE",
            content_width,
            top,
            &[
                format!(
                    "TX {}   RX {}",
                    rate_text(*direction_samples(&logs, "TX").last().unwrap_or(&0.0)),
                    rate_text(*direction_samples(&logs, "RX").last().unwrap_or(&0.0))
                ),
                format!(
                    "RTT {}   SHARED {}   {state}",
                    rtt_samples(&logs)
                        .last()
                        .map(|n| format!("{n:.0} ms"))
                        .unwrap_or_else(|| "--".into()),
                    server.map(|s| s.shared_files().len()).unwrap_or(0)
                ),
            ],
        );
        dashboard.extend(panel(
            "TRANSFER MANAGER / Up/Down: SELECT   Space: OPEN SAVED RX",
            content_width,
            queue_height,
            &transfer_rows(
                &logs,
                content_width - 4,
                queue_height.saturating_sub(2),
                Some(queue_cursor),
                desktop,
            ),
        ));
        if content_width >= 74 && middle >= 6 {
            let left_width = content_width * 56 / 100;
            let right_width = content_width - left_width - 1;
            dashboard.extend(join_panels(
                panel(
                    "SHARED FILES / F: BROWSE",
                    left_width,
                    middle,
                    &shared_rows(server, left_width - 4, middle - 2),
                ),
                panel(
                    "NETWORK HISTORY / TX + RX",
                    right_width,
                    middle,
                    &graph_rows(&logs, right_width - 4, middle - 2),
                ),
            ));
        } else {
            dashboard.extend(panel(
                "SHARED FILES / F: BROWSE",
                content_width,
                middle,
                &shared_rows(server, content_width - 4, middle.saturating_sub(2)),
            ));
        }
        dashboard.extend(panel(
            "CONNECTION LOGS",
            content_width,
            body_height - top - middle - queue_height,
            &connection_log_rows(server, body_height - top - middle - queue_height - 2),
        ));
        dashboard
    } else {
        let rows = match page {
            1 => {
                let mut rows = vec![crop(
                    &explorer.directory().display().to_string(),
                    content_width - 4,
                )];
                rows.extend(file_rows(explorer, content_width - 4, body_height - 3));
                rows
            }
            2 => device_rows(server, content_width - 4, body_height - 2, desktop),
            3 if desktop => {
                let mut rows = vec!["[X] Stop / Resume downloads".into()];
                rows.extend(transfer_rows(
                    &logs,
                    content_width - 4,
                    body_height - 3,
                    None,
                    true,
                ));
                rows
            }
            3 => transfer_rows(&logs, content_width - 4, body_height - 2, None, false),
            _ => {
                let mut rows = vec![
                    if desktop {
                        "[E] Compose a message to the phone"
                    } else {
                        "[M] compose a message to the phone"
                    }
                    .into(),
                ];
                rows.extend(message_rows(
                    server,
                    content_width - 4,
                    body_height - 3,
                    desktop,
                ));
                rows
            }
        };
        panel(names[page], content_width, body_height, &rows)
    };
    if sidebar > 0 {
        let mut nav = vec![
            format!("{BOLD}{state}{NORMAL}"),
            address.clone(),
            "TLS / local network".into(),
            String::new(),
        ];
        for (index, name) in names.iter().enumerate() {
            nav.push(if index == page {
                format!("{INVERSE} {} {name:<16}{NORMAL}", index + 1)
            } else {
                format!(" {} {name}", index + 1)
            });
            nav.push(String::new());
        }
        nav.extend([
            " [P] Connect / Pair".into(),
            " [G] Settings".into(),
            " [?] Help".into(),
            String::new(),
            " Fast. Local. Secure.".into(),
        ]);
        lines.extend(join_panels(
            panel("WORKSPACE", sidebar, body_height, &nav),
            content,
        ));
    } else {
        lines.extend(content);
    }
    if desktop {
        let state = desktop_lan_state(server, stopped);
        lines.push(desktop_status(state, &address, message, width));
        return lines;
    }
    lines.push(format!(
        "{INVERSE} STATUS {NORMAL} {}",
        crop(message, width.saturating_sub(9))
    ));
    let actions = if page == 1 {
        "[↑↓] browse  [Space] mark  [S] share  [A] add  [Enter] open"
    } else {
        "[S] receive  [P] pair  [T] transfers  [M] message"
    };
    lines.push(crop(actions, width));
    lines.push(crop(
        "[1-5/Tab] views  [F] files  [D] devices  [G] settings  [;] commands  [Q] quit",
        width,
    ));
    lines
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

#[cfg(windows)]
pub(crate) fn recipient_rows(count: usize, cursor: usize, height: usize) -> std::ops::Range<usize> {
    let shown = count.min(height.saturating_sub(6)).min(10);
    let first = (cursor + 1)
        .saturating_sub(shown)
        .min(count.saturating_sub(shown));
    first..first + shown
}

pub(crate) fn overlay_popup(
    overlay: OverlayView<'_>,
    width: usize,
    height: usize,
) -> (usize, usize, usize, Vec<String>) {
    let popup_width = width.saturating_sub(4).min(72).max(4).min(width);
    let left = width.saturating_sub(popup_width) / 2;
    match overlay {
        #[cfg(windows)]
        OverlayView::PasteStack {
            cursor,
            items,
            status,
        } => {
            let shown = items.len().min(height.saturating_sub(7)).min(12);
            let first = (cursor + 1).saturating_sub(shown);
            let mut rows = vec![format!(
                "{DIM}{} ready · Android share → TapSend{NORMAL}",
                items.len()
            )];
            if items.is_empty() {
                rows.push("Waiting for screenshots or text from your phone.".into());
            }
            for (index, item) in items.iter().enumerate().skip(first).take(shown) {
                let kind = if item.file {
                    "FILE"
                } else if item.image {
                    "IMG"
                } else {
                    "TXT"
                };
                let label = if item.file {
                    item.name.clone()
                } else if item.image {
                    format!("{}  {}", item.name, item.preview)
                } else {
                    item.preview.clone()
                };
                let row = format!("{kind}  {label}  ·  {}", item.device);
                rows.push(if index == cursor {
                    format!("{INVERSE}▸ {row}{NORMAL}")
                } else {
                    format!("  {row}")
                });
            }
            if !status.is_empty() {
                rows.push(status.into());
            }
            rows.push("[↑↓] Select   [Enter] Copy   [Esc] Close".into());
            rows.push("[U / D] Reorder   [X] Move to end".into());
            rows.push("[H] Hold for folder   [J] Hold for phone   [B] Bring back all".into());
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("PASTE STACK", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::Scratchpad {
            cursor,
            entries,
            status,
        } => {
            let shown = entries.len().min(height.saturating_sub(5)).min(12);
            let first = (cursor + 1).saturating_sub(shown);
            let mut rows = vec![status.to_owned()];
            for (index, snippet) in entries.iter().enumerate().skip(first).take(shown) {
                let direction = if snippet.from_phone { "←" } else { "→" };
                let device: String = snippet
                    .device
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .collect();
                let text: String = snippet
                    .text
                    .chars()
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .collect();
                let line = format!("{direction} {device}  │  {text}");
                rows.push(if index == cursor {
                    format!("{INVERSE}{line}{NORMAL}")
                } else {
                    line
                });
            }
            if entries.is_empty() {
                rows.push("No snippets yet. Share clipboard text or send one from a phone.".into());
            }
            rows.push("[↑↓] Select  [Enter] Copy  [P] Share clipboard".into());
            rows.push("[R] Refresh   [Esc] Close".into());
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("SCRATCHPAD", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::FileSearch {
            explorer,
            query,
            matches,
            cursor,
        } => {
            let visible = matches.len().min(height.saturating_sub(7)).min(12);
            let first = (cursor + 1)
                .saturating_sub(visible)
                .min(matches.len().saturating_sub(visible));
            let directory: String = explorer
                .directory()
                .to_string_lossy()
                .chars()
                .filter(|ch| !ch.is_control())
                .collect();
            let mut rows = vec![
                format!("{DIM}{directory}{NORMAL}"),
                format!(
                    "{BOLD}Find{NORMAL}  {}",
                    if query.is_empty() {
                        "Type a file name…"
                    } else {
                        query
                    }
                ),
                format!(
                    "{DIM}{} match{} · {} marked{NORMAL}",
                    matches.len(),
                    if matches.len() == 1 { "" } else { "es" },
                    explorer.selected_count()
                ),
            ];
            if matches.is_empty() {
                rows.push(
                    if query.trim().is_empty() {
                        "No files in this folder."
                    } else {
                        "No matching files."
                    }
                    .into(),
                );
            } else {
                rows.extend(matches.iter().enumerate().skip(first).take(visible).map(
                    |(position, &index)| {
                        let entry = &explorer.entries()[index];
                        let name: String =
                            entry.name.chars().filter(|ch| !ch.is_control()).collect();
                        let mark = if explorer.selected(&entry.path) {
                            "[x]"
                        } else {
                            "[ ]"
                        };
                        if position == cursor {
                            format!("{INVERSE}▸ {mark} {name}{NORMAL}")
                        } else {
                            format!("  {mark} {name}")
                        }
                    },
                ));
            }
            rows.push("[↑↓] Move  [Enter] Mark  [Esc] Close".into());
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("SEARCH FILES", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::Recipient {
            devices,
            cursor,
            sending,
            status,
        } => {
            let mut rows = Vec::new();
            match devices {
                None => rows.push("Reading paired devices…".into()),
                Some(devices) if devices.is_empty() => {
                    rows.push("No paired devices. Press P to pair.".into())
                }
                Some(devices) => {
                    rows.extend(recipient_rows(devices.len(), cursor, height).map(|i| {
                        let entry = &devices[i];
                        let short: String = entry
                            .id
                            .chars()
                            .filter(|ch| !ch.is_control())
                            .take(8)
                            .collect();
                        let name: String =
                            entry.name.chars().filter(|ch| !ch.is_control()).collect();
                        let pending = if entry.pending {
                            "  ·  Needs approval"
                        } else {
                            ""
                        };
                        let line = format!(
                            "{}  {name}  {short}{pending}",
                            if i == cursor { "▸" } else { " " }
                        );
                        if i == cursor {
                            format!("{INVERSE}{line}{NORMAL}")
                        } else {
                            line
                        }
                    }));
                }
            }
            rows.push(
                if sending {
                    "[Click / Enter] Send to device"
                } else {
                    "[V / ↑↓] Move   [Click / Enter] Select"
                }
                .into(),
            );
            rows.push("[R] Rename  [X] Forget  [C] Clear all".into());
            rows.push("[A] Approve  [P] Pair  [U] Refresh  [Esc] Close".into());
            rows.push(format!("{DIM}{status}{NORMAL}"));
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("CHOOSE DEVICE", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::DeviceEdit {
            title,
            subtitle,
            input,
        } => {
            let mut rows = vec![subtitle];
            if let Some(value) = input {
                rows.push(format!("New name  {value}"));
                rows.push("[Enter] Save   [Esc] Cancel".into());
            } else {
                rows.push("[Y] Confirm   [Esc] Cancel".into());
            }
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel(title, popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::DesktopShare { files, scroll } => {
            let popup_height = (files.len().min(10) + 5).min(height);
            let shown = popup_height.saturating_sub(5);
            let start = scroll.min(files.len().saturating_sub(shown));
            let mut rows = vec![
                format!("{} file(s) selected in File Explorer", files.len()),
                String::new(),
            ];
            rows.extend(files.iter().skip(start).take(shown).map(|path| {
                path.to_string_lossy()
                    .chars()
                    .map(|ch| if ch.is_control() { '�' } else { ch })
                    .collect::<String>()
            }));
            rows.push("[Enter] Choose recipient    [Esc] Cancel".into());
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("SHARE WITH TAPSEND", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::PhoneDownload {
            name,
            index,
            count,
            done,
            total,
            existing,
        } => {
            let mut rows = vec![
                format!("Receiving file {index} of {count}"),
                name.to_owned(),
            ];
            rows.push(if total >= 0 {
                format!("{} / {}", human_size(done), human_size(total as u64))
            } else {
                format!("{} received", human_size(done))
            });
            if existing {
                rows.push("This name already exists. Saving a numbered copy.".into());
            }
            let popup_height = (rows.len() + 2).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            (
                left,
                top,
                popup_width,
                panel("RECEIVING FROM PHONE", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::PhoneDownloadDone { lines } => {
            let popup_height = (lines.len() + 3).min(height);
            let top = height.saturating_sub(popup_height) / 2;
            let mut rows = lines
                .iter()
                .take(popup_height.saturating_sub(3))
                .cloned()
                .collect::<Vec<_>>();
            rows.push("[Enter / Esc] Close".into());
            (
                left,
                top,
                popup_width,
                panel("PHONE TRANSFER RESULT", popup_width, popup_height, &rows),
            )
        }
        #[cfg(windows)]
        OverlayView::PhoneShare {
            files,
            cursor,
            stage,
        } => {
            let popup_width = width.saturating_sub(2).min(90).max(4).min(width);
            let left = width.saturating_sub(popup_width) / 2;
            let popup_height = (files.len().min(12) + 8)
                .clamp(9, height.saturating_sub(2).max(9))
                .min(height);
            let top = height.saturating_sub(popup_height) / 2;
            let mut rows = vec![
                format!("{BOLD}PHONE TO COMPUTER / {} FILE(S){NORMAL}", files.len()),
                "Destination: computer inbox (Settings > Computer inbox)".into(),
                String::new(),
            ];
            if stage == 0 || stage == 3 {
                rows.push(if stage == 0 {
                    "Waiting for approval on the phone.".into()
                } else {
                    "No files shared yet. Add files on the phone.".into()
                });
                rows.push("[R / Enter] Refresh without scanning again    [Esc] Cancel".into());
            } else {
                let visible = popup_height.saturating_sub(7);
                let start = if stage == 2 {
                    cursor.saturating_sub(visible.saturating_sub(1))
                } else {
                    0
                };
                for (index, (name, size, selected)) in
                    files.iter().enumerate().skip(start).take(visible)
                {
                    let name = crop(name, popup_width.saturating_sub(25));
                    let size = if *size < 0 {
                        "unknown".into()
                    } else {
                        human_size(*size as u64)
                    };
                    let line = format!(
                        "{} {:02}  {:<12} {name}",
                        if *selected { "[x]" } else { "[ ]" },
                        index + 1,
                        size
                    );
                    rows.push(if stage == 2 && index == cursor {
                        format!("{INVERSE}{line}{NORMAL}")
                    } else {
                        line
                    });
                }
                rows.push(String::new());
                rows.push(if stage == 1 {
                    "Download all files?  [Y] Yes   [N] Choose files   [Esc] Cancel".into()
                } else {
                    "[Up/Down] Select  [Space] Mark  [Enter] Download marked  [Esc] Cancel".into()
                });
            }
            (
                left,
                top,
                popup_width,
                panel("PHONE SHARE / LOCAL LAN", popup_width, popup_height, &rows),
            )
        }
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
            values,
            selected,
            edit,
            error,
            dirty,
            confirm_exit,
            first_run,
        } => {
            let popup_width = width.saturating_sub(2).min(96).max(4).min(width);
            let left = width.saturating_sub(popup_width) / 2;
            let popup_height = (if first_run { 13usize } else { 26 })
                .min(height.saturating_sub(2))
                .max(8)
                .min(height);
            let top = height.saturating_sub(popup_height) / 2;
            let labels = [
                "Computer inbox",
                "Starting folder",
                "Show hidden files",
                "Listen address",
                "Pairing host",
                "Direct joins",
                "Private data folder",
                "Upload limit (GiB)",
                "Portal Mode",
            ];
            let descriptions = [
                "Files sent from a phone are saved in this folder.",
                "Folder opened when the terminal interface starts.",
                "Show dotfiles when the terminal interface starts.",
                "Address and port used by the computer server.",
                "Host or IP used in QR links; blank uses automatic detection.",
                "Phones that can join a share before approval (1–8).",
                "Changing this path uses a different device identity after restart.",
                "Maximum size of each phone upload (1–64 GiB).",
                "Allow the local web Portal Mode on this computer.",
            ];
            let groups = [
                "TRANSFERS",
                "BROWSER",
                "BROWSER",
                "CONNECTION",
                "CONNECTION",
                "DEVICES",
                "STORAGE",
                "TRANSFERS",
                "PORTAL",
            ];
            let mut rows = Vec::new();
            let mut selected_row = 0;
            for &index in settings_fields(first_run) {
                if !first_run && (index == 0 || groups[index] != groups[index - 1]) {
                    rows.push(format!("{DIM}{}{}", groups[index], NORMAL));
                }
                if selected == index {
                    selected_row = rows.len();
                }
                let value = if selected == index {
                    edit.unwrap_or(&values[index])
                } else {
                    &values[index]
                };
                let display = match (index, value) {
                    (1, "") => "Current directory",
                    (2, "true") => "On",
                    (2, "false") => "Off",
                    (8, "true") => "On",
                    (8, "false") => "Off",
                    (4, "") => "Automatic",
                    _ => value,
                };
                let line = format!(
                    "  {:<19} {}",
                    labels[index],
                    crop(display, popup_width.saturating_sub(27))
                );
                rows.push(if selected == index {
                    format!("{INVERSE}{line}{NORMAL}")
                } else {
                    line
                });
            }
            let footer = if edit.is_some() {
                "ENTER accept value   ESC cancel edit"
            } else if first_run {
                "↑↓/TAB select   ENTER edit   S save & start"
            } else {
                "↑↓/TAB select   ENTER edit/toggle   D defaults   S save   ESC close"
            };
            let available = popup_height.saturating_sub(7);
            let start = selected_row.saturating_sub(available.saturating_sub(2));
            let mut content = vec![
                format!(
                    "{BOLD}{}{}{NORMAL}",
                    if first_run {
                        "CHOOSE STORAGE BEFORE CONNECTING"
                    } else {
                        "COMPUTER  /  PREFERENCES"
                    },
                    if dirty { "  •  UNSAVED" } else { "" }
                ),
                String::new(),
            ];
            content.extend(rows.into_iter().skip(start).take(available));
            content.resize(popup_height.saturating_sub(5), String::new());
            content.push(format!("{DIM}{}{NORMAL}", descriptions[selected.min(8)]));
            content.push(if confirm_exit || !error.is_empty() {
                format!(
                    "{BOLD}{}{NORMAL}",
                    crop(error, popup_width.saturating_sub(4))
                )
            } else {
                String::new()
            });
            content.push(format!("{DIM}{footer}{NORMAL}"));
            (
                left,
                top,
                popup_width,
                panel(
                    if first_run {
                        " FIRST-RUN SETUP "
                    } else {
                        "SETTINGS  /  G"
                    },
                    popup_width,
                    popup_height,
                    &content,
                ),
            )
        }
    }
}

fn device_rows(
    server: Option<&ServerProcess>,
    _width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let mut rows = vec![
        format!("{INVERSE}  DEVICES + JOIN REQUESTS  {NORMAL}"),
        String::new(),
    ];
    let Some(server) = server else {
        rows.push(format!("{BOLD}No device session{NORMAL}"));
        rows.push(
            if desktop {
                "[R] Start receiving from a phone"
            } else {
                "[6] Start receiving from a phone"
            }
            .into(),
        );
        rows.push(
            if desktop {
                "[F] Select a file and press S to share"
            } else {
                "[2] Select a file and press S to share"
            }
            .into(),
        );
        rows.push("Once sharing starts, [P] shows the pairing QR.".into());
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

fn graph_rows(logs: &[String], width: usize, height: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for direction in ["TX", "RX"] {
        let samples = direction_samples(logs, direction);
        rows.push(format!(
            "{BOLD}{direction}{NORMAL}  {}",
            rate_text(*samples.last().unwrap_or(&0.0))
        ));
        rows.push(sparkline(&samples, width));
    }
    if height >= 5 {
        rows.push(format!(
            "RTT {}",
            rtt_samples(logs)
                .last()
                .map(|v| format!("{v:.0} ms"))
                .unwrap_or_else(|| "--".into())
        ));
    }
    if height >= 6 {
        rows.push("History / auto-scaled / last reports".into());
    }
    rows
}

#[cfg(test)]
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
            "◆ {} {:>7}",
            text_cell(&name, width.saturating_sub(12)),
            crop(&size, 7)
        ));
    }
    rows.push(format!("{DIM}Browse left + press A to add another{NORMAL}"));
    rows
}

fn message_rows(
    server: Option<&ServerProcess>,
    _width: usize,
    height: usize,
    desktop: bool,
) -> Vec<String> {
    let mut rows = vec![
        format!("{DIM}TIMESTAMP   SENDER     MESSAGE{NORMAL}"),
        String::new(),
    ];
    let Some(server) = server else {
        rows.push(format!("{BOLD}No conversation yet{NORMAL}"));
        rows.push(
            if desktop {
                "[R] Start receiving, or [F] select files and press S."
            } else {
                "[6] Start receiving, or [2] select files and press S."
            }
            .into(),
        );
        rows.push("Then [P] pair your phone to exchange messages.".into());
        return rows;
    };
    let mut recent = Vec::new();
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
            let time = if line.len() >= 19 && line.as_bytes().get(4) == Some(&b'/') {
                &line[11..19]
            } else {
                "--:--:--"
            };
            recent.push(format!(
                "{DIM}{time}{NORMAL}   {BOLD}{from:<8}{NORMAL}  {content}"
            ));
            if recent.len() >= height.saturating_sub(rows.len()) {
                break;
            }
        }
    }
    if recent.is_empty() {
        rows.push(format!("{BOLD}Connected, waiting for a message{NORMAL}"));
        rows.push("Press M to send the first message to your phone.".into());
    } else {
        rows.extend(recent.into_iter().rev());
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
    let mut rows = vec![format!(
        "┌─ {BOLD}{title}{NORMAL} {}┐",
        "─".repeat(width.saturating_sub(title.chars().map(cell_width).sum::<usize>() + 5))
    )];
    for index in 0..height.saturating_sub(2) {
        rows.push(format!(
            "│ {} │",
            padded(
                content.get(index).map(String::as_str).unwrap_or(""),
                width - 4
            )
        ));
    }
    if height >= 2 {
        rows.push(format!("└{}┘", "─".repeat(width - 2)));
    }
    rows
}

fn file_rows(explorer: &Explorer, width: usize, height: usize) -> Vec<String> {
    file_rows_mode(explorer, width, height, false)
}

fn file_rows_mode(
    explorer: &Explorer,
    width: usize,
    height: usize,
    desktop_queue: bool,
) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    let mut rows = vec![format!(
        "{DIM}{}  FILES    {} MARKED{NORMAL}",
        explorer.entries().len(),
        explorer.selected_count()
    )];
    let list_height = height.saturating_sub(if desktop_queue { 1 } else { 4 });
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
                } else if !desktop_queue {
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
                } else {
                    String::new()
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
            " {symbol} {} {:>7}",
            text_cell(&entry.name, name_width),
            crop(&size, 7)
        );
        rows.push(if index == explorer.cursor() {
            format!("{INVERSE}{BOLD}{line}{NORMAL}")
        } else {
            line
        });
    }
    if desktop_queue {
        return rows;
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
                        rows.push(format!(
                            "{DIM}{:>3} │{NORMAL} {}",
                            index + 1,
                            crop_ellipsis(&clean, width.saturating_sub(6))
                        ));
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

#[cfg(test)]
fn parse_metric(line: &str, field: &str) -> Option<f64> {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(field))
        .and_then(|value| value.strip_suffix("B/s"))
        .and_then(|value| value.parse::<f64>().ok())
}

#[cfg(test)]
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
    desktop: bool,
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
    rows.push(
        if desktop {
            "Code lasts 5 min • [U] makes a new one"
        } else {
            "Code lasts 5 min • r creates a new one"
        }
        .into(),
    );
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
        if body.starts_with("TUI_JOB ")
            || body.starts_with("TUI_SHARE ")
            || body.starts_with("TUI_RECEIPT ")
            || body.starts_with("TUI_OFFER ")
            || body.starts_with("TUI_RESTORE ")
            || body.starts_with("TUI_SNIPPET ")
            || body.starts_with("TUI_SNIPPET_RESULT ")
            || body == "TUI_PASTE"
            || body.starts_with("TUI_PASTE ")
            || body.starts_with("TUI_PASTE_RESULT ")
            || body.starts_with("TUI_RECIPIENTS")
            || body.starts_with("TUI_PAIR_PASS")
            || body == "TUI_READY"
            || body.starts_with("HEALTH ")
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
    let mut command = media_preview_command("ffmpeg");
    command.args(["-v", "error", "-nostdin"]);
    if video {
        let duration = media_preview_command("ffprobe")
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

fn media_preview_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
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
    let mut used = 0;
    s.chars()
        .map(|ch| if ch.is_control() { '?' } else { ch })
        .take_while(|ch| {
            used += cell_width(*ch);
            used <= width
        })
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
    } else if matches!(n,0x1100..=0x115f|0x2329..=0x232a|0x2e80..=0xa4cf|0xac00..=0xd7a3|0xf900..=0xfaff|0xfe10..=0xfe19|0xfe30..=0xfe6f|0xff01..=0xff60|0xffe0..=0xffe6|0x1f000..=0x1faff|0x20000..=0x3fffd)
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
    fn reference_dashboard_contains_all_sections() {
        let explorer = Explorer::new(std::env::current_dir().unwrap()).unwrap();
        let rows = workspace(&explorer, None, "Ready", 0, false, 0, 0, 180, 48);
        let frame = rows.join("\n");
        for title in [
            "DASHBOARD",
            "TX / Upload",
            "RX / Download",
            "Total",
            "Devices",
            "Latency",
            "TRANSFER QUEUE",
            "DEVICES",
            "SHARED FILES",
            "NETWORK USAGE",
            "CONNECTION INFO",
            "CONNECTION LOGS",
            "COMMANDS",
        ] {
            assert!(frame.contains(title), "Missing {title}");
        }
        assert!(frame.contains("⌂"));
        assert!(frame.contains("┌"));
        assert_eq!(rows.len(), 48);
    }

    #[test]
    fn settings_icon_does_not_shift_sidebar_border() {
        assert_eq!(cell_width('▦'), 1);
        let explorer = Explorer::new(std::env::current_dir().unwrap()).unwrap();
        let rows = workspace(&explorer, None, "Ready", 0, false, 0, 0, 160, 45);
        let line = rows
            .iter()
            .find(|row| row.contains("Settings") && row.contains('▦'))
            .unwrap();
        let mut escape = false;
        let width: usize = line
            .chars()
            .filter(|ch| {
                if *ch == '\x1b' {
                    escape = true;
                    return false;
                }
                if escape {
                    if ch.is_ascii_alphabetic() {
                        escape = false;
                    }
                    return false;
                }
                true
            })
            .map(cell_width)
            .sum();
        assert_eq!(width, 160);
    }

    #[test]
    fn file_inspector_keeps_long_preview_inside_its_border() {
        let folder =
            std::env::temp_dir().join(format!("tapsend-ui-preview-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("long.txt");
        fs::write(&file, "x".repeat(500)).unwrap();
        let explorer = Explorer::new(folder.clone()).unwrap();
        let rows = inspector_rows(&explorer, 56, 18);
        assert!(rows.iter().any(|line| line.contains('…')));
        let bordered = panel("FILE INSPECTOR", 60, 20, &rows);
        for row in bordered {
            let mut escape = false;
            let visible: String = row
                .chars()
                .filter(|ch| {
                    if *ch == '\x1b' {
                        escape = true;
                        return false;
                    }
                    if escape {
                        if ch.is_ascii_alphabetic() {
                            escape = false;
                        }
                        return false;
                    }
                    true
                })
                .collect();
            assert_eq!(visible.chars().map(cell_width).sum::<usize>(), 60);
            assert!(visible.ends_with('│') || visible.ends_with('┐') || visible.ends_with('┘'));
        }
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn wide_messages_gives_pairing_and_composing_guidance() {
        let explorer = Explorer::new(std::env::current_dir().unwrap()).unwrap();
        let rows = workspace(&explorer, None, "Ready", 0, false, 4, 0, 160, 45);
        let frame = rows.join("\n");
        for label in [
            "CONVERSATION",
            "SESSION / Activity",
            "No conversation yet",
            "HOW TO MESSAGE",
            "COMPOSE",
            "[6] Receive",
        ] {
            assert!(frame.contains(label), "Missing {label}");
        }
        assert_eq!(rows.len(), 45);
    }

    #[test]
    fn settings_popup_shows_added_controls() {
        let values: [String; 9] = std::array::from_fn(|_| String::new());
        let (_, _, _, rows) = overlay_popup(
            OverlayView::Settings {
                values: &values,
                selected: 8,
                edit: None,
                error: "",
                dirty: false,
                confirm_exit: false,
                first_run: false,
            },
            160,
            45,
        );
        let frame = rows.join("\n");
        assert!(frame.contains("Portal Mode"));
        assert!(frame.contains("Upload limit"));
        assert!(frame.contains("Private data folder"));
    }

    #[test]
    fn workspace_fits_supported_sizes_and_pages() {
        let explorer = Explorer::new(std::env::current_dir().unwrap()).unwrap();
        for (width, height) in [(48, 12), (80, 24), (100, 30), (160, 45), (240, 60)] {
            for page in 0..5 {
                let rows = workspace(&explorer, None, "Ready", 0, false, page, 0, width, height);
                assert_eq!(rows.len(), height, "{width}x{height} page {page}");
                for row in rows {
                    let fitted = padded(&row, width);
                    assert!(!fitted.contains('\n'));
                    let mut escape = false;
                    let visible: String = row
                        .chars()
                        .filter(|ch| {
                            if *ch == '\x1b' {
                                escape = true;
                                return false;
                            }
                            if escape {
                                if ch.is_ascii_alphabetic() {
                                    escape = false;
                                }
                                return false;
                            }
                            true
                        })
                        .collect();
                    assert!(
                        visible.chars().map(cell_width).sum::<usize>() <= width,
                        "{width}x{height} page {page} exceeded its terminal width: {visible}"
                    );
                }
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn desktop_workspace_uses_full_width_and_a_status_only_footer() {
        let explorer = Explorer::new(std::env::current_dir().unwrap()).unwrap();
        for (width, height) in [(48, 12), (115, 26), (154, 38), (180, 48)] {
            for page in 0..5 {
                let rows = desktop_workspace(
                    &explorer, None, "Ready", 0, false, page, 0, width, height, None,
                );
                assert_eq!(rows.len(), height, "{width}x{height}, page {page}");
                assert!(rows.last().unwrap().contains("OFFLINE / LAN"));
                assert!(rows.last().unwrap().contains("Ready"));
                let view = rows.join("\n");
                assert!(!view.contains("NAVIGATION") && !view.contains("WORKSPACE"));
                assert!(!view.contains("[1-5/Tab]") && !view.contains("[6] Receive"));
            }
        }
        let (_, _, _, rows) = desktop_help_popup(48, 12);
        let help = rows.join("\n");
        for shortcut in [
            "[D] Dashboard",
            "[F] Files",
            "[V] Choose device",
            "[T] Transfers",
            "[M] Messages",
            "[W] Windows settings",
            "[N / Esc]",
        ] {
            assert!(
                help.contains(shortcut),
                "{shortcut} was missing from the popup"
            );
        }
    }
    #[test]
    fn transfer_rows_use_latest_request_and_unknown_lengths() {
        let logs = vec![
            "TUI_JOB id=1 dir=TX done=10 total=100 speed=5 state=active name=612e7a6970".into(),
            "TUI_JOB id=1 dir=TX done=100 total=100 speed=10 state=served name=612e7a6970".into(),
            "TUI_JOB id=2 dir=RX done=20 total=-1 speed=10 state=active name=62".into(),
        ];
        let jobs = transfers(&logs);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, 2);
        assert_eq!(jobs[1].name, "a.zip");
        assert_eq!(jobs[1].state, "served");
        let rows = transfer_rows(&logs, 100, 10, None, false);
        assert!(rows.iter().any(|s| s.contains("--%")));
        assert!(rows.iter().any(|s| s.contains("100%")));
    }

    #[test]
    fn dashboard_selection_only_opens_saved_rx() {
        let logs = vec![
            "TUI_JOB id=1 dir=TX done=10 total=10 speed=0 state=served name=612e7a6970".into(),
            "TUI_JOB id=2 dir=RX done=20 total=20 speed=0 state=saved name=622e706466".into(),
        ];
        assert_eq!(selected_received_name(&logs, 0).as_deref(), Some("b.pdf"));
        assert_eq!(selected_received_name(&logs, 1), None);
        let rows = transfer_rows(&logs, 100, 10, Some(1), false);
        assert!(rows[3].contains(INVERSE));
        assert!(!rows[2].contains(INVERSE));
    }

    #[cfg(windows)]
    #[test]
    fn phone_share_popup_offers_all_or_individual_downloads() {
        let files = vec![("a.txt".into(), 5, false), ("b.txt".into(), 7, true)];
        let (_, _, _, ask) = overlay_popup(
            OverlayView::PhoneShare {
                files: files.clone(),
                cursor: 0,
                stage: 1,
            },
            120,
            40,
        );
        assert!(ask.join("\n").contains("Download all files?"));
        let (_, _, _, choose) = overlay_popup(
            OverlayView::PhoneShare {
                files,
                cursor: 1,
                stage: 2,
            },
            120,
            40,
        );
        assert!(choose.join("\n").contains("[Space] Mark"));
    }

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
    fn first_run_settings_only_shows_storage_and_upload_choices() {
        let values = std::array::from_fn(|_| String::new());
        let (_, _, _, rows) = overlay_popup(
            OverlayView::Settings {
                values: &values,
                selected: 7,
                edit: None,
                error: "",
                dirty: false,
                confirm_exit: false,
                first_run: true,
            },
            100,
            25,
        );
        let text = rows.join("\n");
        for label in [
            "FIRST-RUN SETUP",
            "Computer inbox",
            "Private data folder",
            "Upload limit",
            "S save & start",
        ] {
            assert!(text.contains(label), "{label}");
        }
        assert!(!text.contains("Listen address"));
    }

    #[test]
    fn compact_settings_keeps_selected_field_visible() {
        let values = std::array::from_fn(|_| String::new());
        let (_, _, _, rows) = overlay_popup(
            OverlayView::Settings {
                values: &values,
                selected: 5,
                edit: None,
                error: "",
                dirty: false,
                confirm_exit: false,
                first_run: false,
            },
            65,
            10,
        );
        assert!(rows.iter().any(|row| row.contains("Direct joins")));
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
