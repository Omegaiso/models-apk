# Bitácora de progreso — ApkStart Linux

> **Regla:** este documento se actualiza al cerrar cada bloque de trabajo. Es append-only en la sección de registro: no se borran entradas, se corrigen con una nota nueva.
> **Objetivo de la etapa actual:** completar la Fase 1 (MVP) con prioridad en **estabilidad y usabilidad**. Las optimizaciones grandes y el cambio de motor quedan para después de tener el sistema completo.

---

## 1. Decisiones vigentes

| Fecha | Decisión | Motivo |
|---|---|---|
| 2026-11-27 | Motor del MVP: **Android Emulator oficial de Google** (QEMU-based), no QEMU puro | Displays virtuales, imágenes actualizadas, GMS, traducción ARM, GPU y snapshots ya integrados. QEMU puro queda como backend de fase 2. |
| 2026-11-27 | Imagen objetivo: **Android 14 / API 34 x86_64** | `scrcpy --new-display` funciona sin root en API 34. Android 15+ puede exigir `ADD_TRUSTED_DISPLAY` y `adb root`. |
| 2026-11-27 | Imagen de spike: **Google APIs** (sin Play Store) | Tiene GMS y suele permitir `adb root`; evita las restricciones de la imagen Play. |
| 2026-11-27 | Prioridad: **estabilidad y usabilidad** del sistema completo | No se optimiza ni se migra de motor antes de cerrar la Fase 1. |
| 2026-11-27 | SDK de Android en el home del usuario (`~/Android/Sdk`) | No requiere root ni contamina el sistema; el usuario controla licencias e imágenes. |

---

## 2. Entorno de desarrollo (snapshot 2026-11-27)

| Elemento | Estado |
|---|---|
| Distro | CachyOS (Arch-based), kernel 7.3.6 |
| CPU | AMD Ryzen 5 7535HS, 12 hilos |
| RAM | 14 GB totales; ~4 GB disponibles al inicio (hay un `rustc` de otra sesión consumiendo ~2,2 GB) |
| Disco `/home` | 93 GB libres (74 % usado) tras limpieza del usuario |
| GPU | AMD Rembrandt (Mesa 26.3.1); VA-API H.264/HEVC activo |
| Sesión | Wayland (`wayland-1`), Hyprland |
| KVM | Disponible; `/dev/kvm` en modo `666` (no requiere grupo `kvm`) |
| Rust | 1.99.1 |
| Java | OpenJDK 27.0.1 |
| Android SDK / adb / scrcpy / emulator | Instalados el 2026-11-27: cmdline-tools 19.0, platform-tools 37.0.3, emulator 37.3.10, system image API 34 `google_apis` x86_64 |
| scrcpy | 5.0.1 (paquete extraído en `~/.local/opt/scrcpy`; falta instalación limpia del sistema) |
| AVD | `apkstart` (Pixel 7, 3 GB RAM, 4 vCPU, 6 GB datos, GPU host) |
| Repos | `android-tools` 37.0.0, `scrcpy` 5.0.1, `sdl3` ya instalado |

### Restricciones detectadas

- RAM ajustada: el emulador debe arrancar con **3 GB de guest como máximo** para el spike; cerrar aplicaciones pesadas antes de probar apps reales.
- `sudo` requiere contraseña: la instalación de paquetes del sistema se hará cuando el usuario la introduzca; el SDK y scrcpy pueden instalarse en el home para no bloquear el spike.
- El disco es suficiente para imagen + AVD, pero hay que vigilar snapshots y datos del AVD.

---

## 3. Fase 0 — Spike de viabilidad

**Criterio GO:** app real visible y usable en ventana Wayland, sin escritorio Android, resize flexible, input, audio y portapapeles, latencia < 80 ms percibidos, y funcionamiento en al menos 3 compositores.

| # | Tarea | Estado |
|---|---|---|
| 0.1 | Instalar platform-tools, emulator y scrcpy | Hecho |
| 0.2 | Crear AVD API 34 x86_64 Google APIs (3–4 GB RAM, 4 vCPU) | Hecho (`apkstart`, 3 GB, 4 vCPU) |
| 0.3 | Arrancar headless con KVM y `-gpu host` | Hecho |
| 0.4 | Verificar ADB y `sys.boot_completed` | Hecho (arranque frío ≈ 22 s) |
| 0.5 | Crear display virtual con `scrcpy --new-display` | Hecho (display virtual + app real) |
| 0.6 | Validar app real, resize, input, audio y portapapeles | Parcial: app y resize OK; audio con pista Opus; input/clipboard pendientes de prueba manual |
| 0.7 | Medir latencia, CPU, GPU y RAM | Parcial: arranque y consumo observados; latencia input-to-photon pendiente |
| 0.8 | Probar en Hyprland, GNOME y KDE | Parcial: validado en KDE Wayland; falta Hyprland y GNOME |
| 0.9 | Probar app ARM-only y app con `FLAG_SECURE` | Pendiente |
| 0.10 | Documento GO/NO-GO | GO provisional (ver registro) |

