# TapSend

TapSend shares files and messages across devices on the same local network. Run the terminal app on a Linux computer to share selected files with a phone through a browser or the Android app. The Android app can also share files directly with another phone, without a computer or cloud service.

> TapSend is in development. Try it with non-critical files before relying on it for important transfers.

## Features

- Browse and select files in a Rust terminal interface, then share them through a Go HTTPS server.
- Pair phones with a QR code or manual code, and reconnect previously paired devices.
- Download files, upload to the computer's inbox, and exchange messages from the browser or Android app.
- Share files directly between phones on the same Wi-Fi network or hotspot. Recipients can use a browser or the Android app.
- Approve additional recipients, limit a share to one device, pause computer downloads, or revoke a paired device.

## Requirements

| Component | Requirement |
| --- | --- |
| Computer | Linux, Go 1.24+, Rust and Cargo with edition 2024 support, and `make` |
| Terminal QR code | `qrencode` |
| File previews | `ffmpeg` and `ffprobe` (optional) |
| Android app | Android 13+; to build it, Android SDK platform 35, build tools 35.0.0, and Java 17 |

The computer and phone must be able to reach each other on the same local network. The computer's firewall must allow inbound connections to port `8443`.

## Build and run

```bash
make build
TAPSEND_SERVER="$PWD/bin/tapsend-server" ./bin/tapsend
```

Run those commands from the repository root. `make build` creates `bin/tapsend` and `bin/tapsend-server`. To browse a different directory, start the second command from that directory and set `TAPSEND_SERVER` to the **absolute path** of the server binary.

In the terminal, focus a file and press `s` to share it. Use Space to select multiple files first. Scan the displayed QR code with the Android app, or open the pairing link in a phone browser. You can also enter the displayed address, code, and security key manually in the app. Press `r` to issue a fresh pairing code.

If the QR does not appear, install `qrencode` and use the displayed pairing link or manual details in the meantime. A previously paired phone can reconnect while a share is running.

### Browser certificate setup

TapSend creates a local certificate authority at `~/.local/share/tapsend/tapsend-ca.crt`. A phone browser may warn about the server certificate until you install **that CA certificate** on the phone and trust it. On Android, look for **Settings → Security → Install a certificate → CA certificate**; the exact labels vary by version. Reopen the browser and scan a fresh QR afterward.

The Android app verifies the server fingerprint from the pairing QR or manual security key. Keep `~/.local/share/tapsend/server.key` private. If you replace the local CA or server certificate, pair again with a fresh QR.

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
| `q` | Quit and stop the server |

The command menu supports actions such as `add /path/to/file`, `message Hello`, `devices`, `approve DEVICE_ID`, `deny DEVICE_ID`, `recipient DEVICE_ID`, `recipient all`, and `revoke DEVICE_ID`. Use `devices` to find IDs and pending requests. The first two distinct phones can join a computer share directly; additional phones need approval in the terminal.

## Phone options

### Browser

Open the pairing link and choose **Download** for a shared file. **Send to computer** uploads files to `~/.local/share/tapsend/inbox/`. The **Messages** section exchanges messages with the terminal. Browser retry can reuse completed chunks while the page remains open for supported file sizes; it does not provide permanent resume after a page reload.

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

TapSend stores certificates, paired-device records, transfer history, and uploaded files under `~/.local/share/tapsend/` by default. Build outputs and private data are excluded from Git. The server shares only files selected by the computer user. Browser downloads are confirmed when data reaches the browser; saving the file to permanent storage depends on the browser.
