# ApkStart Linux

Gestor nativo de aplicaciones Android para Linux: Android completo sobre Android Emulator/QEMU, una ventana Wayland por aplicación y sin mostrar el escritorio Android.

El diseño y las decisiones están en [`docs/plan-inicial/`](docs/plan-inicial/). El avance real se registra en [`docs/bitacora.md`](docs/bitacora.md).

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

## Uso rápido

```bash
# Estado del entorno y del runtime
cargo run -p apkstart-cli -- doctor
cargo run -p apkstart-cli -- runtime status

# Arrancar Android headless
cargo run -p apkstart-cli -- runtime start

# Instalar un APK y lanzarlo en su propia ventana
cargo run -p apkstart-cli -- install /ruta/app.apk
cargo run -p apkstart-cli -- launch com.example.app
```

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

El código de ApkStart es MIT/Apache-2.0. El Android Emulator, las imágenes de Google y la traducción ARM son propietarios: no se redistribuyen. El usuario instala el SDK y acepta sus licencias. `apk-info` es Apache-2.0; `scrcpy` es Apache-2.0; QEMU es GPL-2.0.