---

## 4. Fase 1 — MVP (checklist viva)

### Núcleo

- [x] Workspace Rust (`crates/`, `Cargo.toml`, `rustfmt`, `clippy`)
- [x] `apkstart-core`: tipos, errores, configuración, SQLite + migraciones
- [x] `apkstart-apk`: manifiesto, label, versión, activity, ABIs, permisos e icono (vía `apk-info`, Apache-2.0)
- [x] `apkstart-engine`: `AndroidEmulator`, cliente ADB y sesiones scrcpy (falta extraer el trait `Engine` formal para el backend QEMU futuro)
- [ ] `apkstart-daemon`: `apkstartd`, D-Bus `org.apkstart.Manager1`, systemd --user
- [x] `apkstart-cli`: doctor, runtime start/stop/status, install, launch, list, uninstall, settings, export-log
- [ ] `apkstart-viewer` propio: el MVP usa scrcpy directamente, con `SDL_APP_ID` por app
- [ ] `apkstart-settings`: GTK4 + libadwaita
- [ ] `apkstart-integration`: red, audio, notificaciones, carpeta compartida (la entrada `.desktop` y el icono ya funcionan)

### Producto

- [x] `.desktop` + icono por app (icono extraído del APK; resolución aún limitada)
- [x] Arranque headless sin escritorio Android
- [x] Una ventana Wayland por app con título y `app_id` único (`SDL_APP_ID`)
- [ ] Instalar APK desde el gestor de archivos (MIME); hoy solo CLI
- [ ] Persistencia de tamaño/estado por app (tabla creada; falta conectar el viewer)
- [ ] Audio, portapapeles y carpeta compartida integrados en la app (audio validado en el spike)
- [ ] Red avanzada (proxy/DNS/port forwarding); el NAT básico lo pone el emulador
- [ ] Ajustes GTK (runtime, apps, red, periféricos, integración, diagnóstico)
- [x] `apkstart doctor` y exportación de diagnóstico
- [ ] Empaquetado de prueba (AppImage o deb)
- [ ] Documentación de instalación y limitaciones

### Calidad

- [x] Tests unitarios del parser APK y de la DB
- [ ] Test de integración automatizado con emulador headless (probado manualmente)
- [ ] Matriz Hyprland / GNOME / KDE (solo KDE por ahora)
- [ ] Fuzz mínimo del parser y prueba de zip-slip
- [x] Revisión de licencias documentada (no redistribuir Google)

---

## 5. Registro de trabajo

### 2026-11-27 — Arranque del proyecto

- Se revisó el plan inicial completo (`docs/plan-inicial/`).
- Se confirmó el motor del MVP: Android Emulator oficial; QEMU puro se pospone a fase 2.
- Se inspeccionó el host: KVM disponible, GPU AMD con VA-API, Wayland/Hyprland, 93 GB libres y RAM ajustada.
- Se detectó que `sudo` pide contraseña y que aún no hay SDK, adb ni scrcpy instalados.
- Se creó esta bitácora.
- Siguiente: instalar SDK/emulator, crear AVD API 34 y ejecutar el spike.

### 2026-11-27 — Spike de viabilidad ejecutado

**Instalación**
- SDK en `~/Android/Sdk` sin root: cmdline-tools 19.0, platform-tools 37.0.3, emulator 37.3.10, system image `system-images;android-34;google_apis;x86_64` (4,2 GB).
- AVD `apkstart` creado con perfil Pixel 7; ajustes: 3 GB RAM, 4 vCPU, 6 GB de datos, `hw.gpu.mode=host`, teclado físico.
- scrcpy 5.0.1 probado desde `~/.local/opt/scrcpy` (servidor en `usr/share/scrcpy/scrcpy-server`, requiere `SCRCPY_SERVER_PATH`). La instalación limpia por `pacman` queda pendiente porque `sudo` pide contraseña.

