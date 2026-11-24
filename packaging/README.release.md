# TapSend v1.0.1 — quick start

TapSend shares files and messages between devices on the same local network. The computer app runs on Linux or Windows; phones can connect through a browser or the TapSend Android app. The Android app can also share files directly with another phone.

TapSend is in development. Try it with non-critical files before relying on it for important transfers.

## Install and run

**Linux (x86_64 portable archive):** Extract the archive and run `bin/tapsend` from a terminal. To install the included binaries and an application-menu launcher for your user account, run `bash scripts/install-linux.sh` from the extracted folder. The four executables in `bin/` must stay together.

**Windows (x64 portable ZIP):** Extract the entire ZIP and run `tapsend.exe` from a terminal. Keep all four `.exe` files together.

The four desktop executables must stay together; the server generates pairing QR codes without an extra program. The computer and phone must be on the same reachable local network, and the computer must allow inbound connections on port `8443`.

After the computer changes IP address, restart the share and scan a fresh QR in the Android app. On an existing v1.0.0 installation, the first such change also creates a new local CA; install the new CA certificate again if you use a phone browser. On Windows, allow TapSend through the firewall for the current network profile.

## Share with a phone

1. Run TapSend on the computer. Select a file with Space, then press `s` to start sharing. You can also focus one file and press `s`.
2. Scan the QR code with the Android app, open the pairing link in a phone browser, or enter the displayed pairing details manually in the app.
3. Download the selected files on the phone. To send files back, use **Send to computer** in the browser or **Send** in the Android app. Files arrive in the computer inbox, shown by `tabsend settings`.

To receive a phone upload without selecting a computer file, run `tabsend receive` in a terminal and keep it open until the upload finishes. On Linux portable, use `bin/tabsend receive`; on Windows portable, use `tabsend.exe receive` from the extracted folder.

For **Portal Mode**, open `https://localhost:8443/portal.html` on the computer while sharing or receiving. Scan the QR on that page, then choose files and flick upward in the Android app or phone browser. The portal shows each file when it is saved to the computer inbox.

Already paired a phone? Use `tabsend devices` to list it, then `tabsend send --device "My Phone" --path "/path/to/file"` to offer a file directly. The phone must be able to reach the computer while the command runs.

## Browser certificate

For computer-to-phone sharing, TapSend uses a local HTTPS certificate. A phone browser may show a certificate warning until you install and trust the TapSend CA certificate from `~/.local/share/tapsend/tapsend-ca.crt` on Linux or `%APPDATA%\TapSend\data\tapsend-ca.crt` on Windows. The Android app verifies the fingerprint supplied during pairing. Keep the server private key in that data folder private.

## Android app and phone-to-phone sharing

Install the [v1.0.0 Android APK](https://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.0) on Android 13 or newer until the v1.0.1 APK is available. Pair with a computer through **Connect**, then use **Send**, **Receive**, and **Messages**. QR scanning uses Google Play services; manual pairing and pasted links are also available. If you previously installed a debug APK, uninstall it first because the release uses a different signing key.

To share directly between phones, connect both to the same Wi-Fi network or hotspot. On the sending phone, choose **Send → Add files to phone share**. The other phone can scan the displayed QR code or open its link in a browser. Keep the sending app open. Nearby phone sharing uses unencrypted local HTTP, so use a network you trust.

For all controls, settings, and source-build instructions, see the [project README](https://github.com/RDXFGXY1/TapSend#readme).
