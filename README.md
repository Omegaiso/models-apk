# models-apk

Gestor nativo de aplicaciones Android para Linux: Android completo sobre Android Emulator/QEMU, una ventana Wayland por aplicación y sin mostrar el escritorio Android.

> Proyecto: **models-apk**. Los identificadores internos (`apkstart-*`, binario `apkstart`, rutas `~/.config/apkstart`) se mantienen por compatibilidad hasta un rename controlado.

El diseño y las decisiones están en [`docs/plan-inicial/`](docs/plan-inicial/). El avance real se registra en [`docs/bitacora/bitacora.md`](docs/bitacora/bitacora.md).

## Estado

MVP en construcción. El spike de viabilidad ya validó:

- Android 14 (API 34) x86_64 headless con KVM y GPU del host.
- Display virtual por app con scrcpy 5.x.
- Ventana Wayland con título propio y resize flexible.
- Audio Opus desde Android.
- Teclado Android dentro del display virtual (`--display-ime-policy=local`).

Pendiente: input/portapapeles manual, compositores Hyprland/GNOME, `FLAG_SECURE` y apps ARM-only.

## Requisitos del host

- Linux x86_64 con `/dev/kvm` accesible.
- Android SDK con `platform-tools`, `emulator` y una imagen API 34 `google_apis` x86_64.
- `scrcpy` 5.x (o ruta al binario y a `scrcpy-server`).
- Wayland recomendado; X11/XWayland es best-effort.
- Rust 1.95+ para compilar.

## Ajustes GTK

`apkstart-settings` es la UI de administración (GTK4/libadwaita):

- Inicio: estado del runtime, arrancar/detener, sincronizar.
- Aplicaciones: lista con icono, abrir/cerrar e instalar APK.
- Recursos: CPU/RAM agregados de `apkstartd`, emulador, scrcpy, ADB y la UI.
- Diagnóstico: `doctor`, logs en vivo cada 2 s y exportación de logs.
- Acerca de: versión y licencia.

En `btop`/monitor de sistema los procesos aparecen con sus nombres reales (`apkstartd`, `qemu-system-x86_64-headless`, `scrcpy`); la página **Recursos** los agrupa como models-apk. El servicio está en `models-apk.slice`, así que `systemd-cgls --user | grep -A4 models-apk.slice` o `systemd-cgtop -1 -b --depth=5 | grep models-apk` muestran la agrupación.

`apkstart setup` crea el acceso `io.github.apkstart.Settings.desktop` en el menú. La UI delega en el binario `apkstart`, que usa el daemon cuando está disponible.

## AVD persistente

El AVD debe conservar la partición de datos entre reinicios. Si lo creas con `avdmanager`, revisa `~/.android/avd/<nombre>.avd/config.ini` y **elimina** esta línea si existe:

```ini
disk.dataPartition.path=<temp>
```

Ajustes recomendados para el AVD de ApkStart:

```ini
hw.ramSize=3072
hw.cpu.ncore=4
hw.gpu.enabled=yes
hw.gpu.mode=host
disk.dataPartition.size=6G
userdata.useQcow2=yes
```

El primer arranque de un AVD nuevo es en frío. A partir de ahí se usa quickboot. Si un snapshot se corrompe, arranca con `APKSTART_COLD_BOOT=1` y, si persiste, borra `~/.android/avd/<nombre>.avd/snapshots/`.

`apkstart doctor` avisa si detecta una partición temporal.

## Compilar y probar

```bash
cargo build
cargo test
cargo run -p apkstart-cli -- doctor
```

## Instalación completa

```bash
cargo build --release --locked
sudo packaging/install.sh          # /usr/local
# o: PREFIX=/usr sudo packaging/install.sh
apkstart setup                     # MIME y daemon del usuario actual
```

`packaging/install.sh` registra **automáticamente** `binfmt_misc` para que `./app.apk` funcione; no se debe pedir al usuario el comando manual. Los paquetes de distro deben ejecutar el mismo hook en `postinst` (ver [`packaging/README.md`](packaging/README.md)).

## Uso rápido

