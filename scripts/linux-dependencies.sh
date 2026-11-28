#!/usr/bin/env bash

GO_INSTALL_VERSION=1.26.4

die() {
    printf 'TapSend: %s\n' "$*" >&2
    exit 1
}

confirm() {
    local answer
    printf '\n%s [y/N] ' "$1" >&2
    IFS= read -r -u 3 answer && [[ $answer == [yY] || $answer == [yY][eE][sS] ]]
}

table_header() {
    printf '\n%s\n' "$1"
    table_row Dependency Required Detected Status
    table_row ---------- -------- -------- ------
}

table_row() { printf '  %-21s %-14s %-16s %s\n' "$@"; }

tool_version() {
    local output
    output=$("$@" 2>/dev/null) || return 0
    if [[ $output =~ (^|[^0-9])([0-9]+\.[0-9]+(\.[0-9]+)?) ]]; then
        printf '%s' "${BASH_REMATCH[2]}"
    fi
}

version_at_least() {
    local major minor
    [[ $1 =~ ^([0-9]+)\.([0-9]+) ]] || return 1
    major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]}
    (( 10#$major > $2 || (10#$major == $2 && 10#$minor >= $3) ))
}

version_row() {
    local name=$1 detected=$2 major=$3 minor=$4
    if version_at_least "$detected" "$major" "$minor"; then
        table_row "$name" "$major.$minor+" "$detected" Ready
    elif [[ -n $detected ]]; then
        table_row "$name" "$major.$minor+" "$detected" 'Upgrade needed'
        return 1
    else
        table_row "$name" "$major.$minor+" — Missing
        return 1
    fi
}

check_desktop() {
    local command rust cargo rust_ready=0 cargo_ready=0
    system_tools=()
    need_go=0
    need_rust=0
    table_header 'Desktop dependencies'
    for command in curl tar make cc sha256sum install; do
        if command -v "$command" >/dev/null; then
            table_row "$command" system installed Ready
        else
            table_row "$command" system — Missing
            system_tools+=("$command")
        fi
    done
    version_row Go "$(tool_version env GOTOOLCHAIN=local go version)" 1 24 || need_go=1
    rust=$(tool_version rustc --version)
    cargo=$(tool_version cargo --version)
    version_row Rust "$rust" 1 88 || rust_ready=1
    version_row Cargo "$cargo" 1 88 || cargo_ready=1
    (( rust_ready || cargo_ready )) && need_rust=1
    return 0
}

install_packages() {
    local name
    local -a packages=() admin=()
    if (( EUID != 0 )); then
        command -v sudo >/dev/null || die 'Install the missing system packages as an administrator, then run setup again.'
        admin=(sudo)
    fi
    if command -v apt-get >/dev/null; then
        for name; do
            case "$name" in
                cc) packages+=(build-essential) ;;
                java) packages+=(openjdk-17-jdk-headless) ;;
                sha256sum|install) packages+=(coreutils) ;;
                *) packages+=("$name") ;;
            esac
        done
        "${admin[@]}" apt-get update <&3
        "${admin[@]}" apt-get install -y "${packages[@]}" <&3
    elif command -v dnf >/dev/null; then
        for name; do
            case "$name" in
                cc) packages+=(gcc glibc-devel) ;;
                java) packages+=(java-17-openjdk-devel) ;;
                sha256sum|install) packages+=(coreutils) ;;
                *) packages+=("$name") ;;
            esac
        done
        "${admin[@]}" dnf install -y "${packages[@]}" <&3
    elif command -v pacman >/dev/null; then
        for name; do
            case "$name" in
                cc) packages+=(gcc glibc) ;;
                java) packages+=(jdk17-openjdk) ;;
                sha256sum|install) packages+=(coreutils) ;;
                *) packages+=("$name") ;;
            esac
        done
        "${admin[@]}" pacman -S --needed --noconfirm "${packages[@]}" <&3
    else
        die "Install these tools with your distribution's package manager and retry: $*"
    fi
}

download() {
    curl --proto '=https' --fail --location --show-error --retry 2 --output "$2" "$1"
}

verify_sha256() {
    printf '%s  %s\n' "$1" "$2" | sha256sum --check --status ||
        die "Checksum verification failed for ${2##*/}."
}

install_desktop_dependencies() {
    local archive sha destination stage
    if (( ${#system_tools[@]} )); then install_packages "${system_tools[@]}"; fi
    if (( need_go )); then
        archive="go$GO_INSTALL_VERSION.linux-$architecture.tar.gz"
        # From https://go.dev/dl/
        case "$architecture" in
            amd64) sha=1153d3d50e0ac764b447adfe05c2bcf08e889d42a02e0fe0259bd47f6733ad7f ;;
            arm64) sha=ef758ae7c6cf9267c9c0ef080b8965f453d89ab2d25d9eb22de4405925238768 ;;
        esac
        download "https://go.dev/dl/$archive" "$work/$archive"
        verify_sha256 "$sha" "$work/$archive"
        mkdir -p "$toolchains"
        destination="$toolchains/go$GO_INSTALL_VERSION"
        [[ ! -e $destination ]] || die "Go already exists at $destination but is unusable."
        stage=$(mktemp -d "$toolchains/.go.XXXXXXXX")
        if ! tar -xzf "$work/$archive" --strip-components=1 -C "$stage" ||
            ! mv -T -- "$stage" "$destination"; then
            rm -rf -- "$stage"
            die 'Could not install Go.'
        fi
        export PATH="$destination/bin:$PATH"
    fi
    if (( need_rust )); then
        if ! command -v rustup >/dev/null; then
            download https://sh.rustup.rs "$work/rustup-init.sh"
            bash "$work/rustup-init.sh" -y --profile minimal --default-toolchain none --no-modify-path
        fi
        rustup toolchain install stable --profile minimal
        export RUSTUP_TOOLCHAIN=stable
    fi
    hash -r
}
