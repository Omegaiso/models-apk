# Estructura del repositorio

## 1. Árbol

```text
models-apk/
├── Cargo.toml                 # workspace, edición 2024, deps y perfil release
├── Cargo.lock
├── LICENSE                    # MIT
├── README.md                  # presentación y guía rápida
├── AGENTS.md                  # contexto operativo para agentes/desarrollo
├── crates/
│   ├── apkstart-core/         # Config, Db, modelos, .desktop, iconos, errores
│   ├── apkstart-apk/          # parser APK (apk-info)
│   ├── apkstart-engine/       # emulador, ADB y sesiones scrcpy
│   ├── apkstart-app/          # install/registro/sync compartido
│   ├── apkstart-daemon/       # apkstartd + Manager + D-Bus
│   ├── apkstart-cli/          # binario apkstart + cliente D-Bus
│   └── apkstart-settings/     # UI GTK4/libadwaita
├── docs/
│   ├── README.md              # índice técnico
│   ├── arquitectura.md
│   ├── estructura.md
│   ├── dependencias.md
│   ├── instalacion.md
│   ├── bugs.md
│   ├── actualizaciones.md
│   ├── desarrollo.md
│   ├── images/                # capturas usadas en el README
│   ├── design/settings-ui/    # laboratorio HTML de las 3 variantes
│   ├── plan-inicial/          # plan técnico original
│   └── bitacora/bitacora.md   # historial append-only
└── packaging/
    ├── install.sh             # instalación local/system-wide + binfmt
    ├── README.md              # reglas para paquetes de distro
    ├── binfmt/                # registro de ./app.apk
    ├── dbus/                  # activación D-Bus de sesión
    ├── desktop/               # .desktop del handler y de ajustes
    └── systemd/               # apkstartd.service (usuario)
```

## 2. Archivos clave por crate

### `apkstart-core`

| Archivo | Contenido |
|---|---|
| `src/config.rs` | `Config`, defaults, TOML, variables `APKSTART_*`, rutas XDG, expansión de `~`, `PATH` de herramientas. |
| `src/db.rs` | Apertura SQLite, WAL, migraciones `user_version`, CRUD de apps/ventanas/settings. |
| `src/model.rs` | `App`, `RuntimeState`, `RuntimeStatus`, `WindowState`, `sync`/`launch` timestamps. |
| `src/desktop.rs` | Generación de `.desktop`, categorías freedesktop, iconos hicolor y fallback PNG. |
| `src/error.rs` | `Error` de core con `thiserror`. |

### `apkstart-apk`

- `src/lib.rs`: `read_apk`, `read_icon`, `read_resource`; clasificación de
  categorías y extracción de metadata con `apk-info`.
- `examples/inspect.rs` y `examples/icon.rs`: utilidades de inspección manual.

### `apkstart-engine`

| Archivo | Contenido |
|---|---|
| `src/emulator.rs` | `AndroidEmulator`: `checks`, `start`, `stop`, `status`, `ensure_running`, `is_online`; flags de QEMU, quickboot y cold boot. |
| `src/adb.rs` | Cliente ADB con timeout, serial propio, `install`, `shell`, `list_packages`, `pull`. |
| `src/scrcpy.rs` | `ScrcpyOptions`, `ScrcpySession`: spawn, logs, PID, `wait`, `kill`; argumentos de display y `SDL_APP_ID`. |
| `src/error.rs` | Errores del motor. |

### `apkstart-app`

- `src/lib.rs`: `install_apk`, `register_app_from_apk`, `sync_apps`,
  `SyncReport`, locks de install/sync. Es la lógica compartida por CLI y daemon.

### `apkstart-daemon`

| Archivo | Contenido |
|---|---|
| `src/main.rs` | Arranque tokio, conexión D-Bus de sesión, bucle de sync cada 15 s y limpieza de sesiones cada 5 s. |
| `src/manager.rs` | `Manager`: runtime, apps, install, sync, launch/stop, mapa de sesiones y PID liveness. |
| `src/dbus.rs` | Interfaz `org.apkstart.Manager1` y serialización JSON. |

### `apkstart-cli`

| Archivo | Contenido |
|---|---|
| `src/main.rs` | Comandos `clap`: doctor, setup, sync, runtime, install, launch, list, uninstall, stop, settings, export-log. |
| `src/daemon.rs` | Cliente D-Bus con fallback al modo directo si `apkstartd` no está. |

### `apkstart-settings`

| Archivo | Contenido |
|---|---|
| `src/main.rs` | Aplicación Adw, páginas, hilos de datos (`std::sync::mpsc`), acciones que ejecutan `apkstart`. |
| `src/ui.rs` | Sistema visual: CSS, `page`, `header`, `card`, `metric`, `pill`, `flow`, `brand_mark`. |

