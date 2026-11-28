# TapSend console TUI

This page describes the console interface on Linux and Windows. On Windows, run `tapsend-tui.exe` inside Windows Terminal or Command Prompt. Use the [Windows desktop guide](windows.md) for the standalone `TapSend-Desktop.exe` window and its letter shortcuts.

On Windows, **C** scans a QR displayed by a phone that is hosting a nearby share. Press **Esc** to cancel or **R** to refresh the file list after the scan. TapSend downloads a single shared file directly. For multiple files, press **Y** to download all, or press **N** and use arrows and **Space** to choose files, then press **Enter** to download. Keep the scanner files alongside the TapSend executables. The scanner requires the .NET 8 Runtime and Windows camera permission. For computer pairing, scan the computer's QR from Android.

On Linux, build with
`cargo build --release --manifest-path tui/Cargo.toml` and
`go build -o tui/target/release/tapsend-server ./cmd/tapsend-server` and
`go build -o tui/target/release/tapsend ./cmd/tapsend`, then run
`./tui/target/release/tapsend` in an interactive terminal.

The terminal interface targets Linux terminals and Windows 10/11 VT-capable consoles.
Windows Terminal is recommended. No Nerd Font or GUI dependency is required.
Use at least 48 columns by 12 rows; the persistent sidebar appears at 100 columns.
The reference dashboard appears at 130 columns by 34 rows; 160 by 45 or larger is recommended. It includes Unicode icons, solid borders, five metric panels, transfer rows, a vertical network graph, device/file panels and connection logs.

| Key | Action |
| --- | --- |
| 1–5 / Tab | Dashboard, Files, Devices, Transfers, Messages |
| F / D | Files / devices; D also requests a device refresh |
| Up/Down, J/K | Browse files |
| Up/Down on Dashboard | Select a transfer in the queue |
| Space on Dashboard | Open the selected completed received file (RX) with the OS default app |
| Enter / Backspace | Open folder / parent folder |
| Space | Mark a file |
| S | Start receiving; on Files, start sharing selected/focused files |
| A | Add selected/focused files from Files while sharing |
| P | Cycle pairing QR, manual details, and current page |
| C (Windows) | Scan a phone-share QR with the laptop camera |
| R | New pairing code |
| T | Stop/resume downloads (not resumable per-file pause) |
| M | Compose a message |
| G / ? / ; | Settings / help / command palette |
| Q / Ctrl+C | Exit and restore terminal settings |

Settings controls the inbox, private data folder, starting folder, hidden files,
listen address, pairing host, direct join limit, upload size limit, and local
Portal Mode. Press G, use arrows and Enter to edit, and press S to save. Changing
the private data folder starts a new device identity after restarting the share.
Connection and upload settings apply to new shares, not an active session.

Transfer statistics come from the server. TX means computer to phone; RX means
phone to computer. Progress and ETA describe individual HTTP requests, including
parallel ranges, rather than claiming a whole file is complete. `served` means
the HTTP response finished; `saved` means an incoming file reached the inbox.
Unknown lengths display no percentage or ETA. Network history uses the recent
bounded log window, auto-scales, and shows last-reported rates. RTT is the latest
phone report; absent measurements remain unavailable. Saved devices are not
automatically labelled online.

Windows builds and terminal navigation were checked locally. Linux and Windows
CI checks are defined in `.github/workflows/tui-check.yml`; they run after these
changes are pushed. Linux runtime and separate Windows 10/11 device tests are
still required; CI runners do not replace those checks.

