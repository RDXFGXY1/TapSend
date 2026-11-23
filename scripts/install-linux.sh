#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_bin="${TAPSEND_INSTALL_BIN:-$HOME/.local/bin}"
applications_dir="${TAPSEND_APPLICATIONS_DIR:-$HOME/.local/share/applications}"

for name in tapsend tabsend tapsend-tui tapsend-server; do
    if [[ ! -f "$project_root/bin/$name" ]]; then
        printf 'Missing %s. Run make build first.\n' "$project_root/bin/$name" >&2
        exit 1
    fi
done

mkdir -p "$install_bin" "$applications_dir"
install -m 755 "$project_root/bin/tapsend" "$install_bin/tapsend"
install -m 755 "$project_root/bin/tabsend" "$install_bin/tabsend"
install -m 755 "$project_root/bin/tapsend-tui" "$install_bin/tapsend-tui"
install -m 755 "$project_root/bin/tapsend-server" "$install_bin/tapsend-server"

cat > "$applications_dir/tapsend.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TapSend
Comment=Share files with phones on the local network
Exec="$install_bin/tapsend"
Terminal=true
Icon=folder-send
Categories=Network;FileTransfer;
StartupNotify=false
EOF

printf 'Installed TapSend (tapsend and tabsend) in %s and launcher in %s\n' "$install_bin" "$applications_dir"
if [[ ":$PATH:" != *":$install_bin:"* ]]; then
    printf 'Add %s to PATH to run tabsend from any terminal.\n' "$install_bin"
fi
