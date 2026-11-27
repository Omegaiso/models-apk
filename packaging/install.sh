#!/usr/bin/env bash
# Instalación local/system-wide de models-apk.
# El paso de binfmt se ejecuta automáticamente; no debe pedirse al usuario.
#
# Uso:
#   cargo build --release --locked
#   sudo packaging/install.sh
#   PREFIX=/usr sudo packaging/install.sh
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PREFIX="${PREFIX:-/usr/local}"
APKSTART_BIN="${APKSTART_BIN:-$REPO_ROOT/target/release/apkstart}"

if [ "$(id -u)" -ne 0 ]; then
    echo "Se necesitan permisos de root para instalar y registrar binfmt_misc." >&2
    exec sudo -- "$0" "$@"
fi

if [ ! -x "$APKSTART_BIN" ]; then
    echo "No se encontró el binario: $APKSTART_BIN" >&2
    echo "Compila primero: cargo build --release --locked" >&2
    exit 1
fi

install -Dm755 "$APKSTART_BIN" "$PREFIX/bin/apkstart"
install -Dm644 "$REPO_ROOT/LICENSE" "$PREFIX/share/licenses/models-apk/LICENSE"
install -Dm755 "$SCRIPT_DIR/binfmt/install.sh" "$PREFIX/lib/models-apk/binfmt/install.sh"
install -Dm644 "$SCRIPT_DIR/binfmt/models-apk.conf" "$PREFIX/lib/models-apk/binfmt/models-apk.conf"
install -Dm755 "$SCRIPT_DIR/binfmt/uninstall.sh" "$PREFIX/lib/models-apk/binfmt/uninstall.sh"

tmp_desktop="$(mktemp)"
trap 'rm -f "$tmp_desktop"' EXIT
sed "s|@APKSTART_BIN@|$PREFIX/bin/apkstart|g" \
    "$SCRIPT_DIR/desktop/io.github.apkstart.install.desktop.in" > "$tmp_desktop"
install -Dm644 "$tmp_desktop" "$PREFIX/share/applications/io.github.apkstart.install.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi

# Obligatorio: registra ./app.apk automáticamente en la instalación.
"$PREFIX/lib/models-apk/binfmt/install.sh" "$PREFIX/bin/apkstart"

echo
echo "models-apk instalado en $PREFIX."
echo "Binfmt para ./app.apk registrado automáticamente."
echo "Cada usuario puede ejecutar una vez: apkstart setup"
echo "  (handler MIME por usuario, timer de sincronización y cachés)."
