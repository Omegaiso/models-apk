#!/usr/bin/env bash
# Desregistra binfmt_misc al desinstalar models-apk.
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
    echo "Se necesitan permisos de root para desregistrar binfmt_misc." >&2
    exec sudo -- "$0" "$@"
fi

rm -f /etc/binfmt.d/models-apk.conf
rm -f /usr/local/bin/apkstart-run

if command -v systemctl >/dev/null 2>&1; then
    systemctl restart systemd-binfmt.service || true
fi

echo "binfmt_misc de models-apk desregistrado."
