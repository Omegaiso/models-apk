#!/usr/bin/env bash
# Registra el handler binfmt_misc para ejecutar APKs como binarios.
# Uso:
#   sudo packaging/binfmt/install.sh /ruta/al/binario/apkstart
#   o bien: sudo packaging/binfmt/install.sh   (si apkstart está en PATH)
#
# Después:
#   chmod +x MiApp.apk
#   ./MiApp.apk
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if [ "$(id -u)" -ne 0 ]; then
    echo "Se necesitan permisos de root para registrar binfmt_misc." >&2
    exec sudo -- "$0" "$@"
fi

APKSTART_BIN="${1:-${APKSTART_BIN:-$(command -v apkstart || true)}}"
if [ -z "$APKSTART_BIN" ] || [ ! -x "$APKSTART_BIN" ]; then
    echo "Uso: sudo $0 /ruta/al/binario/apkstart" >&2
    echo "El binario debe ser ejecutable." >&2
    exit 1
fi
APKSTART_BIN="$(readlink -f "$APKSTART_BIN")"

install -d -m 0755 /usr/local/bin
cat > /usr/local/bin/apkstart-run <<EOF
#!/bin/sh
exec "$APKSTART_BIN" install --launch "\$@"
EOF
chmod 0755 /usr/local/bin/apkstart-run

install -D -m 0644 "$SCRIPT_DIR/models-apk.conf" /etc/binfmt.d/models-apk.conf

if command -v systemctl >/dev/null 2>&1; then
    systemctl restart systemd-binfmt.service
else
    echo "systemctl no encontrado; registra manualmente /etc/binfmt.d/models-apk.conf" >&2
    exit 1
fi

if [ -e /proc/sys/fs/binfmt_misc/models-apk ]; then
    echo "binfmt_misc registrado: /proc/sys/fs/binfmt_misc/models-apk"
else
    echo "aviso: no se encontró la entrada binfmt; revisa systemd-binfmt.service" >&2
fi

echo
echo "Listo. Prueba:"
echo "  chmod +x /ruta/app.apk"
echo "  /ruta/app.apk"
echo
echo "El wrapper usa: $APKSTART_BIN"