**Resultados**
- Arranque headless con `-no-window -gpu host -accel on`: correcto; `sys.boot_completed=1` en ~22 s.
- ADB detecta `emulator-5556`; Android 14 / API 34 / x86_64.
- `scrcpy --new-display=900x1600/240 --start-app=...`: crea display virtual, arranca F-Droid y Settings y emite vídeo.
- `--flex-display`: al redimensionar la ventana de 552×984 a 720×972, Android recibió el nuevo tamaño y la app refloweó. **Validado.**
- Ventana nativa en KDE Wayland (SDL3) con título propio. Captura de pantalla confirma que no se ve el escritorio Android.
- Audio: scrcpy inició captura y el MP4 de prueba contiene pista Opus 48 kHz estéreo; falta verificar sonido audible de una app real.
- `--display-ime-policy=local`: el teclado Android aparece dentro del display virtual. **Validado.**
- Consumo observado: emulador ≈ 1,8 GB RSS con guest de 3 GB; arranque en frío rápido con KVM.

**Bloqueos del spike**
- La inyección automática de teclado/ratón en el host no es trivial en KDE Wayland: `wtype` no está soportado por KWin, `ydotool` no pudo abrir `/dev/uinput` (módulo no cargado) y `xdotool` solo funciona si scrcpy se fuerza a XWayland. La prueba de input queda para validación manual sobre la ventana de scrcpy.
- El portapapeles bidireccional no se validó aún.
- Solo se probó el compositor KDE; Hyprland y GNOME quedan pendientes.
- `FLAG_SECURE` y apps ARM-only quedan pendientes.
- La ventana de scrcpy usa `app_id`/clase `scrcpy`; la identidad única por app requiere parchear scrcpy o el viewer propio (decisión de fase 2, pero impacta el MVP).

**Decisión:** GO provisional. El riesgo principal (display virtual + app real + ventana Wayland) está despejado. Se continúa con el esqueleto Rust y el motor Android Emulator; las validaciones manuales pendientes se cierran en paralelo.

### 2026-11-27 — Primer slice de Fase 1

**Código creado**
- Workspace Rust (`resolver = 2`, edition 2024) con `apkstart-core`, `apkstart-apk`, `apkstart-engine` y `apkstart-cli`. Repositorio Git inicializado.
- `apkstart-core`: configuración XDG + env `APKSTART_*`; SQLite con WAL y tablas `apps`, `window_state`, `settings`; modelos; integración `.desktop`/iconos.
- `apkstart-apk`: usa `apk-info` (Apache-2.0) para AXML/ARSC; extrae package, label, versión, activity, ABIs, permisos y recurso de icono; `read_resource` para extraer el PNG/WebP.
- `apkstart-engine`: `AndroidEmulator` (checks, arranque headless, parada, estado), cliente ADB con timeouts y sesión scrcpy con `--new-display`, `--flex-display`, `--display-ime-policy=local`.
- `apkstart-cli`: `doctor`, `runtime start|stop|status`, `install`, `launch`, `list`, `uninstall`, `settings`, `export-log`.
- Identidad de ventana resuelta sin fork: `SDL_APP_ID=io.github.apkstart.<package>` hace que SDL3/Wayland use ese `app_id` y X11 esa clase. El `.desktop` usa el mismo nombre y `StartupWMClass`, por lo que el compositor puede asociar icono y ventana.
- Iconos: se extrae el recurso resuelto por `apk-info` y se guarda en `~/.local/share/icons/hicolor/256x256/apps/`. En F-Droid el recurso resuelto es un PNG de 48×48; la selección de la densidad máxima/adaptive icon queda pendiente.

**Pruebas**
- `cargo check --workspace` y `cargo test --workspace`: 22 tests, todos verdes.
- `apkstart doctor`: KVM, adb, emulator, AVD, scrcpy, scrcpy-server, Wayland y D-Bus OK.
- Flujo real: `runtime start` → `install /tmp/opencode/F-Droid.apk` → `list` → `launch org.fdroid.fdroid`; la ventana se abre, scrcpy sobrevive a la salida del CLI y no aparece el escritorio Android.
- Se corrigió un bug del cliente ADB: `shell` no añadía el subcomando `shell`, por lo que `sys.boot_completed` nunca se leía.
- Se corrigió el entorno de scrcpy: el CLI pasa el directorio del `adb` del SDK en `PATH`, porque scrcpy invoca `adb` por nombre.

**Limitaciones de este slice**
- No hay daemon ni D-Bus todavía; el CLI habla directamente con el motor. Es una desviación temporal y consciente para validar la UX.
- La UI GTK4, el visor propio y la persistencia de tamaño no están conectados.
- La activity registrada para F-Droid es `org.fdroid.IconActivity` (primera activity de launcher que devuelve `apk-info`), no necesariamente la principal. `scrcpy --start-app` sí resuelve el launcher correcto; hay que mejorar la selección de activity.
- La sesión de escritorio se bloqueó durante las pruebas; no se pudo verificar visualmente el icono en la barra de tareas, pero `kdotool` confirma `resourceClass=io.github.apkstart.org.fdroid.fdroid`.
- Faltan las pruebas manuales de input, portapapeles, audio audible y los compositores Hyprland/GNOME.

