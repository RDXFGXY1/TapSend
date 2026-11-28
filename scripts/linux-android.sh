#!/usr/bin/env bash

find_jdk() {
    local java_dir javac_path output
    jdk_home= jdk_version=
    javac_path=$(command -v javac || true)
    if [[ -n $javac_path ]]; then
        javac_path=$(dirname -- "$(dirname -- "$(readlink -f -- "$javac_path")")")
    fi
    for java_dir in "${JAVA_HOME:-}" "$javac_path" /usr/lib/jvm/*; do
        [[ -n $java_dir && -x "$java_dir/bin/java" && -x "$java_dir/bin/javac" ]] || continue
        output=$("$java_dir/bin/javac" -version 2>/dev/null) || continue
        [[ $output =~ ^javac[[:space:]]+([0-9]+)([.][0-9]+)* ]] || continue
        jdk_version=${output#javac }
        if (( BASH_REMATCH[1] >= 17 && BASH_REMATCH[1] <= 23 )); then
            jdk_home=$java_dir
            return
        fi
    done
}

find_sdkmanager() {
    local candidate
    sdkmanager=
    for candidate in "$sdk_root"/cmdline-tools/latest/bin/sdkmanager \
        "$sdk_root"/cmdline-tools/*/bin/sdkmanager "$sdk_root"/tools/bin/sdkmanager; do
        if [[ -x $candidate ]]; then
            sdkmanager=$candidate
            return
        fi
    done
}

check_android() {
    local detected
    if [[ $architecture != amd64 ]]; then
        printf '\nAndroid build is available on x86-64 Linux.\n'
        return
    fi
    sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
    find_jdk
    find_sdkmanager
    need_sdkmanager=0
    android_packages=()
    table_header 'Optional Android dependencies'
    if [[ -n $jdk_home ]]; then
        table_row 'Java JDK' 17–23 "$jdk_version" Ready
    elif [[ -n $jdk_version ]]; then
        table_row 'Java JDK' 17–23 "$jdk_version" 'Compatible JDK needed'
    else
        detected=$(java -version 2>&1 || true)
        if [[ $detected == *version* ]]; then
            table_row 'Java JDK' 17–23 'runtime only' 'JDK needed'
        else
            table_row 'Java JDK' 17–23 — Missing
        fi
    fi
    if [[ -f "$sdk_root/platforms/android-35/android.jar" ]]; then
        table_row 'Android platform' 35 35 Ready
    else
        table_row 'Android platform' 35 — Missing
        android_packages+=('platforms;android-35')
    fi
    if [[ -x "$sdk_root/build-tools/35.0.0/aapt2" ]]; then
        table_row 'Android build tools' 35.0.0 35.0.0 Ready
    else
        table_row 'Android build tools' 35.0.0 — Missing
        android_packages+=('build-tools;35.0.0')
    fi
    if [[ -n $sdkmanager ]]; then
        table_row sdkmanager 'for SDK setup' installed Ready
    elif (( ${#android_packages[@]} )); then
        table_row sdkmanager 'for SDK setup' — Missing
        need_sdkmanager=1
    else
        table_row sdkmanager optional — 'SDK already ready'
    fi
    if command -v unzip >/dev/null; then
        table_row unzip 'for SDK setup' installed Ready
    elif (( need_sdkmanager )); then
        table_row unzip 'for SDK setup' — Missing
    else
        table_row unzip optional — 'SDK tools available'
    fi
    table_row Gradle 8.13 'project wrapper' 'Downloaded by wrapper if needed'
}

prepare_android() {
    local id=15859902 archive destination
    if [[ -z $jdk_home ]] || (( ${#android_packages[@]} )); then
        confirm 'Install the Android dependencies marked above?' || exit 0
    fi
    if [[ -z $jdk_home ]]; then
        install_packages java
        find_jdk
        [[ -n $jdk_home ]] || die 'A Java 17–23 JDK is required. Set JAVA_HOME and try again.'
    fi
    export JAVA_HOME="$jdk_home" ANDROID_HOME="$sdk_root" ANDROID_SDK_ROOT="$sdk_root"
    export PATH="$JAVA_HOME/bin:$PATH"
    if (( need_sdkmanager )); then
        confirm 'Download the SDK command-line tools under the Android SDK license (developer.android.com/studio#terms-and-conditions)?' || exit 0
        command -v unzip >/dev/null || install_packages unzip
        archive="commandlinetools-linux-${id}_latest.zip"
        download "https://dl.google.com/android/repository/$archive" "$work/$archive"
        # From https://developer.android.com/studio#command-tools
        verify_sha256 4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583 "$work/$archive"
        unzip -q "$work/$archive" -d "$work/android-tools"
        mkdir -p "$sdk_root/cmdline-tools"
        destination="$sdk_root/cmdline-tools/tapsend-$id"
        [[ ! -e $destination ]] || die "SDK tools at $destination are incomplete. Repair that installation and retry."
        mv -T -- "$work/android-tools/cmdline-tools" "$destination"
        sdkmanager="$destination/bin/sdkmanager"
    fi
    if (( ${#android_packages[@]} )); then
        "$sdkmanager" --sdk_root="$sdk_root" --licenses <&3
        "$sdkmanager" --sdk_root="$sdk_root" "${android_packages[@]}" <&3
        check_android
        (( ${#android_packages[@]} == 0 )) || die 'Android SDK packages are still missing. Check SDK license answers and permissions.'
    fi
}

build_android() {
    local builds="${XDG_DATA_HOME:-$HOME/.local/share}/tapsend/builds" output
    make -C "$root" android 'ANDROID_GRADLE=bash ./gradlew'
    mkdir -p "$builds"
    output=$(mktemp -d "$builds/android.XXXXXXXX")
    apk="$output/TapSend-Android-debug.apk"
    install -m 600 "$root/bin/tapsend-android-debug.apk" "$apk"
    printf '\nBuilt APK: %s\n' "$apk"
}

serve_android() {
    local -a host=()
    if [[ -n ${TAPSEND_APK_HOST:-} ]]; then host=(-apk-host "$TAPSEND_APK_HOST"); fi
    printf '\n%s\n' 'Connect your phone and PC to the same network.' \
        'Scan the QR and download the APK. A browser may ask you to trust the local HTTPS connection.' \
        'Open the downloaded APK on Android. Allow installs from your browser if Android asks.'
    local result=0
    "$root/bin/tapsend-server" -share-apk "$apk" "${host[@]}" </dev/null || result=$?
    if (( result != 0 && result != 130 )); then return "$result"; fi
}