La UI es **responsive**: `AdwBreakpointBin` colapsa la sidebar a overlay por
debajo de 800 px, `AdwClamp` limita el contenido a 1120 px y `GtkFlowBox`
reordena las tarjetas de 3 a 2 a 1 columna. Requiere libadwaita con features
`v1_4` y `gtk_v4_10`.

## 3. Superficie del CLI

```text
apkstart doctor          # KVM, SDK, emulador, AVD, scrcpy, Wayland, D-Bus
apkstart setup           # integración de escritorio, MIME, daemon
apkstart sync [--refresh] [--if-running] [--quiet]
apkstart runtime start|stop|status
apkstart install [--launch] <apk>
apkstart launch [--title] [--size] [--dpi] <package>
apkstart list [--json]
apkstart uninstall <package>
apkstart stop <package>
apkstart settings
apkstart export-log
```

El CLI usa el daemon si está disponible. `launch --size/--dpi` es la vía para
probar otros perfiles de display sin tocar la configuración global.

## 4. Esquema SQLite

`PRAGMA user_version = 2`. Motor `rusqlite` con `bundled` y WAL.

### `apps`

| Columna | Tipo | Notas |
|---|---|---|
| `package` | TEXT PK | `org.fdroid.fdroid` |
| `label` | TEXT | nombre visible |
| `version_name` | TEXT | puede ser NULL |
| `version_code` | INTEGER | default 0 |
| `activity` | TEXT | primera activity de launcher |
| `icon_path` | TEXT | PNG en XDG data |
| `apk_path` | TEXT | APK registrado |
| `min_sdk` / `target_sdk` | INTEGER | |
| `abis` | TEXT | JSON array |
| `categories` | TEXT | JSON array freedesktop (v2) |
| `orientation` | TEXT | |
| `installed_at` | INTEGER | epoch |
| `last_launched_at` | INTEGER | epoch o NULL |
| `enabled` | INTEGER | 0/1 |

### `window_state`

`package PK, width, height, maximized, fullscreen, output, scale, updated_at`.
Se usa para persistir tamaño/estado; la integración con el visor propio aún es
parcial.

### `settings`

`key TEXT PK, value TEXT` para preferencias sueltas.

> **v3 pendiente:** tabla `sessions` (package, pid, display, timestamps) para
> adoptar sesiones scrcpy tras reiniciar el daemon.

## 5. Rutas y variables

| Variable | Default | Efecto |
|---|---|---|
| `ANDROID_SDK_ROOT` / `ANDROID_HOME` | `~/Android/Sdk` | raíz del SDK |
| `APKSTART_SDK_ROOT` | — | sobreescribe el SDK |
| `APKSTART_AVD` | `apkstart` | nombre del AVD |
| `APKSTART_PORT` | `5556` | puerto del emulador |
| `APKSTART_ADB_PORT` | `5038` | servidor ADB propio |
| `APKSTART_RAM_MB` | `3072` | RAM del guest |
| `APKSTART_CORES` | `4` | vCPU del guest |
| `APKSTART_GPU` | `host` | modo GPU (`host`, `swiftshader_indirect`, …) |
| `APKSTART_SCRCPY` | autodetectado | binario scrcpy |
| `APKSTART_SCRCPY_SERVER` | autodetectado | `scrcpy-server` |
| `APKSTART_CONFIG` | `~/.config/apkstart/config.toml` | archivo TOML alternativo |
| `APKSTART_ALLOW_TCG` | `false` | permite arrancar sin KVM (muy lento) |
| `APKSTART_COLD_BOOT` | `false` | fuerza arranque en frío |
| `APKSTART_PAGE` | `home` | **solo QA**: página inicial de la UI (`home`, `apps`, `resources`, `network`, `peripherals`, `integration`, `diagnostics`, `about`) |

Rutas:

```text
~/.config/apkstart/config.toml
~/.local/share/apkstart/apkstart.db
~/.local/share/apkstart/cache/
~/.local/share/applications/
~/.local/share/icons/hicolor/
~/.local/state/apkstart/emulator.log
~/.local/state/apkstart/sessions/<package>.log
```

## 6. Empaquetado

`packaging/install.sh` instala los tres binarios en `$PREFIX/bin`, la licencia,
las unidades de systemd/D-Bus, los `.desktop` y ejecuta el hook de `binfmt_misc`.
Está pensado para que los paquetes de distro repliquen exactamente los mismos
pasos; ver [`packaging/README.md`](../packaging/README.md).

Los instaladores `.deb`, `.rpm`, Arch y el script `install.sh` de la raíz son
Fase 2; el diseño está en [actualizaciones.md](actualizaciones.md).
