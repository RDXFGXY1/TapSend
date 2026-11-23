# Planned CLI and settings

This document describes the TapSend command interface and settings. The basic commands and first two TUI settings are implemented. Later additions are identified below.

## Commands

```text
tabsend                                      Open the terminal interface
tabsend devices                              List paired devices and their IDs
tabsend send --device "My Phone" --path photo.jpg
tabsend send -d "My Phone" -p photo.jpg
tabsend send -p photo.jpg -d "My Phone"      Named options can appear in either order
tabsend send "My Phone" photo.jpg            Positional shorthand
tabsend send                                 Choose a device and file interactively
tabsend receive                              Start phone uploads with no PC file selected
tabsend status                               Show the active transfer and its result
```

`--device` (`-d`) accepts a saved device name or ID. A name with spaces must be quoted according to the user's shell. If two devices have the same name, TapSend must ask for an ID instead of guessing. Device names should be matched exactly; the device list can show the names and IDs.

`--path` (`-p`) accepts a local file path. It can be repeated to send several files. Named options can be mixed with a single positional value when the missing role is clear, for example `tabsend send -d "My Phone" photo.jpg`. If both values are positional, their order is always `DEVICE PATH`; guessing from whether a string looks like a file path would be unreliable. Paths with spaces must be quoted. The parser should support `--` before a positional path beginning with `-`.

When a value is missing in an interactive terminal, TapSend lists paired devices and prompts for the missing name or file path. In scripts and other noninteractive contexts, it prints usage and returns a nonzero exit code. `tabsend send --help` shows examples. A visual file picker remains a later improvement.

## Send flow

1. Resolve the device name to a unique paired device ID and validate each path before opening a share.
2. Start a share restricted to that device. A previously paired Android app reconnects without a new QR code.
3. Show an incoming offer in the Android app when it is open. The phone user accepts the download; automatic saving is a separate opt-in setting on the phone.
4. Show distinct states: `waiting for device`, `offered`, `downloading`, `saved`, `failed`, and `cancelled`. Report `saved` only after the phone confirms that it finished writing the file.
5. Keep the sender command running until completion or cancellation. A later background service can add queued sending while the command is closed.

## Terminal settings

The TUI has a **Settings** popup (press `g`) with visible inbox and connection values, validation, and save/cancel actions. Settings persist between runs and have the same meaning for TUI and CLI use. The other rows below are future improvements.

| Section | Setting | Default and behavior |
| --- | --- | --- |
| Transfers | Computer inbox folder | Current TapSend data directory's `inbox/`; controls files uploaded **to the computer** |
| Transfers (planned) | Default send device | None; can preselect a paired device without silently sending to it |
| Connection | Listen port | `8443`; validate availability before starting a share |
| Connection (planned) | Network interface | Automatic; display the address that phones must reach |
| Devices (planned) | Paired devices | Rename, inspect ID, and revoke access |
| Devices (planned) | Additional receiver approval | Keep the current approval behavior as the default |
| Notifications (planned) | Transfer and join notices | Show status in the terminal; desktop notifications can be optional |

The Android download folder is controlled by the Android app, not the computer TUI. The app currently uses `Downloads/TapSend`. A future Android setting may let the phone user choose another destination using Android's file picker.

## Platform packaging

- Windows: a manual GitHub Actions workflow builds an installer `.exe` that places the binaries together, adds `tabsend` to `PATH`, and creates a Start menu shortcut, plus a portable `.zip` with the same binaries. This workflow has not yet been run on a Windows runner.
- Linux: installable package or installer that puts `tabsend` on `PATH` and creates a desktop entry; portable archive with the binaries together.
- The Rust terminal now uses Windows console modes on Windows and Unix `stty` on Linux. Windows behavior still needs a successful runner build and hands-on verification.
