# Desarrollo

## 1. Entorno

```bash
rustc --version        # 1.95+
pkg-config --modversion gtk4 libadwaita-1
```

Paquetes de compilación:

```bash
# Arch
sudo pacman -S --needed base-devel rust gtk4 libadwaita

# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel
```

## 2. Comandos del proyecto

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

No hay CI todavía (Fase 2). Esos tres comandos son el estándar antes de cerrar
cualquier cambio.

Ejecutar:

```bash
cargo run -p apkstart-cli -- doctor
cargo run -p apkstart-cli -- runtime start
cargo run -p apkstart-cli -- install --launch /ruta/app.apk
cargo run -p apkstart-daemon          # apkstartd en primer plano
cargo run -p apkstart-settings        # UI GTK4
```

El binario es `apkstart` (paquete `apkstart-cli`). `cargo build` deja
`target/debug/apkstart`, `apkstartd` y `apkstart-settings`.

## 3. Dónde tocar cada cosa

| Cambio | Archivo(s) |
|---|---|
| Configuración, defaults, env | `crates/apkstart-core/src/config.rs` |
| Esquema/consultas SQLite | `crates/apkstart-core/src/db.rs` |
| `.desktop`, iconos, categorías | `crates/apkstart-core/src/desktop.rs` |
| Parseo de APK | `crates/apkstart-apk/src/lib.rs` |
| Emulador/ADB/scrcpy | `crates/apkstart-engine/src/` |
| install/sync compartido | `crates/apkstart-app/src/lib.rs` |
| Daemon, D-Bus, sesiones | `crates/apkstart-daemon/src/` |
| CLI y fallback | `crates/apkstart-cli/src/` |
| UI y diseño | `crates/apkstart-settings/src/main.rs`, `ui.rs` |
| Empaquetado | `packaging/` |

## 4. Reglas que no se deben romper

1. **Densidad:** el display por defecto es `552×984/215` (411 dp). Si cambias el
   tamaño o el dpi, recalcula `dpi = round(ancho_px × 160 / ancho_dp)` y actualiza
   la documentación. No pongas `--window-x/y`: KWin los ignora en Wayland.
2. **Identidad de ventana:** mantén `SDL_APP_ID=io.github.apkstart.<package>`;
   sin él todas las ventanas comparten la clase `scrcpy`.
3. **Persistencia del AVD:** no introduzcas `-no-snapshot-load` en el arranque
   normal ni `disk.dataPartition.path=<temp>`; los APK desaparecen.
4. **SQLite:** cualquier cambio de columnas requiere migración y subir
   `PRAGMA user_version`. No borres la base del usuario.
5. **ADB:** usa el puerto `5038` del proyecto; no toques el servidor del usuario.
6. **Binfmt:** cualquier instalación debe registrar `./app.apk`; no pidas al
   usuario un comando manual.
7. **Licencias:** no commitees APK, imágenes de Android, emulador ni binarios de
   Google. `*.apk` está ignorado.
8. **Bitácora:** es append-only. Añade una entrada al cerrar un bloque; no
   reescribas el historial.

## 5. Tests

- Unitarios: `cargo test --workspace` (39 tests en el último corte).
- Test de APK real (se salta si no existe el archivo):

```bash
APKSTART_TEST_APK=/ruta/app.apk cargo test -p apkstart-apk parses_real_apk_when_available
```

- Un test concreto:

```bash
cargo test -p apkstart-core db::tests::upsert_and_read_app
cargo test -p apkstart-engine
```

Los tests **no** deben arrancar el emulador ni escribir en `~/.local/share`.
La prueba funcional con efectos reales es manual y requiere KVM, SDK, AVD y
scrcpy:

```bash
target/debug/apkstart doctor
target/debug/apkstart runtime start
target/debug/apkstart install --launch /ruta/app.apk
target/debug/apkstart launch <package>
target/debug/apkstart stop <package>
target/debug/apkstart runtime stop
```

Para ADB manual usa el puerto propio:

```bash
ANDROID_ADB_SERVER_PORT=5038 adb devices
ANDROID_ADB_SERVER_PORT=5038 adb -s emulator-5556 shell dumpsys display
```

