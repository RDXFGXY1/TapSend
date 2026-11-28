# TapSend quick start

TapSend shares files and messages between devices on the same local network. The computer app runs on Linux or Windows; phones can connect through a browser or the TapSend Android app. The Android app can also share files directly with another phone.

TapSend is in development. Try it with non-critical files before relying on it for important transfers.

## Install and run

Get the [Windows setup EXE](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64-setup.exe) or the [Linux installer](https://raw.githubusercontent.com/RDXFGXY1/TapSend/tap-send/install.sh). For Linux setup and the optional Android APK build, see the [installer guide](https://github.com/RDXFGXY1/TapSend/blob/tap-send/docs/development.md#interactive-linux-installer).

**Linux (x86_64 portable archive):** Extract the archive and run `bin/tapsend` from a terminal. To install the included binaries and an application-menu launcher for your user account, run `bash scripts/install-linux.sh` from the extracted folder. The four executables in `bin/` must stay together.

**Windows (x64 portable ZIP):** Extract the entire ZIP and open `TapSend-Desktop.exe`. Keep the executables together.

**Windows installer:** The default install directory is `C:\TapSend`. Choose where to save phone uploads, where to store TapSend's private data, and the maximum size of each incoming file. An existing installation can keep its settings. Run the setup EXE with an internet connection. After you click **Install**, setup downloads and verifies the package from GitHub Releases, then installs it into your chosen folder. For a portable first launch, press **S** on the First-Run Setup screen once the settings are ready.

The server generates pairing QR codes locally. The computer and phone must be on the same reachable local network, and the computer must allow inbound connections on port `8443`.

In TapSend Desktop, press **W** for Windows options. **Receive in background and show notifications** keeps TapSend in the tray when you close its window. To stop receiving and exit, right-click the tray icon and choose **Quit TapSend**. **File Explorer Send to shortcut** adds **Send to → TapSend Desktop** to the file context menu. TapSend shows the selected files for confirmation before sharing.

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

The [v1.0.0 Android APK](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.0/TapSend-v1.0.0-android.apk) works on Android 13 or newer. The current Linux installer can build a debug APK on x64 Linux. Android requires matching signing keys for an in-place update.

To share directly between phones, connect both to the same Wi-Fi network or hotspot. On the sending phone, choose **Send → Add files to phone share**. The other phone can scan the displayed QR code or open its link in a browser. Keep the sending app open. Nearby phone sharing uses unencrypted local HTTP, so use a network you trust.

For all controls, settings, and source-build instructions, see the [project README](https://github.com/RDXFGXY1/TapSend#readme).

## Windows Screen Share and file tools

- **Freeze & Grab.** Open **Screen Share**. Press **Space** and drag over an area. Press **G** to save the image or **T** to read text using Windows OCR. Open Paste Stack with **Shift+V**. Press **Space** again for the live feed.
- **Throw to This Folder.** Press **V** to choose a paired phone. Open **Files**, navigate to the destination, and press **Shift+R**. Send from Android and confirm the folder. The destination stays available for ten minutes. Duplicate files receive numbered names.
- **Hold This for Me.** In Paste Stack, press **H** to hold the selected item until you return to the folder open in Files, or **J** to hold it until the selected phone reconnects. In Files, **Shift+H** holds a reference to the focused file until you return. **B** in Paste Stack restores all held items.
- **Live Red Pen.** On Android, choose **Tools → Share screen with Live Red Pen** and allow TapSend to display over other apps. On the PC, right-drag on the live picture to send a temporary arrow.
- **File Relay.** Set up a writable Windows network share on a PC that will stay on. On the destination PC, run `:relay create \\relay-pc\share` in TapSend to copy a relay pass. Share the pass privately with the sending PC. Copy it there and press **Shift+J**. Focus a file in **Files** and run `:relay send`, then use `:relay status` to see its route. When TapSend runs on the destination PC again, it saves the file to the **Relay** folder inside its inbox and confirms the save.
