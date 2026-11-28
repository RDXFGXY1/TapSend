#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf '%s\n' 'Usage: bash install.sh [--check]' \
        'Install the Linux console app, with an optional Android build.' \
        'Use --check to list dependencies without installing or building.'
}

case "${1:-}" in
    --help|-h) usage; exit 0 ;;
    --check|'') ;;
    *) usage >&2; exit 2 ;;
esac
if (( $# > 1 )); then usage >&2; exit 2; fi
if [[ $(uname -s) != Linux ]]; then
    printf '%s\n' 'Run this installer on Linux.' >&2
    exit 1
fi
case "$(uname -m)" in
    x86_64) architecture=amd64 ;;
    aarch64|arm64) architecture=arm64 ;;
    *) printf '%s\n' 'The installer supports x86-64 and ARM64 Linux.' >&2; exit 1 ;;
esac

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
root=
if [[ -n ${BASH_SOURCE[0]:-} ]]; then
    root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fi
if [[ -z $root || ! -f "$root/scripts/linux-dependencies.sh" || ! -f "$root/Makefile" ]]; then
    command -v curl >/dev/null || { printf '%s\n' 'Install curl to fetch TapSend.' >&2; exit 1; }
    command -v tar >/dev/null || { printf '%s\n' 'Install tar to unpack TapSend.' >&2; exit 1; }
    root="$work/source"
    mkdir "$root"
    printf '%s\n' 'Fetching the TapSend source from GitHub.'
    curl --proto '=https' --fail --silent --show-error --location --retry 2 \
        --output "$work/source.tar.gz" https://codeload.github.com/RDXFGXY1/TapSend/tar.gz/refs/heads/tap-send
    tar -xzf "$work/source.tar.gz" --strip-components=1 -C "$root"
fi

source "$root/scripts/linux-dependencies.sh"
source "$root/scripts/linux-android.sh"

toolchains="${XDG_DATA_HOME:-$HOME/.local/share}/tapsend/toolchains"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
if [[ -x "$toolchains/go$GO_INSTALL_VERSION/bin/go" ]] &&
    ! version_at_least "$(tool_version env GOTOOLCHAIN=local go version)" 1 24; then
    export PATH="$toolchains/go$GO_INSTALL_VERSION/bin:$PATH"
fi

printf '\n╭─ TapSend / Linux setup\n'
check_desktop
check_android
[[ ${1:-} != --check ]] || exit 0

# curl | bash uses stdin for the script, so take answers from the terminal.
if ! { exec 3</dev/tty; } 2>/dev/null; then
    die 'An interactive terminal is required. Use --check for a dependency report.'
fi

if (( ${#system_tools[@]} || need_go || need_rust )); then
    confirm 'Install or upgrade the desktop dependencies marked above?' || exit 0
    install_desktop_dependencies
    check_desktop
    if (( ${#system_tools[@]} || need_go || need_rust )); then
        die 'Some desktop dependencies are still unavailable. See the table above.'
    fi
fi

confirm 'Build and install TapSend for your user?' || exit 0
make -C "$root" install-linux
if [[ $architecture == amd64 ]] && confirm 'Build the Android APK too?'; then
    prepare_android
    build_android
    serve_android
fi
