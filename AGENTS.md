# AGENTS.md — models-apk (ApkStart Linux)

## Contexto que no se deduce de los nombres

- Proyecto/repo: **models-apk** (`github.com/Omegaiso/models-apk`). Los identificadores internos siguen siendo `apkstart-*` y el binario `apkstart`; no hagas un rename masivo sin migrar configuración, SQLite, `.desktop` y `app_id`.
- El repo implementa hoy **siete crates**: `apkstart-core`, `apkstart-apk`, `apkstart-engine`, `apkstart-app`, `apkstart-daemon` (`apkstartd`), `apkstart-cli` y `apkstart-settings` (GTK4/libadwaita).
- `docs/plan-inicial/` describe el diseño completo. **Aún no existen** `apkstart-viewer` propio ni `apkstart-integration`.
- `apkstartd` expone D-Bus (`org.apkstart.Manager1`): runtime status/start/stop, listar, instalar, sincronizar, lanzar y cerrar app. `install`, `sync`, `launch` y `stop` del CLI lo usan si está disponible y caen al modo directo si no. El daemon tiene un bucle de sync cada 15 s; el timer antiguo se retira al activarlo. Aún no persiste sesiones/adopta scrcpy.
- Motor fijado para el MVP: **Android Emulator oficial + API 34 x86_64 `google_apis`**. Prioridad: estabilidad y usabilidad. No migrar a QEMU puro/Waydroid ni introducir Electron/Node/Java en el producto host (el SDK/`sdkmanager` puede usar Java como herramienta).
- La bitácora vigente es **`docs/bitacora/bitacora.md`**. Es append-only: al cerrar un bloque añade una entrada; no reescribas el historial.

## Comandos

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# El paquete es apkstart-cli; el binario es apkstart
cargo run -p apkstart-cli -- doctor
cargo run -p apkstart-cli -- setup
cargo run -p apkstart-cli -- sync
cargo run -p apkstart-daemon            # apkstartd en primer plano
cargo run -p apkstart-settings          # UI GTK4/libadwaita
target/debug/apkstart doctor
busctl --user call org.apkstart.Manager1 /org/apkstart/Manager1 org.apkstart.Manager1 GetVersion

# Un test / un paquete
cargo test -p apkstart-core db::tests::upsert_and_read_app
cargo test -p apkstart-engine

