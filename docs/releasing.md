# Release maintenance

The v1.0.0 packages are built by `.github/workflows/release-build.yml`. Run that workflow from the release commit, download its three artifacts, verify the APK signature and archive contents, then publish a GitHub release tagged `v1.0.0` with the four files and `release.md` as its notes.

The Android APK is signed with the keystore at `android/.android-user/release.keystore`. Its credentials are in `android/.android-user/release.properties`. Both are ignored by Git and the key was uploaded to GitHub Actions secrets as `TAPSEND_RELEASE_KEYSTORE_B64`, `TAPSEND_RELEASE_STORE_PASSWORD`, and `TAPSEND_RELEASE_KEY_PASSWORD`. **Back up both local files securely.** Losing the signing key means future APKs cannot update this release in place.

For a later release, increment the Android `versionCode`, update `versionName`, Rust package version, Windows installer version, CLI version, release filenames, and release notes together. Build and test all platforms before publishing the new tag.
