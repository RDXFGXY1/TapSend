# Releases and downloads

## Download TapSend

| Platform | Download | How to start |
| --- | --- | --- |
| Windows x64 | [v1.0.2 setup EXE](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64-setup.exe) · [Portable ZIP](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64.zip) | Run the EXE with an internet connection, or extract the complete ZIP and launch `TapSend-Desktop.exe`. |
| Linux x64 / ARM64 | [Interactive installer](https://raw.githubusercontent.com/RDXFGXY1/TapSend/tap-send/install.sh) · [v1.0.0 x64 archive](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.0/TapSend-v1.0.0-linux-x86_64.tar.gz) | The installer checks dependencies and builds the current console app. [Install guide](development.md#interactive-linux-installer). |
| Android 13+ | [v1.0.0 APK](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.0/TapSend-v1.0.0-android.apk) | To build a current debug APK and share it by QR, use the optional Android step on x64 Linux. |

## Current workspace builds

The following packages are in this checkout's `dist/` folder. Install the matching Windows and Android builds for the newest workflows.

| Platform | File |
| --- | --- |
| Windows x64 installer | [TapSend-v1.0.2-windows-x64-setup.exe](../dist/TapSend-v1.0.2-windows-x64-setup.exe) |
| Windows x64 portable ZIP and web installer package | [TapSend-v1.0.2-windows-x64.zip](../dist/TapSend-v1.0.2-windows-x64.zip) |
| Android 13+ | [TapSend-Android-Upload-Limits.apk](../dist/TapSend-Android-Upload-Limits.apk) |

The Windows setup EXE needs an internet connection and the matching ZIP published under GitHub release `v1.0.2`. End users run the EXE by itself. For a portable installation, extract the ZIP and run TapSend Desktop. The `dist/` files are local build outputs. See the published release links below or [build from source](development.md).

## Published on GitHub

| Release | Summary |
| --- | --- |
| **v1.0.2 Windows x64** | Standalone desktop TUI, web installer, send queue, device management, and background receiving. Download the [setup EXE](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64-setup.exe) or [portable ZIP](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64.zip). Read the [v1.0.2 release notes](https://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.2). |
| **v1.0.1 Windows IP pairing fix beta** | Windows ZIP with updated pairing IP and certificate handling. [v1.0.1 release notes and assets](https://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.1) |
| **v1.0.0** | The initial public packages for Linux x86_64, Windows x64, and Android 13+. Includes core local transfers, messages, pairing, and Portal Mode. [v1.0.0 release notes and assets](https://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.0) |

Compare a downloaded asset with the SHA-256 shown on its release page or in its checksum file, when available. On Windows, run `Get-FileHash <file> -Algorithm SHA256` in PowerShell.

## Update an existing installation

- **Windows.** Quit TapSend from the tray and run the updated setup EXE. Choose your install folder and keep your existing profile when prompted. For portable installs, extract the full updated ZIP and start `TapSend-Desktop.exe` from that folder. If you use Explorer **Send to**, check that its shortcut opens the updated location.
- **Android.** Android accepts an in-place update when the signing key matches the installed app. The workspace APK uses a debug signature. To switch between builds signed by different keys, uninstall the old app first. Uninstalling removes its app-private pairing and queued items. Pair again after installing the new build.

For release work, see [Release maintenance](releasing.md).
