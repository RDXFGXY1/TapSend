# Troubleshooting

## The phone cannot pair or reconnect

1. Confirm TapSend is running and both devices are on a local network that allows them to reach each other. A guest Wi-Fi network can isolate devices.
2. On Windows, check that the firewall allows TapSend on the active network profile and that the listen port, normally `8443`, is reachable.
3. If the computer's IP changed, restart TapSend and display a fresh pairing QR with **P** and **U**. Scan it again. If the wrong network adapter appears in the pairing link, set **Pairing host** to the correct local address in Settings.
4. If Android asks for approval, press **V** on Windows, highlight the request, and press **A**.
5. If camera scanning is unavailable, copy the pairing pass from the Windows pairing screen with **Y**. On Android, **Connect a PC → Why won’t it connect?** shows checks for the current connection.

## The app still looks like the older build

On Windows, quit the old instance from the notification area. Extract the new ZIP together and open the new folder's `TapSend-Desktop.exe`. If launching through Explorer, check the **Send to** shortcut too. On Android, check which APK you installed. See [Releases and downloads](releases.md) if Android rejects an update.

## A transfer seems stuck or the file is missing

- **Waiting for phone.** Open TapSend on the selected phone and respond to the offer. Keep the computer reachable and the original file in place.
- **Browser save unconfirmed.** Check the browser's downloads. The browser controls the final save when it takes over.
- **Saved under a different name.** Android and Windows folder drops keep the existing file and report the numbered copy. Check `Downloads/TapSend` on Android or the inbox or drop folder shown for a Windows upload.
- **Interrupted download.** Open the same file from the same connection and retry. See [Download recovery](download-recovery.md) for storage and revision requirements.
- **Folder drop expired.** Set the folder again with **Shift+R** and confirm the refreshed destination on Android.

## Screen Share is missing, blank, or slow

- Use the current builds on both devices. Click **Screen Share** or press **L** on Windows, choose the intended phone, then begin capture from **Tools** on Android.
- If the viewer does not open, make sure the full Windows ZIP was extracted and the .NET 8 Runtime is present.
- If the viewer opens but waits, confirm the Android capture prompt. Start a new session if Android has revoked that permission. Some protected apps can appear blank.
- If motion stutters, check local Wi-Fi quality and close competing large transfers. Frame rate varies with the devices and network.
- If arrows never appear, choose **Share screen with Live Red Pen** on Android, grant **Display over other apps**, and start the share again.

## File Relay still says Holding

Verify both computers can write to the same network share and that the relay computer is awake. Start TapSend on the destination and check the **Relay** folder inside its inbox. Run `relay status` on the sender again. A missing or invalid confirmation needs investigation before the sender can treat the transfer as saved.