```bash
# Estado del entorno y del runtime
cargo run -p apkstart-cli -- doctor
cargo run -p apkstart-cli -- runtime status

# Registrar la integración con el escritorio (una vez)
cargo run -p apkstart-cli -- setup

# Sincronizar apps instaladas dentro de Android (F-Droid, etc.)
cargo run -p apkstart-cli -- sync

# UI de administración
cargo run -p apkstart-settings

# Arrancar Android headless
cargo run -p apkstart-cli -- runtime start

# Instalar un APK y lanzarlo en su propia ventana
cargo run -p apkstart-cli -- install /ruta/app.apk
cargo run -p apkstart-cli -- install --launch /ruta/app.apk
cargo run -p apkstart-cli -- launch com.example.app
cargo run -p apkstart-cli -- stop com.example.app
```

`apkstart setup` instala y activa `apkstartd`, el daemon D-Bus `org.apkstart.Manager1`; `launch` y `stop` pasan por él cuando está disponible y caen al modo directo si no lo está.

## Escritorio, menú y doble clic

- `install` crea una entrada `.desktop` y un icono en `~/.local/share`; la app aparece en el menú y en el escritorio como una app más.
- Las apps se clasifican en categorías freedesktop (`Network;WebBrowser;`, `Game;`, `AudioVideo;`, `Office;`, `System;`…) usando `android:appCategory` y heurísticas de package/label.
- `setup` registra el handler MIME `application/vnd.android.package-archive`: al hacer doble clic en un APK descargado desde el navegador se instala y se abre. En el MVP se abre una terminal para ver el progreso.
- `setup` instala y activa `apkstartd`; el daemon sincroniza cada 15 s las apps instaladas **dentro** de Android (por ejemplo desde F-Droid) con el escritorio Linux. El timer antiguo se elimina cuando el daemon arranca; si no está disponible, queda el timer como respaldo. También puedes forzar `apkstart sync` o `apkstart sync --refresh`.
- Las apps con icono vectorial/adaptive sin bitmap usan un icono de respaldo con su inicial; el icono adaptativo completo queda pendiente.
- Para ejecutar un APK como un binario (`./app.apk`), Linux necesita `binfmt_misc` y permisos de ejecución. En una instalación completa, `packaging/install.sh` lo registra automáticamente. Solo para desarrollo sin instalar:

```bash
sudo packaging/binfmt/install.sh /ruta/a/target/debug/apkstart
chmod +x app.apk
./app.apk
```

El handler binfmt llama a `apkstart install --launch`; el APK se ejecuta con los permisos del usuario que lo lanza, no como root.

La configuración se lee de `~/.config/apkstart/config.toml` y se puede sobreescribir con variables `APKSTART_*`:

| Variable | Efecto |
|---|---|
| `APKSTART_SDK_ROOT` | Ruta del SDK de Android |
| `APKSTART_AVD` | Nombre del AVD |
| `APKSTART_PORT` | Puerto del emulador (por defecto 5556) |
| `APKSTART_ADB_PORT` | Servidor ADB propio (por defecto 5038) |
| `APKSTART_SCRCPY` / `APKSTART_SCRCPY_SERVER` | Rutas de scrcpy y su servidor |
| `APKSTART_COLD_BOOT=1` | Fuerza cold boot (recuperación) |
| `APKSTART_ALLOW_TCG=1` | Permite arrancar sin KVM (muy lento) |

ApkStart usa un servidor ADB propio en el puerto 5038 para no interferir con el ADB del usuario ni con dispositivos físicos.

## Estructura

```text
crates/
  apkstart-core/    configuración, errores, modelos, SQLite
  apkstart-apk/     lectura de APK (manifiesto, label, versión, activity)
  apkstart-engine/  emulador, ADB y sesiones scrcpy
  apkstart-cli/     binario apkstart
docs/
  plan-inicial/     plan técnico
  bitacora.md       decisiones y progreso
```

## Licencias

El código de models-apk es **MIT** (ver [`LICENSE`](LICENSE)). El Android Emulator, las imágenes de Google y la traducción ARM son propietarios: no se redistribuyen. El usuario instala el SDK y acepta sus licencias. `apk-info` es Apache-2.0; `scrcpy` es Apache-2.0; QEMU es GPL-2.0.