## 6. UI de ajustes y capturas

La UI es responsive:

- `AdwBreakpointBin` colapsa el `AdwOverlaySplitView` por debajo de **800 px**.
- `AdwClamp` limita el contenido a **1120 px**.
- `GtkFlowBox` homogéneo reordena las tarjetas (3 → 2 → 1 columna).
- Los labels de tarjetas tienen `max_width_chars` para que el flujo no se
  degrade con textos largos.

### Página inicial para QA

```bash
APKSTART_PAGE=apps cargo run -p apkstart-settings
```

Valores: `home`, `apps`, `resources`, `network`, `peripherals`, `integration`,
`diagnostics`, `about`. Es una variable **solo de desarrollo**; sin ella la UI
abre en Inicio.

### Periféricos

```bash
apkstart peripherals                       # estado y configuración
apkstart peripherals --json                # consumo de la UI
apkstart peripherals --camera-back webcam0
apkstart runtime restart
```

La cámara se puede validar abriendo la app Camera del sistema en una sesión:

```bash
target/debug/apkstart launch com.android.camera2
```

El micrófono usa `-allow-host-audio` y `QEMU_AUDIO_IN_DRV=alsa`. Para depurar el
backend, revisa `~/.local/state/apkstart/emulator.log` y
`pactl info`/`pactl list sources`. El gamepad se comprueba con `--gamepad=uhid`
en los argumentos de scrcpy y un mando visible en `/dev/input`.

### Capturas para documentación

Procedimiento usado para `docs/images/` (KDE Wayland):

```bash
# 1. Abrir la página deseada a 1120x780
APKSTART_PAGE=home target/debug/apkstart-settings &
# 2. Capturar la ventana activa (incluye sombra)
spectacle -b -n -a -o /tmp/captura.png
# 3. Recortar el contenido (la captura activa añade ~25 px de sombra)
magick /tmp/captura.png -crop 1120x780+25+25 +repage -strip docs/images/inicio.png
```

Para una app real: `apkstart launch <package>`, capturar la ventana y recortar
`552×1012` (incluye la barra de título).

### Laboratorio de diseño

`docs/design/settings-ui/index.html` contiene 9 páginas × 3 variantes (A/B/C),
tema claro/oscuro y calculadora dp/dpi. Se abre con:

```bash
xdg-open docs/design/settings-ui/index.html
```

La variante **B · Centro de control** es la implementada en GTK.

## 7. Depuración

| Qué | Dónde |
|---|---|
| Emulador | `~/.local/state/apkstart/emulator.log` |
| Sesión de una app | `~/.local/state/apkstart/sessions/<package>.log` |
| Daemon | `journalctl --user -u apkstartd -f` |
| UI de ajustes | ejecutar desde terminal para ver stderr |
| D-Bus | `busctl --user call org.apkstart.Manager1 /org/apkstart/Manager1 org.apkstart.Manager1 GetVersion` |
| Estado Android | `ANDROID_ADB_SERVER_PORT=5038 adb -s emulator-5556 shell dumpsys display` |
| Recursos | `systemd-cgls --user`, `systemd-cgtop -1 -b --depth=5`, `btop` |

El daemon tiene un bucle de sync cada 15 s y un limpiador de sesiones cada 5 s.
Si una ventana se cierra a mano, el daemon la retira al comprobar
`/proc/<pid>/cmdline`.

## 8. Build de release

```bash
cargo build --release --locked
sudo packaging/install.sh
```

El perfil release usa `lto = "thin"`, `codegen-units = 1` y `strip = "debuginfo"`.
La instalación local requiere los tres binarios release; `packaging/install.sh`
falla con un mensaje claro si faltan.

## 9. Convenciones

- Código e identificadores en inglés; documentación y mensajes de usuario en
  español.
- Commits en español, imperativos y con alcance: `Daemon: ...`, `UI: ...`.
- No mezclar refactors masivos con correcciones funcionales.
- Al corregir un bug relevante: entrada en `docs/bugs.md` y en la bitácora.
- Al cambiar arquitectura o empaquetado: actualizar `AGENTS.md` y los documentos
  de `docs/`.