# El test de APK real se salta si falta /tmp/opencode/F-Droid.apk
APKSTART_TEST_APK=/ruta/app.apk cargo test -p apkstart-apk parses_real_apk_when_available
```

No hay CI ni task runner: esas tres verificaciones (`fmt`, `clippy`, `test`) son el estándar antes de cerrar un cambio.

## Prueba funcional (manual, con efectos reales)

Requiere KVM, SDK, AVD `apkstart` y scrcpy. `install` arranca el emulador si hace falta, escribe en `~/.local/share`, crea `.desktop`/icono y toca SQLite; `setup` además escribe en `~/.local/share/applications` y `~/.config/systemd/user`. No los ejecutes como parte de tests.

```bash
target/debug/apkstart doctor
target/debug/apkstart runtime start
target/debug/apkstart install --launch /ruta/app.apk
target/debug/apkstart launch <package>
target/debug/apkstart runtime stop
```

- Para inspeccionar ADB usa el **puerto propio**: `ANDROID_ADB_SERVER_PORT=5038 adb devices`, no el 5037 del usuario.
- Logs: `~/.local/state/apkstart/emulator.log` y `~/.local/state/apkstart/sessions/<package>.log`.
- `launch` deja scrcpy vivo al salir el CLI; si `apkstartd` está activo, la sesión la posee el daemon y `apkstart stop <package>` la cierra. `runtime stop` cierra el emulador y todas las sesiones.
- El `.desktop` guarda como `Exec` la ruta absoluta del binario que ejecutó `install` (normalmente `target/debug/apkstart`). Si mueves/borras el binario, reinstala.

## Quirks críticos

- **Persistencia:** el AVD no debe tener `disk.dataPartition.path=<temp>` y el arranque normal **no** debe usar `-no-snapshot-load`; debe usar quickboot. Si no, los APK desaparecen al reiniciar. `APKSTART_COLD_BOOT=1` es solo para recuperación. `doctor` avisa del AVD temporal.
- **Identidad de ventana:** `apkstart-engine` exporta `SDL_APP_ID=io.github.apkstart.<pkg>` a scrcpy. No lo quites: sin eso todas las apps usan la clase `scrcpy` y pierden icono/agrupación. El `.desktop` y `StartupWMClass` usan el mismo id.
- **Densidad de apps:** el display virtual es **552×984 a 215 dpi = 411 dp** (ancho de un Pixel 6), con `--flex-display`. Android dibuja por dp: si bajas el dpi, el texto crece y cabe menos; si lo subes, parece un teléfono pequeño. Al redimensionar la ventana el display cambia de píxeles con dpi fijo, así que el ancho lógico crece en vez de hacer zoom. No cambies `display_dpi` sin recalcular `dpi = round(ancho_px × 160 / ancho_dp)`.
- **UI de ajustes responsive:** `AdwBreakpointBin` colapsa el `AdwOverlaySplitView` a overlay por debajo de 800 px (botón de menú en el header) y el contenido se limita con `AdwClamp` a 1120 px. Las tarjetas usan `GtkFlowBox` homogéneo (`360`/`380` px mínimo): 3 columnas en ancho, 1 en ventana estrecha. Requiere libadwaita con features `v1_4` y `gtk_v4_10`; sin ellas no existen `OverlaySplitView` ni `BreakpointBin`.
- **Configuración:** `~/.config/apkstart/config.toml` (opcional) + variables `APKSTART_*`; `APKSTART_CONFIG` cambia el archivo. SDK por defecto `~/Android/Sdk`; scrcpy se autodetecta en `PATH` o `~/.local/opt/scrcpy`.
- **Datos:** SQLite en `~/.local/share/apkstart/apkstart.db`, esquema con `PRAGMA user_version` (hoy 2, categorías; el daemon añadirá 3). No cambies columnas sin migración.
- **Categorías:** los `.desktop` usan `Categories=` freedesktop a partir de `android:appCategory` + heurísticas de package/label; un `sync --refresh` regenera entradas antiguas.
- **APK:** el parser es `apk-info` (Apache-2.0), no un AXML propio. Entradas: `read_apk`, `read_icon` y `read_resource`.
- **Activity:** `apk-info` devuelve la primera activity de launcher; en F-Droid sale `IconActivity`. `scrcpy --start-app` abre bien, pero no asumas que `App.activity` es la principal.
- **Iconos:** se guardan en la carpeta hicolor según el tamaño del PNG (48×48 → `hicolor/48x48`); adaptive icons y elegir la máxima densidad siguen pendientes.
- **Integración APK:** `apkstart setup` crea `~/.local/share/applications/io.github.apkstart.install.desktop`, fija el handler MIME con `xdg-mime`, instala/activa `apkstartd` y retira el timer antiguo `apkstart-sync.timer`. El doble clic ejecuta `install --launch %f` con `Terminal=true`.
- **Sincronización:** `apkstartd` lista los paquetes Android `-3` cada 15 s, extrae el APK base a `~/.local/share/apkstart/cache`, crea `.desktop`/icono y borra entradas de apps desinstaladas. `apkstart sync` usa el daemon; `--refresh` re-registra todas. Si el daemon no está, `setup` deja el timer como respaldo y el CLI sincroniza directo (`--if-running --quiet`).
- **Recursos/logs:** la UI agrupa CPU instantánea/RAM de `apkstartd`, emulador, scrcpy, ADB y la propia UI; la GPU se lee global de `/sys/class/drm/card*/device` (no atribuible por proceso). `btop` los muestra por nombre y `systemd-cgls --user`/`systemd-cgtop --depth=5` agrupan por `models-apk.slice`. Los logs se refrescan cada 2 s (daemon + emulador + última sesión).
- **Sesiones scrcpy:** cerrar la ventana a mano no avisa al daemon; `Manager` verifica `/proc/<pid>/cmdline` antes de reutilizar una sesión y un limpiador cada 5 s elimina PIDs muertos. Si una app se reabre, debe lanzar una sesión nueva, no devolver un PID caducado.
- **`./app.apk` / instalación:** `packaging/install.sh` (root) instala el binario y registra `binfmt_misc` automáticamente. Cualquier paquete de distro debe ejecutar el mismo hook en `postinst`; no pidas al usuario el comando manual (ver `packaging/README.md`). El wrapper llama a `apkstart install --launch` como el usuario, no como root. No ejecutes el instalador en tests.
- **Iconos de respaldo:** si el APK solo trae un adaptive icon vectorial (o no trae bitmap), se genera un PNG 256×256 con la inicial y un color estable. No es el icono adaptativo real.
- **RAM:** en el host de desarrollo (14 GB) el guest va a 3 GB. No subir sin medir; el emulador compite con compilaciones.
- **Licencias:** el código propio es **MIT** (`LICENSE`). No commitear APK (`*.apk` está ignorado), imágenes de Android, emulador ni binarios de Google. El usuario instala y acepta el SDK.

## Mapa de código

- `apkstart-core`: `Config`, `Db`, modelos, `.desktop`/iconos, errores.
- `apkstart-apk`: wrapper de `apk-info`; `read_apk`, `read_resource`.
- `apkstart-engine`: `AndroidEmulator` (checks/arranque/parada), `Adb`, `ScrcpySession`.
- `apkstart-app`: install/registro/sync compartido entre CLI y daemon; aquí vive `register_app_from_apk` y `sync_apps`.
- `apkstart-daemon`: `Manager` (sesiones scrcpy en memoria, install/sync con lock) + `apkstartd` D-Bus y bucle de sync.
- `apkstart-cli`: binario `apkstart`; `src/daemon.rs` es el cliente D-Bus con fallback directo.
- `apkstart-settings`: UI GTK4/libadwaita; llama al binario `apkstart` (runtime/apps/instalar/diagnóstico) y crea su entrada de menú con `apkstart setup`. El diseño vive en `src/ui.rs` (variante B: hero, cards, métricas, pills) y es responsive: sidebar overlay por debajo de 800 px, contenido con `AdwClamp` a 1120 px y tarjetas en `GtkFlowBox` que pasan de 3 a 2 a 1 columna.
- Documentación y mensajes de usuario en español; identificadores y código en inglés.

## Documentos

- Presentación y guía rápida: `README.md` (capturas en `docs/images/`).
- Índice técnico: `docs/README.md`.
- Arquitectura, estructura, dependencias, instalación, bugs y desarrollo: `docs/arquitectura.md`, `docs/estructura.md`, `docs/dependencias.md`, `docs/instalacion.md`, `docs/bugs.md`, `docs/desarrollo.md`.
- Paquetes, CI y actualizaciones (Fase 2, aún no implementado): `docs/actualizaciones.md`.
- Diseño original: `docs/plan-inicial/`; laboratorio visual: `docs/design/settings-ui/`.
- Progreso, decisiones y riesgos: `docs/bitacora/bitacora.md`.
- Si cambias arquitectura o corriges un fallo relevante, añade entrada a la bitácora, actualiza `docs/bugs.md` y el checklist si aplica.
