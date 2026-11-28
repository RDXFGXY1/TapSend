# Sending and receiving files

## Send to a specific phone

1. Press **V** on Windows and select a paired phone with **Enter**.
2. Press **F** and browse to the files. Use **Space** to mark each file. To search the current folder, press **Shift+S**, type part of a filename, mark matches with **Enter**, and close search with **Esc**.
3. Press **S** to send the marked files. The target can accept or decline the offer.
4. Check the queue in Files or open the **Transfers** view with **T**.

| State | What happened |
| --- | --- |
| **Waiting for phone** | The offer is queued for the paired phone. It can respond when it opens TapSend again. |
| **Accepted / downloading** | The recipient accepted the offer. TapSend reports progress as data transfers. |
| **Saved** | TapSend received confirmation that the destination saved the file. |
| **Browser save unconfirmed** | The download reached the browser, which took over saving the file. Check the browser's downloads. |
| **Declined / failed** | The recipient declined, the file changed or became unavailable, or the transfer hit an error. Check the row for details. |

## Send Later

Use the same **V → F → Space → S** flow while the phone is offline. Desktop offers survive a restart. Keep the source files in place and unchanged until the phone confirms the save. The PC must be reachable again for the phone to accept and download. For PC-to-PC delivery while the destination is offline, see [File Relay](file-relay.md).

## Receive from Android

Open TapSend on Windows and leave it running. On the phone choose **Computer → Send files**, or share files to TapSend from another app. TapSend normally saves uploads in the computer inbox configured under **G**. **R** opens the Receive view. With a [folder drop](folder-drop.md), the Android confirmation shows the chosen destination before sending.

Android downloads are saved under `Downloads/TapSend`. When a name is taken, TapSend keeps the existing file and reports the numbered name it saved. A folder drop on Windows also preserves the existing file. The normal Windows inbox uses a unique incoming filename.

For browser-based uploads and the Portal Mode arrival display, see [Phone browser](browser.md).
