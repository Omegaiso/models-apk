# AGENTS.md — models-apk (ApkStart Linux)

## Contexto que no se deduce de los nombres

- Proyecto/repo: **models-apk** (`github.com/Omegaiso/models-apk`). Los identificadores internos siguen siendo `apkstart-*` y el binario `apkstart`; no hagas un rename masivo sin migrar configuración, SQLite, `.desktop` y `app_id`.
- El repo implementa hoy **solo 4 crates**: `apkstart-core`, `apkstart-apk`, `apkstart-engine` y `apkstart-cli`.
- `docs/plan-inicial/` describe el diseño completo (daemon, GTK4, viewer propio, QEMU). **Aún no existen** `apkstartd`, D-Bus, `apkstart-settings`, `apkstart-viewer` ni `apkstart-integration`. No escribas UI o servicios asumiendo que están.
- El flujo actual es **CLI → `AndroidEmulator`/ADB/scrcpy directamente**. No hay daemon ni systemd.
- Motor fijado para el MVP: **Android Emulator oficial + API 34 x86_64 `google_apis`**. Prioridad: estabilidad y usabilidad. No migrar a QEMU puro/Waydroid ni introducir Electron/Node/Java en el producto host (el SDK/`sdkmanager` puede usar Java como herramienta).
- La bitácora vigente es **`docs/bitacora/bitacora.md`** (el README todavía enlaza `docs/bitacora.md`). Es append-only: al cerrar un bloque añade una entrada; no reescribas el historial.

## Comandos

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# El paquete es apkstart-cli; el binario es apkstart
cargo run -p apkstart-cli -- doctor
target/debug/apkstart doctor

# Un test / un paquete
cargo test -p apkstart-core db::tests::upsert_and_read_app
cargo test -p apkstart-engine

# El test de APK real se salta si falta /tmp/opencode/F-Droid.apk
APKSTART_TEST_APK=/ruta/app.apk cargo test -p apkstart-apk parses_real_apk_when_available
```

No hay CI ni task runner: esas tres verificaciones (`fmt`, `clippy`, `test`) son el estándar antes de cerrar un cambio.

## Prueba funcional (manual, con efectos reales)

Requiere KVM, SDK, AVD `apkstart` y scrcpy. `install` arranca el emulador si hace falta, escribe en `~/.local/share`, crea `.desktop`/icono y toca SQLite; no lo ejecutes como parte de tests.

```bash
target/debug/apkstart doctor
target/debug/apkstart runtime start
target/debug/apkstart install /ruta/app.apk
target/debug/apkstart launch <package>
target/debug/apkstart runtime stop
```

- Para inspeccionar ADB usa el **puerto propio**: `ANDROID_ADB_SERVER_PORT=5038 adb devices`, no el 5037 del usuario.
- Logs: `~/.local/state/apkstart/emulator.log` y `~/.local/state/apkstart/sessions/<package>.log`.
- `launch` deja scrcpy vivo al salir el CLI; `runtime stop` lo cierra.
- El `.desktop` guarda como `Exec` la ruta absoluta del binario que ejecutó `install` (normalmente `target/debug/apkstart`). Si mueves/borras el binario, reinstala.

## Quirks críticos

- **Persistencia:** el AVD no debe tener `disk.dataPartition.path=<temp>` y el arranque normal **no** debe usar `-no-snapshot-load`; debe usar quickboot. Si no, los APK desaparecen al reiniciar. `APKSTART_COLD_BOOT=1` es solo para recuperación. `doctor` avisa del AVD temporal.
- **Identidad de ventana:** `apkstart-engine` exporta `SDL_APP_ID=io.github.apkstart.<pkg>` a scrcpy. No lo quites: sin eso todas las apps usan la clase `scrcpy` y pierden icono/agrupación. El `.desktop` y `StartupWMClass` usan el mismo id.
- **Configuración:** `~/.config/apkstart/config.toml` (opcional) + variables `APKSTART_*`; `APKSTART_CONFIG` cambia el archivo. SDK por defecto `~/Android/Sdk`; scrcpy se autodetecta en `PATH` o `~/.local/opt/scrcpy`.
- **Datos:** SQLite en `~/.local/share/apkstart/apkstart.db`, esquema con `PRAGMA user_version` (hoy 1). No cambies columnas sin migración.
- **APK:** el parser es `apk-info` (Apache-2.0), no un AXML propio. Entradas: `read_apk` y `read_resource`.
- **Activity:** `apk-info` devuelve la primera activity de launcher; en F-Droid sale `IconActivity`. `scrcpy --start-app` abre bien, pero no asumas que `App.activity` es la principal.
- **Iconos:** se guardan siempre en `hicolor/256x256` aunque el PNG sea 48×48; adaptive icons pendiente.
- **RAM:** en el host de desarrollo (14 GB) el guest va a 3 GB. No subir sin medir; el emulador compite con compilaciones.
- **Licencias:** no commitear APK (`*.apk` está ignorado), imágenes de Android, emulador ni binarios de Google. El usuario instala y acepta el SDK.

## Mapa de código

- `apkstart-core`: `Config`, `Db`, modelos, `.desktop`/iconos, errores.
- `apkstart-apk`: wrapper de `apk-info`; `read_apk`, `read_resource`.
- `apkstart-engine`: `AndroidEmulator` (checks/arranque/parada), `Adb`, `ScrcpySession`.
- `apkstart-cli`: único binario `apkstart`; subcomandos en `main.rs`.
- Documentación y mensajes de usuario en español; identificadores y código en inglés.

## Documentos

- Diseño: `docs/plan-inicial/`.
- Progreso, decisiones y riesgos: `docs/bitacora/bitacora.md`.
- Si cambias arquitectura o corriges un fallo relevante, añade entrada a la bitácora y actualiza el checklist si aplica.
