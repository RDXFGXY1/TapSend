# TapSend

TapSend shares files and messages across devices on the same local network. Run the terminal app on a Linux or Windows computer to share selected files with a phone through a browser or the Android app. The Android app can also share files directly with another phone, without a computer or cloud service.

See [release.md](release.md) for v1.0.0 downloads, features, and setup.

> TapSend is in development. Try it with non-critical files before relying on it for important transfers.

## Features

- Browse and select files in a Rust terminal interface, then share them through a Go HTTPS server.
- Use `tabsend send` to offer a file to one paired phone by device name or ID.
- Pair phones with a QR code or manual code, and reconnect previously paired devices.
- Download files, upload to the computer's inbox, and exchange messages from the browser or Android app.
- Share files directly between phones on the same Wi-Fi network or hotspot. Recipients can use a browser or the Android app.
- Approve additional recipients, limit a share to one device, pause computer downloads, or revoke a paired device.
- Open **Portal Mode** on the computer for a live pairing QR and animated file arrivals; flick selected files upward from the Android app or phone browser.

## Requirements

| Component | Requirement |
| --- | --- |
| Computer | Linux or Windows; Go 1.24+ and Rust with edition 2024 support to build from source (`make` for Linux builds) |
| Terminal and portal QR code | Built in |
| File previews | `ffmpeg` and `ffprobe` (optional) |
| Android app | Android 13+; to build it, Android SDK platform 35, build tools 35.0.0, and Java 17 |

The computer and phone must be able to reach each other on the same local network. The computer's firewall must allow inbound connections to port `8443`.

## Build and run on Linux

```bash
make build
./bin/tapsend
```

Run those commands from the repository root. `make build` creates `bin/tapsend` and its `bin/tabsend` alias, plus `bin/tapsend-tui` (the terminal interface) and `bin/tapsend-server`. Keep them together. The terminal app finds the server beside its own executable, so you can launch it from another directory to browse that directory. Set `TAPSEND_SERVER` only if the server binary lives elsewhere.

### Send to a paired phone from the command line

```bash
./bin/tabsend devices
./bin/tabsend send --device "My Phone" --path "/home/me/photo.jpg"
./bin/tabsend send -p "/home/me/photo.jpg" -d "My Phone"
./bin/tabsend send "My Phone" "/home/me/photo.jpg"
./bin/tabsend status
```

Named options can appear in either order. Repeat `-p` or `--path` for several files. Quote device names and paths containing spaces. The positional form uses `DEVICE PATH` order. With a terminal attached, missing values prompt you to choose a device or enter a file path. The command stays open while the phone receives the offer and exits once the phone confirms it saved every file. Press Ctrl+C to cancel the share.

The Android app shows **Incoming offer for this phone** and an **Accept & Save** button for targeted sends. The phone must be paired already and able to reach the computer. TapSend does not start a background queue when the command is closed.

### Receive from a phone without sharing a computer file

```bash
tabsend receive
```

This starts the server with an empty shared-file list and prints pairing details. On a previously paired phone, open TapSend and use **Send → Choose files**; a new phone can pair using the displayed link or code first. Files arrive in the computer inbox shown by `tabsend settings`. Keep the command running until the upload finishes, then press Ctrl+C. In the TUI, press `s` while no file is focused, or open the command menu (`;`) and choose `receive` from any folder. You can add files to a receive-only session later with `a`.

For Portal Mode, open `https://localhost:8443/portal.html` on the computer while `tabsend receive` or a TUI share is running. Use your configured listen port if it is not `8443`. Scan the QR in the portal with the Android app, or open its pairing link in the phone browser. On Android, use **Send → Choose portal files**, then flick the file tile upward. In the phone browser, use **Portal Mode → Choose portal files**. The portal animates when each file has been saved to the computer inbox. Its arrival feed is available only from the computer itself. Pairing codes expire after five minutes; use **New pairing QR** when needed.

### Settings

Press `g` in the terminal interface to open Settings. Use ↑/↓ or Tab to select a row, Enter to edit or toggle it, `d` to load defaults, and `s` to save. Press Esc to close; if you changed a value, press Esc again to discard it. The settings include the computer inbox, starting browser folder, hidden files, listen address, pairing link host, and the number of phones that may join directly before approval. A blank starting folder uses the current working directory; a blank pairing host uses automatic detection.

The same values are available through `tabsend settings`, `tabsend settings defaults`, and `tabsend settings reset`. For example, `tabsend settings set --inbox /absolute/path --start-folder /home/me/Pictures --show-hidden=true --listen :8443 --direct-joins 2`. Use `--pairing-host` only when automatic detection picks the wrong address; the chosen IP or hostname must be reachable from the phone and covered by the existing server certificate. TapSend reports a certificate mismatch when starting a share. Restart an active share after changing connection or join settings. The phone's `Downloads/TapSend` folder is controlled by the Android app.

### Linux installation and portable archive

