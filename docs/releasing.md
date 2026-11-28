# Release maintenance

Use [Releases and downloads](releases.md) for the published release history and workspace builds.

## Prepare a version

Update Android's `versionCode` and `versionName`, the Rust package version, the Go CLI version, the Windows installer version, archive names, and release notes together. Run the checks in [Development](development.md).

## Build the packages

- **Linux.** Run `make portable-linux` after installing the `x86_64-unknown-linux-musl` Rust target. Check that the archive includes the CLI, terminal app, server, and installer script.
- **Windows.** Run the [Windows package workflow](../.github/workflows/windows-package.yml) with the version you plan to tag. It packages the Go, Rust, and .NET files and pins the ZIP's checksum inside the installer. For a local build, follow [Development](development.md) to stage the Windows files, then run `pwsh -File packaging/build-windows-installer.ps1 -ReleaseVersion 1.0.2`. Upload the matching ZIP, installer, and checksum file to the same GitHub release tag, with their generated filenames intact.

- **Android.** Assemble the release APK using the existing signing key and check it with `apksigner verify --verbose`. Keep the keystore and passwords private and backed up. Updates to an installed release need the same key.

Windows update notifications look for a stable public release tagged `vMAJOR.MINOR.PATCH` that has both the installer and matching ZIP. The packaging workflow stamps the GitHub repository and version into the Windows CLI used by the desktop updater. For a different version in a local build, set both values on the Go CLI using `-ldflags "-X main.releaseVersion=VERSION -X main.releaseRepository=OWNER/REPO"` before packaging. Add your own `packaging/tapsend.ico` to use it in the setup EXE, installed shortcuts, desktop window, and notification area.

## Verify and publish

Install or extract the packages on their target devices. Verify startup, pairing, sends in both directions, save confirmations, and any features listed in the release notes. On Windows, open the actual packaged EXE and verify that Screen Share loads with its companion files.

Generate `SHA256SUMS.txt` for the final assets. Publish the verified packages and checksums with the intended tag. List which platforms are available and identify which workflows the packages contain.
