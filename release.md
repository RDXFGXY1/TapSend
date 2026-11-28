# TapSend v1.0.2 for Windows

TapSend now opens in its own terminal-style Windows window and starts the local transfer server automatically after first-run setup.

## Download and install

- [Windows x64 setup EXE](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64-setup.exe) — Run it by itself with an internet connection. Setup downloads and verifies the package, then installs it in the folder you choose. The default is `C:\TapSend`.
- [Windows x64 portable ZIP](https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.2/TapSend-v1.0.2-windows-x64.zip) — Extract the full archive and open `TapSend-Desktop.exe`.

The terminal-style installer offers English and Arabic, an inbox and private storage location, a per-file upload limit, and optional desktop and command-line shortcuts. It can keep an existing profile. Windows can also notify you when a newer compatible release is available on GitHub.

## What's new

- **Desktop shortcuts and file search.** Use **D** for Dashboard, **V** for Devices, **N** for shortcut help, and **Shift+S** to search the current folder.
- **Send queue and Send Later.** Select a paired phone with **V**, mark files, and press **S**. The phone can accept or decline. The queue tracks transfer progress and phone-confirmed saves. Pending offers survive a desktop restart while the source files stay available.
- **Pairing and device management.** Pair with a QR or a copyable pairing pass. Rename or forget saved phones, or clear the full list. The Windows camera scanner also has a terminal-style preview for scanning a nearby share QR.
- **Windows integration.** Keep receiving from the tray, get arrival notifications, or queue files from File Explorer’s **Send to → TapSend Desktop**.
- **Paste Stack and screen sharing.** Open Android screenshots and text from **Shift+V** on Windows. Android can queue items while the PC is offline. Screen Share adds **Freeze & Grab** and **Live Red Pen** for captures, OCR, and temporary annotations.
- **More transfer tools.** Scratchpad keeps short notes and links handy. **Throw to This Folder** sends files into a specific temporary destination, **Hold This for Me** keeps an item handy, and **File Relay** uses a shared folder to hold deliveries between PCs.

## Fixes and improvements

- Buffered desktop drawing reduces flicker and blank frames during redraws.
- **V** opens the picker and advances the highlight when pressed again. Arrow keys move the highlight and **Enter** selects the phone.
- Pairing help now suggests checking the local network and Windows Firewall, or refreshing the QR after the computer’s IP changes.
- Eligible phone browser downloads can reuse saved chunks after a page reload when the browser retains the site's data and the file is unchanged.
- Android and Windows folder drops preserve files when names collide. Android reports the new numbered filename; transfer status distinguishes a confirmed save from a browser download whose save cannot be confirmed.
- Screen Share improves capture and playback pacing. Performance depends on the phone and network.

## Compatibility

The downloadable binaries in this release are for **Windows x64**. Features such as targeted save confirmation, Android Share to TapSend, Paste Stack, and Screen Share need a compatible Android build. Android and Linux users can [build the current source](https://github.com/RDXFGXY1/TapSend/blob/tap-send/docs/development.md) or use the [earlier public packages](https://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.0) for the features available in those builds.

[Windows guide](https://github.com/RDXFGXY1/TapSend/blob/tap-send/docs/windows.md) · [Pairing help](https://github.com/RDXFGXY1/TapSend/blob/tap-send/docs/pairing.md) · [Documentation](https://github.com/RDXFGXY1/TapSend/tree/tap-send/docs) · [Full changes since v1.0.1](https://github.com/RDXFGXY1/TapSend/compare/v1.0.1...v1.0.2)
