# Windows desktop

## Install and choose preferences

Run the Windows installer and choose where to install the program. The default is `C:\TapSend`. Choose a computer inbox, a private data folder, and a per-file upload limit from 1 to 64 GiB. The default limit is 8 GiB. If TapSend finds a saved profile, it offers to keep it. The inbox receives phone uploads, and private storage holds pairing and transfer state.

The installer has English and Arabic options. The destination screen shows whether the program folder already exists. You can keep your current storage and pairing settings and choose whether to add a desktop shortcut or command-line access.

Run the setup EXE by itself with an internet connection. When you click **Install**, setup downloads its matching package from GitHub Releases, verifies its checksum, and extracts the files into the program folder you selected. The desktop shortcut, command-line PATH entry, and opening the app after install are optional.

If you extract the portable ZIP instead, keep its files together and open `TapSend-Desktop.exe`. On a fresh profile, use the arrows and **Enter** to edit the First-Run Setup fields and press **S** when ready. TapSend starts its server once preferences are saved. After setup, it starts the server when you open the app. `tapsend-tui.exe` opens the [console version](tui-2.md) and uses its own controls.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| **N** or **?** | Open shortcut help |
| **D**, **F**, **R**, **T**, **M** | Dashboard, Files, Receive, Transfers, and Messages |
| **V** | Choose a phone. In the picker, use arrows or press **V** again to move, then **Enter** to select. |
| **P**, **U**, **Y** | Open pairing, refresh its QR/pass, or copy the pass from the pairing view |
| **Space**, **S** | Mark a file in Files, then send the marked files |
| **Shift+S** | Search by filename in the current folder. Mark results with **Enter**. |
| **C** | Scan a QR displayed by a phone that is hosting a nearby file share |
| **L** | Open or close [Screen Share](screen-share.md) |
| **K**, **Shift+V** | Open Scratchpad or Paste Stack |
| **Shift+R**, **Shift+H** | Set a temporary folder drop or hold a focused file |
| **Shift+J**, **:** | Pair File Relay from the clipboard or open the command input |
| **G**, **W** | Open TapSend settings or Windows options |
| **I** | Open GitHub release updates |
| **Q** | Quit TapSend |

## Check for updates

TapSend checks your repository's public GitHub releases when it starts after setup and every six hours while it is running. A newer stable Windows release with both the installer and ZIP shows **[I] UPDATE** in the title bar. When tray notifications are enabled, TapSend also shows a Windows notification. In the update panel, press **Enter** to open the official release and start the update with its installer when you are ready. Press **R** to check again.

## Receive in the background

1. Press **W** and enable **Receive in background and show notifications**.
2. Close the desktop window. TapSend stays in the notification area and can continue receiving.
3. Click the TapSend tray icon to reopen its window. To stop the server, right-click the icon and choose **Quit TapSend**.

## Send from File Explorer

1. Press **W** and enable **File Explorer Send to shortcut**.
2. In Explorer, select the files and right-click. On Windows 11, choose **Show more options** if needed.
3. Choose **Send to → TapSend Desktop**. Review the files and select a recipient before sending.

## Scan a phone's share QR

Start a nearby file share on the phone. Press **C** on Windows and point the camera at the phone's displayed QR. The scanner uses the terminal-style preview and closes after it recognizes the link. Use **Esc** to cancel. Windows camera permission, the .NET 8 Runtime, and the scanner files included in the bundle are required.

If the phone shares one file, TapSend downloads it to the computer inbox. For multiple files, press **Y** to download all, or press **N** to choose which ones to save. If the phone still needs to approve your computer, approve it there and press **R** to refresh.

To pair an Android phone with the computer, use the opposite direction. Press **P** on Windows and scan that QR with the phone.
