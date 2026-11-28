# Phone browser and Portal Mode

## Connect in a browser

1. Display the computer's pairing QR or link while TapSend is running.
2. Open the link in the phone browser from the same local network.
3. Use **Download** for shared files, the offer controls for targeted transfers, or **Send to computer** for uploads. The page also supports Messages and Scratchpad when available.

TapSend uses a local HTTPS certificate for computer connections. If the browser asks you to trust it, get `tapsend-ca.crt` from your own computer's TapSend data folder and install that CA certificate on the phone. Keep the computer's private key on the computer. See [Privacy and storage](privacy-and-storage.md) for paths.

When a browser manages the final save, TapSend can confirm it handed the file over. Check the browser's download list for the saved file. See [Download recovery](download-recovery.md) for interrupted downloads.

## Portal Mode

1. On the computer running TapSend, open `https://localhost:8443/portal.html`. If you changed the listen port, use that port.
2. Scan the portal's QR with Android or open its share link in a phone browser.
3. On the phone open Portal Mode, choose the files, and flick the send tile upward.
4. Watch the computer portal. It shows each arrival after the computer's inbox saves it.

The arrival feed and portal QR are available from the computer itself. If the portal is turned off, enable **Portal Mode** in TapSend Settings and restart the session.
