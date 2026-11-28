# CLI and settings

This document describes the command-line interface and persistent settings. For Windows desktop shortcuts, see [Windows desktop](windows.md).

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

`--device` (`-d`) accepts a saved device name or ID. Quote names containing spaces. If a name is ambiguous, use the ID shown by `tabsend devices`.

`--path` (`-p`) accepts a local file path. Repeat it for several files. You can mix named options with one positional value, as in `tabsend send -d "My Phone" photo.jpg`. With two positional values, use `DEVICE PATH` order. Quote paths containing spaces. Use `--` before a positional path beginning with `-`.

When a value is missing in an interactive terminal, TapSend prompts for it. In scripts, provide the recipient and file paths explicitly. Run `tabsend send --help` for examples.

## Send flow

1. Resolve the device name to a unique paired device ID and validate each path before opening a share.
2. Start a share restricted to that device. A previously paired Android app reconnects without a new QR code.
3. Review the offer on the phone and accept or decline it.
4. Use `tabsend status` to check progress. TapSend reports a confirmed save when the phone finishes writing the file.
5. Keep the sender command running until completion or cancellation. The standalone Windows app has a separate persistent [Send Later](transfers.md#send-later) queue.

## Terminal settings

Press **G** for Settings. Use arrows or Tab to move, Enter to edit or toggle, **D** to load defaults, **S** to save, and **Esc** to close. Press Esc again to discard unsaved changes. The CLI equivalents are `tabsend settings`, `tabsend settings set`, `tabsend settings defaults`, and `tabsend settings reset`.

| Section | Setting | Default and behavior |
| --- | --- | --- |
| Transfers | Computer inbox folder | Current TapSend data directory's `inbox/`; controls files uploaded **to the computer** |
| Browser | Starting folder | Current directory by default; must be an absolute path when set |
| Browser | Show hidden files | Off by default; applies when the TUI opens and immediately after saving |
| Connection | Listen address | `:8443` by default; binds the server to an address and port |
| Connection | Pairing host | Automatic by default; overrides the host/IP in pairing links and QR codes, without changing the bind address |
| Devices | Direct joins | `2` by default; number of paired phones that can join before sender approval, from 1 to 8 |
| Storage | Private data folder | Platform-specific TapSend data directory. See [Privacy and storage](privacy-and-storage.md). |
| Transfers | Upload limit | 8 GiB by default on the server. The setting accepts 1–64 GiB. Client limits can also apply. |
| Connection | Portal Mode | Enabled by default. Change it with `--portal-enabled=false` or through Settings. |

Connection and direct join changes take effect when a new share starts. The pairing host must be reachable from the phone and covered by the existing server certificate; TapSend reports a mismatch when a share starts. The starting folder and hidden-file preference apply to the terminal interface; the Android download folder remains controlled by the Android app.

Android downloads go to `Downloads/TapSend`. The computer inbox setting applies to uploads received by the computer.

## File Relay commands

The CLI exposes `relay create SHARED_FOLDER`, `relay connect` (reads the pass from standard input), `relay send FILE`, and `relay status`. See the [File Relay guide](file-relay.md) for the Windows walkthrough.