### 2026-11-27 — Persistencia de datos y ADB aislado

**Hallazgo crítico:** los APK instalados desaparecían tras reiniciar el emulador. Dos causas combinadas:
1. El AVD creado por `avdmanager` traía `disk.dataPartition.path=<temp>`: la partición de datos era temporal.
2. El CLI arrancaba siempre con `-no-snapshot-load`, que fuerza cold boot y no carga el estado guardado por quickboot.

**Correcciones**
- AVD: se eliminó `disk.dataPartition.path=<temp>`, se dejó `disk.dataPartition.size=6G` y `userdata.useQcow2=yes`.
- Arranque normal sin `-no-snapshot-load`: el primer arranque de un AVD nuevo es en frío; los siguientes cargan el snapshot quickboot y conservan datos. `APKSTART_COLD_BOOT=1` (o `cold_boot = true` en TOML) fuerza cold boot para recuperación.
- ADB dedicado en el puerto **5038** (`APKSTART_ADB_PORT`), aplicado al proceso del emulador, al cliente ADB y a scrcpy. Así no se toca el servidor ADB del usuario ni sus dispositivos físicos.
- `doctor` añade la comprobación `AVD datos` y muestra el puerto de ADB.

**Verificación**
- `install F-Droid` → `runtime stop` → `runtime start` → `pm list packages -3` sigue mostrando `org.fdroid.fdroid`.
- `launch org.fdroid.fdroid` funciona tras el reinicio y mantiene `app_id`/título.
- El arranque con quickboot es notablemente más rápido que el cold boot.

**Riesgo residual:** un snapshot corrupto puede impedir el arranque; la recuperación es `APKSTART_COLD_BOOT=1` y, si hace falta, borrar `~/.android/avd/<avd>/snapshots/`. Se probará en el bloque de recuperación de fallos.

**Checkpoint:** commit `0f62fe7` con el primer slice funcional y la documentación.

**Siguiente bloque recomendado**
1. `apkstartd` + D-Bus `org.apkstart.Manager1` (mover la lógica del CLI al daemon, systemd --user).
2. Persistencia de tamaño/estado de ventana conectada a scrcpy/viewer.
3. UI GTK4/libadwaita mínima (runtime, apps, diagnóstico).
4. Manejador MIME para instalar APK desde el gestor de archivos.
5. Selección correcta de activity e icono de máxima densidad.
6. Pruebas manuales de input, portapapeles, audio audible y Hyprland/GNOME.

---

## 6. Bloqueos y riesgos activos

| Riesgo | Estado | Mitigación |
|---|---|---|
| RAM disponible (~4 GB) insuficiente para guest de 4 GB | Activo | Guest de 3 GB; cerrar aplicaciones; vigilar swap |
| AVD con partición de datos temporal | Resuelto | `doctor` detecta `disk.dataPartition.path=<temp>`; AVD de desarrollo corregido y persistente |
| Snapshot quickboot corrupto | Conocido | `APKSTART_COLD_BOOT=1` para arranque en frío; borrar snapshots si persiste |
| ADB compartido con el usuario | Resuelto | Servidor ADB propio en 5038 (`APKSTART_ADB_PORT`) |
| `sudo` con contraseña | Activo | Instalación local en `~/Android/Sdk`; paquetes del sistema cuando el usuario autorice |
| Input sintético en KDE Wayland | Activo | Validar manualmente; para automatizar, usar XWayland o cargar `uinput` con permiso |
| Identidad de ventana (`app_id`) única por app | Resuelto | `SDL_APP_ID=io.github.apkstart.<pkg>` + `.desktop` con el mismo nombre y `StartupWMClass` |
| Icono de alta resolución / adaptive icons | Activo | Hoy se usa el recurso resuelto por `apk-info`; en F-Droid es 48×48. Falta elegir la máxima densidad o parsear el XML adaptativo |
| Activity principal | Activo | `apk-info` devuelve la primera activity de launcher (`IconActivity` en F-Droid); `scrcpy --start-app` funciona, pero la metadata debe corregirse |
| Daemon y D-Bus | Pendiente | El CLI llama al motor directamente; el siguiente bloque crea `apkstartd` y mueve la lógica allí |
| Android 15+ endurece displays virtuales | Conocido | Fijar API 34; imagen Google APIs para permitir `adb root` |
| Imagen Play no redistribuible | Conocido | No empaquetar; descarga y aceptación por el usuario |
| `FLAG_SECURE` / DRM / anticheat | Conocido | Documentar; no prometer compatibilidad |
| GPU AMD con `-gpu host` | Por validar | Fallback `swiftshader_indirect`; medir |