Run `make install-linux` to install the binaries in `~/.local/bin` and create a **TapSend** launcher in your desktop application menu. If that folder is on your `PATH`, you can then type `tabsend` or `tapsend` from any terminal. Run `make portable-linux` to create a `.tar.gz` archive under `dist/`; extract it and run `bin/tabsend` without installing.

For Windows, the **Build release packages** GitHub Actions workflow builds `TapSend-v1.0.0-windows-x64-setup.exe` and `TapSend-v1.0.0-windows-x64.zip`. The installer places TapSend in the current user's app folder, adds its folder to the user's `PATH`, and creates a Start menu shortcut. The portable archive runs from an extracted folder. The Windows console backend and packaging are in the repository; use the release workflow results to verify the Windows build.

In the terminal, focus a file and press `s` to share it. Use Space to select multiple files first. Scan the displayed QR code with the Android app, or open the pairing link in a phone browser. You can also enter the displayed address, code, and security key manually in the app. Press `r` to issue a fresh pairing code.

If the QR does not appear, use the displayed pairing link or manual details. A previously paired phone can reconnect while a share is running.

### Browser certificate setup

TapSend creates a local certificate authority at `~/.local/share/tapsend/tapsend-ca.crt` on Linux or `%APPDATA%\TapSend\data\tapsend-ca.crt` on Windows. A phone browser may warn about the server certificate until you install **that CA certificate** on the phone and trust it. On Android, look for **Settings → Security → Install a certificate → CA certificate**; the exact labels vary by version. Reopen the browser and scan a fresh QR afterward.

The Android app verifies the server fingerprint from the pairing QR or manual security key. Keep the server's private key (`server.key` in the same data folder) private. If you replace the local CA or server certificate, pair again with a fresh QR.

## Terminal controls

| Key | Action |
| --- | --- |
| `↑` / `↓` or `j` / `k` | Move through files |
| `Enter` / `Backspace` | Open a directory / go to its parent |
| `Space` | Select or deselect a file |
| `s` | Share the focused or selected files |
| `a` | Add files to the running share |
| `f` / `d` | Show files / saved devices |
| `h` | Show or hide dotfiles |
| `p` | Cycle pairing details and dashboard views |
| `t` | Pause or resume computer downloads |
| `r` | Generate a new pairing code |
| `;` or `:` | Open the command menu |
| `?` | Show keyboard help |
| `g` | Open settings |
| `q` | Quit and stop the server |

The command menu supports actions such as `add /path/to/file`, `message Hello`, `devices`, `approve DEVICE_ID`, `deny DEVICE_ID`, `recipient DEVICE_ID`, `recipient all`, and `revoke DEVICE_ID`. Use `devices` to find IDs and pending requests. The first two distinct phones can join a computer share directly; additional phones need approval in the terminal.

## Phone options

### Browser

Open the pairing link and choose **Download** for a shared file. **Send to computer** uploads files to the configured computer inbox (`~/.local/share/tapsend/inbox/` by default). The **Messages** section exchanges messages with the terminal. Browser retry can reuse completed chunks while the page remains open for supported file sizes; it does not provide permanent resume after a page reload.

### Android app

Build a debug APK with:

```bash
make android
```

The APK is written to `bin/tapsend-android-debug.apk`. You can also open `android/` in Android Studio and build the `app` module. Install the APK on Android 13 or newer, then use **Connect → Scan QR**, **Pair manually**, or **Pair from link**. QR scanning uses Google Play services; the manual and pasted-link options remain available if scanning is unavailable.

The app's **Send** and **Receive** tabs handle transfers, **Connect** manages computer pairing, and **Messages** handles chat. Downloads are saved under `Downloads/TapSend`; files sent to the computer arrive in its TapSend inbox.

### Phone-to-phone sharing

Connect both phones to the same Wi-Fi network or hotspot. On the sending phone, open **Send → Add files to phone share**. A recipient can scan the displayed QR code in a browser, or use **Receive → Scan QR** or the pasted link in the Android app. Keep the sending app open while sharing. Additional recipients beyond the first two need the sender's approval. Stop the share in the app to invalidate its link.

Nearby phone shares use local HTTP so browsers can join without certificate setup. The link is the access credential and the traffic is unencrypted, so use a network you trust.

## Development

```bash
make test
node --check internal/server/web/app.js
```

`make test` runs the Go and Rust tests. The JavaScript syntax check requires Node.js. `make fmt` formats the Go and Rust sources.

| Path | Purpose |
| --- | --- |
| `tui/` | Rust file browser and terminal dashboard |
| `cmd/tapsend-server/` | Go server entry point |
| `internal/` | HTTPS server, pairing, device records, certificates, files, and phone web page |
| `android/` | Android app and nearby phone sharing |

TapSend stores certificates, paired-device records, transfer history, and uploaded files under `~/.local/share/tapsend/` by default. The computer inbox can be moved in Settings. Build outputs and private data are excluded from Git. The server shares only files selected by the computer user. Browser downloads are confirmed when data reaches the browser; saving the file to permanent storage depends on the browser.
