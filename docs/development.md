# Development

## Source layout

| Path | Responsibility |
| --- | --- |
| [tui/](../tui/) | Rust terminal UI, Windows desktop host, file picker, and desktop controls |
| [cmd/tapsend/](../cmd/tapsend/) | Go command-line launcher and commands |
| [cmd/tapsend-server/](../cmd/tapsend-server/) | Local server entry point |
| [internal/server/](../internal/server/) | HTTPS transfers, pairing routes, desktop control, Paste Stack, and Screen Share |
| [internal/server/web/](../internal/server/web/) | Phone browser and Portal Mode |
| [internal/relay/](../internal/relay/) | Encrypted File Relay queue and save confirmations |
| [android/](../android/) | Android sharing, pairing, recovery, capture, and annotations |
| [camera-scanner/](../camera-scanner/) | Windows QR scanner and live screen viewer |

## Build

Requirements for this checkout include Go 1.24 or newer, Rust with edition 2024 support, and platform build tools. The Windows scanner and screen viewer use the .NET 8 SDK to build and the .NET 8 Runtime to run. Android uses Java 17, SDK Platform 35, and Android Build Tools 35.0.0.

On Linux, run `make build` and launch `./bin/tapsend`. Run `make portable-linux` after installing the `x86_64-unknown-linux-musl` Rust target to create the portable archive.

On Windows, run these commands from the repository root.

```powershell
New-Item -ItemType Directory -Force dist/windows | Out-Null
go build -o dist/windows/tapsend.exe ./cmd/tapsend
go build -o dist/windows/tapsend-server.exe ./cmd/tapsend-server
cargo build --release --manifest-path tui/Cargo.toml
dotnet publish camera-scanner/TapSend.CameraScanner.csproj -c Release -r win-x64 --self-contained false -o dist/windows/camera
```

Copy `tui/target/release/tapsend-desktop.exe` into `dist/windows` as `TapSend-Desktop.exe`. Copy the Rust `tapsend.exe` into the same folder as `tapsend-tui.exe`. Put the published camera helper files directly alongside the desktop EXE and preserve their runtime folder structure.

For Android, open `android/` in Android Studio and build the `app` module, or run `./gradlew :app:assembleDebug` from that directory on a shell that can run the checked-in wrapper. The APK is under `android/app/build/outputs/apk/debug/`.

## Verify changes

```text
go test ./...
cargo test --manifest-path tui/Cargo.toml
node --check internal/server/web/app.js
node --check internal/server/web/portal.js
```

Smoke-test the relevant workflow using the packaged binaries. An emulator can test Android UI and capture. Check Wi-Fi behavior, camera permissions, save handling, and installation on the devices you plan to support before publishing.

See [Release maintenance](releasing.md) for packaging and signing.
