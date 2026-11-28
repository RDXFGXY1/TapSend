# Privacy and storage

## Connections

- **Computer ↔ Android or phone browser.** TapSend runs a local HTTPS server. Android checks the certificate fingerprint from pairing. A browser may require trust in the computer's local TapSend CA. Pairing links and passes are credentials. Share them privately and refresh them when needed.
- **Nearby phone sharing.** The share uses local HTTP. Use a trusted Wi-Fi network or hotspot and stop the share after use.
- **Screen Share.** Android prompts for capture and the chosen Windows viewer receives the frames. A regular session streams without automatic recording. A grab is stored on the PC only when you ask to save an image or recognized text. Live Red Pen also requires a separate Android opt-in.
- **File Relay.** The relay folder carries encrypted contents. The destination confirms the save with an authenticated receipt. Keep the relay pass private because it contains the route and the key needed to read that route's transfers.

## Where information is stored

| Location | What's there |
| --- | --- |
| Windows `%APPDATA%\TapSend\data\` by default | Computer certificate and private keys, paired devices, transfer records, outgoing offers, Scratchpad, and Paste Stack |
| Linux `~/.local/share/tapsend/` by default | Computer data for the terminal app and server |
| Computer inbox | Normal Android and browser uploads. Check **Settings → Computer inbox** for the actual path. File Relay saves under its `Relay` subfolder. |
| Android `Downloads/TapSend` | Finished downloads saved by the Android app |
| Android app-private storage | Pairing data, partial downloads, and pending Paste Stack deliveries |
| Phone browser site storage | Browser pairing and eligible partial download chunks |

For browser setup, move `tapsend-ca.crt` from your own computer to your phone. Keep `server.key` and the CA signing key private. If you change the private data directory or replace the certificate identity, use [Pairing and devices](pairing.md) to reconnect.
