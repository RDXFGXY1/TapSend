# TapSend v1.0.1

TapSend moves files and short messages between devices on the same local network. It has a terminal app for Linux and Windows, an Android app, and a phone browser interface. Transfers go directly between your devices; no TapSend account or cloud storage is needed.

## Fixed in v1.0.1

- A new computer IP address no longer leaves the pairing QR pointing at an address missing from the server certificate. Starting a share renews the server certificate for the current private IPv4 addresses.
- New installations retain the local CA signing key, so later IP changes keep the same CA trusted by phone browsers.
- Automatic pairing addresses now skip public and link-local IPv4 adapters that the Android app cannot use for local pairing.

After upgrading, restart the computer share and scan a fresh QR. If the server certificate changes, Android phones need to pair again. If this is the first IP change on an installation from v1.0.0, TapSend must also create a new local CA; phone browsers need to trust its new CA certificate. Windows Firewall must allow inbound connections on the current network profile.

## What you can do

- Browse files in the desktop terminal interface and share selected files with a phone.
- Send a file to one previously paired phone with `tabsend send`, or receive phone uploads with `tabsend receive`.
- Pair by QR code, link, or manual code. Paired devices can reconnect without scanning every time.
- Upload to the computer inbox, download shared files, and exchange messages from Android or a phone browser.
- Share files directly between two Android phones on the same Wi-Fi network or hotspot; recipients can also use a browser.
- Limit recipients, approve additional phones, pause desktop downloads, and revoke paired devices.
- Open **Portal Mode** on the computer, flick selected files upward on the phone, and watch each arrival animate after it is saved to the inbox.

## Downloads

| Device | File | What is inside |
| --- | --- | --- |
| Linux x86_64 | `TapSend-v1.0.1-linux-x86_64.tar.gz` | Portable desktop commands, terminal interface, server, and installer script |
| Windows x64 | `TapSend-v1.0.1-windows-x64.zip` | Portable desktop commands, terminal interface, and server |
| Android 13+ | `TapSend-v1.0.1-android.apk` | Signed Android app |

Download `SHA256SUMS.txt` alongside the package to verify its SHA-256 checksum.

## Get started

1. Install or extract the desktop package. Keep the desktop executables together if you use a portable archive.
2. Run `tapsend` to browse and share files, or run `tabsend receive` to accept files from a phone without selecting a computer file.
3. Pair your phone using the QR, link, or manual details shown by the computer. The computer and phone must be on the same reachable local network; allow inbound port `8443` on the computer.
4. To try Portal Mode, open `https://localhost:8443/portal.html` on the computer while TapSend is running. Choose **Portal files** on the phone and flick the file tile upward. Use your configured listen port if it differs from `8443`.

## Security and compatibility notes

- Computer sharing uses HTTPS and a local TapSend certificate authority. The Android app checks the certificate fingerprint from pairing. A phone browser may warn until you install and trust the TapSend CA certificate from the computer.
- Nearby phone-to-phone sharing uses local HTTP so browser recipients can join without certificate setup. Its traffic is unencrypted; use a network you trust.
- Portal arrival details and pairing QR are available only to a browser running on the computer.
- The Android release APK uses a release signing key. An older debug APK has a different signature and must be uninstalled before installing this release.
- The app supports Linux x86_64, Windows x64, and Android 13 or newer in this release. There is no macOS or iOS app in this package.
- The Linux archive uses static binaries. The Windows package is built on a Windows runner. Confirm pairing and transfers on physical Windows and Android devices before relying on this fix.

For controls, settings, and source build instructions, see [README.md](README.md).
