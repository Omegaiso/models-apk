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
| 2026-11-27 | Nombre del proyecto/repositorio: **models-apk** (`github.com/Omegaiso/models-apk`) | Nombre elegido por el usuario. Los identificadores internos `apkstart-*` se mantienen hasta un rename controlado. |
| 2026-11-27 | Licencia del código propio: **MIT** (`LICENSE`) | Proyecto enteramente open source; licencia permisiva y simple. |
| 2026-11-27 | La instalación completa registra `binfmt_misc` automáticamente | Requisito del usuario: `./app.apk` debe funcionar sin pedir comandos con sudo al final. |

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

- [x] `.desktop` + icono por app (icono extraído del APK; fallback con inicial si es vectorial)
- [x] Arranque headless sin escritorio Android
- [x] Una ventana Wayland por app con título y `app_id` único (`SDL_APP_ID`)
- [x] Instalar APK desde el gestor de archivos (MIME) y por doble clic (`apkstart setup`)
- [x] Sincronización automática de apps instaladas dentro de Android (bucle del daemon cada 15 s; timer como respaldo)
- [x] Clasificación de apps Android en categorías freedesktop (Internet, Juegos, Multimedia, etc.)
- [x] Ejecución `./app.apk` vía `binfmt_misc` (script incluido; requiere root)
- [x] Licencia MIT
- [x] Instalador local (`packaging/install.sh`) con registro binfmt automático
- [ ] Paquete de distro (postinst ejecuta el hook binfmt; especificado en `packaging/README.md`)
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

## 4.1 Fases de implementación (acordadas 2026-11-27)

| Fase | Alcance | Estado |
|---|---|---|
| 1 | `apkstartd` + D-Bus `org.apkstart.Manager1`, sesiones, systemd --user, CLI cliente, sustituir el timer | Parcial: daemon integra runtime, install, sync, launch, stop y sync loop; faltan sesiones/adopción |
| 2 | UI GTK4/libadwaita: runtime, apps, instalar/desinstalar, ajustes, diagnóstico | Pendiente |
| 3 | Ventanas: persistencia de tamaño/estado, viewer propio o wrapper, límite multi-app, activity e iconos adaptativos | Parcial |
| 4 | Integración: notificaciones bridge, carpeta compartida, red/proxy/DNS, portapapeles/audio, cámara/micrófono/gamepad/USB | Pendiente |
| 5 | Empaquetado real Arch/deb/rpm, activación D-Bus, units systemd, hooks de instalación/desinstalación | Parcial: `packaging/install.sh` y hook binfmt |
| 6 | Calidad: tests de integración, matriz de compositores, fuzz, recuperación, documentación de límites | Pendiente |

**Nota de migración SQLite:** la clasificación de apps añade `apps.categories` como `user_version = 2`; la Fase 1 del daemon añadirá la tabla `sessions` como `user_version = 3`. No mezclar ambas migraciones en la misma versión.

**Criterio de cierre de la Fase 1:** el daemon es el único dueño del runtime; el CLI no arranca instancias paralelas; `runtime`, `launch`, `install`, `sync` y las sesiones funcionan por D-Bus; el timer se retira; systemd puede arrancar/parar el servicio y hay pruebas de integración con emulador.

**Criterio de clasificación de apps:** cada `.desktop` usa categorías freedesktop (`Network;WebBrowser;`, `Game;`, `AudioVideo;`, etc.) y aparece en la categoría correcta del lanzador KDE/GNOME.

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

### 2026-11-27 — Repositorio y nombre models-apk

- El proyecto se respalda en `https://github.com/Omegaiso/models-apk` (privado).
- Nombre de proyecto/repositorio: **models-apk**.
- Se mantienen los identificadores internos `apkstart-*`, el binario `apkstart` y las rutas `~/.config/apkstart` / `~/.local/share/apkstart` hasta un rename coordinado que migre configuración, base de datos, `.desktop`, iconos y `app_id`.
- Backup del primer slice de Fase 1, AGENTS.md, README y esta bitácora.

### 2026-11-27 — Escritorio, sincronización y APK ejecutable

**Problema detectado por el usuario:** instaló Lantern Sudoku desde F-Droid dentro del emulador y no aparecía fuera de F-Droid. El flujo anterior solo creaba `.desktop` cuando la instalación venía del CLI del host.

**Implementado**
- `apkstart setup`: registra el handler MIME `application/vnd.android.package-archive` con `xdg-mime`, crea `io.github.apkstart.install.desktop` (doble clic = `install --launch %f`) y activa un timer de `systemd --user` cada 15 s.
- `apkstart sync`: lista los paquetes `-3` de Android, extrae el APK base a `~/.local/share/apkstart/cache`, parsea label/icono/activity y crea/actualiza `.desktop`, icono y SQLite. También elimina entradas de apps desinstaladas. `--refresh` re-registra todas; `--if-running --quiet` es el modo del timer.
- `install --launch`: instala y abre la app; lo usa el handler de doble clic y el wrapper binfmt.
- Icono de respaldo: si el APK solo tiene adaptive icon vectorial, se genera un PNG 256×256 con la inicial y un color estable.
- Iconos hicolor por tamaño real (F-Droid 48×48; fallback 256×256) y refresco de `gtk-update-icon-cache`, `update-desktop-database` y `kbuildsycoca6`.
- Licencia **MIT** (`LICENSE`, `Cargo.toml`, README).
- `packaging/binfmt/install.sh` + `models-apk.conf`: registra `./app.apk` mediante `binfmt_misc`; requiere root y `chmod +x`. No se ejecutó en esta sesión.

**Verificación**
- Tras `setup`, Lantern Sudoku y F-Droid aparecen en `~/.local/share/applications` con icono; `xdg-mime query default application/vnd.android.package-archive` devuelve `io.github.apkstart.install.desktop`.
- `apkstart-sync.timer` está activo; las ejecuciones del servicio terminan con status 0.
- `apkstart sync` sin cambios devuelve 0/0; `sync --refresh` regenera entradas.
- `launch io.github.ukpratik.lanternsudoku` abre la ventana con `resourceClass=io.github.apkstart.io.github.ukpratik.lanternsudoku`.
- `desktop-file-validate` pasa en las entradas generadas.

**Limitaciones**
- El timer apunta al binario actual (`target/debug/apkstart` en desarrollo). Si se mueve o se ejecuta `cargo clean`, hay que repetir `apkstart setup`.
- El icono real del adaptive icon vectorial no se renderiza; se usa el fallback con inicial.
- `binfmt_misc` no se registró por requerir root; el script está listo y validado con `bash -n`.
- El timer es un puente hasta `apkstartd`; no sustituye al daemon del plan.
- El handler de doble clic usa `Terminal=true` para mostrar progreso; la UI GTK llegará después.

### 2026-11-27 — Instalación automática de binfmt

**Requisito del usuario:** en la versión completa, el registro de `binfmt_misc` para `./app.apk` debe ser automático al instalar; no se debe pedir `sudo packaging/binfmt/install.sh ...` al usuario final.

**Implementado**
- `packaging/install.sh`: instala el binario, licencia, handler `.desktop` y scripts binfmt en el prefijo (`/usr/local` o `/usr`), y llama automáticamente a `packaging/binfmt/install.sh` con el binario instalado.
- `packaging/binfmt/uninstall.sh`: limpia la entrada binfmt y el wrapper para los hooks de desinstalación.
- `packaging/README.md`: regla obligatoria para paquetes de distro (`postinst`/`post_install` debe ejecutar el hook binfmt; `prerm`/`pre_remove` debe desregistrarlo).
- README y AGENTS documentan que la instalación completa registra binfmt sin intervención y que el comando manual es solo para desarrollo.

**Estado**
- Scripts validados con `bash -n`; la plantilla `.desktop` se puede validar con `desktop-file-validate`.
- No se ejecutó `packaging/install.sh` porque requiere root y esta sesión no tiene sudo sin contraseña. El paquete de distro real queda pendiente.
- El instalador local no activa el timer/MIME de cada usuario; eso sigue siendo `apkstart setup` por usuario hasta que exista `apkstartd`/empaquetado completo.

### 2026-11-27 — Clasificación de apps por categorías

- `ApkInfo` incluye `android_category` y `categories`; se leen `android:appCategory` y se aplican heurísticas por package/label, con permisos solo para accesibilidad.
- Mapeo principal: navegador → `Network;WebBrowser;`, juego → `Game;`, audio/vídeo → `AudioVideo;`, imagen → `Graphics;`, social → `Network;InstantMessaging;`, productividad → `Office;`, F-Droid → `System;`, desconocido → `Utility;`.
- `App.categories` y SQLite `user_version = 2` con migración `ALTER TABLE apps ADD COLUMN categories ...` sin perder datos.
- `.desktop` escribe `Categories=` saneado y semicolon-terminated; F-Droid aparece en Sistema y un juego en Juegos.
- Tests añadidos: 9 de clasificación/APK, migración v1→v2 y formato/saneo de categorías; `cargo test --workspace` verde (11+6+19+2), clippy limpio.
- Las entradas existentes no cambian hasta `apkstart sync --refresh` o reinstalar; F-Droid ya se regeneró como `System;`.

### 2026-11-27 — Daemon mínimo y cliente D-Bus

**Implementado**
- Nuevo crate `apkstart-daemon` con librería `Manager` y binario `apkstartd`.
- D-Bus de sesión `org.apkstart.Manager1` en `/org/apkstart/Manager1`: `GetVersion`, `GetRuntimeStatus`, `StartRuntime`, `StopRuntime`, `ListApps`, `Launch`, `StopApp`. Los compuestos viajan como JSON temporal.
- `Manager` mantiene sesiones scrcpy en memoria; `Launch` reutiliza una sesión viva, `StopApp` la cierra. `runtime start/stop` delega en `AndroidEmulator`.
- `apkstart setup` instala `apkstartd.service` y la activación D-Bus de usuario, y lo arranca con `systemctl --user`; el timer de sync se mantiene porque el daemon aún no sincroniza.
- `apkstart launch` usa el daemon si está disponible y no se pasan opciones de tamaño/título; `apkstart stop <pkg>` cierra la sesión del daemon. Si el daemon no está, `launch` cae al modo directo.
- `packaging/install.sh` instala `apkstartd`, una unit de usuario y el service file de D-Bus.

**Verificación**
- `systemctl --user status apkstartd` activo; `GetVersion`/`GetRuntimeStatus`/`ListApps` responden.
- `apkstart launch org.fdroid.fdroid` abre la ventana con `daemon pid=...`; `apkstart stop org.fdroid.fdroid` la cierra.
- `cargo test --workspace` verde (11+6+19+2), `clippy -D warnings` limpio.
- No se añadió todavía la tabla `sessions` ni la adopción de procesos; el daemon nuevo no reclama sesiones abiertas por el CLI.

**Pendiente de Fase 1**
- Mover `install` y `sync` al daemon, con serialización install/sync.
- Tabla `sessions` (migración v3), adopción de scrcpy por `/proc`, watchdog del runtime.
- Bucle de sync dentro del daemon y retirada del timer.
- `launch` con opciones de tamaño/título por D-Bus y `stop` de sesiones abiertas en modo directo.
- Tests D-Bus y de sesiones; empaquetado definitivo.

### 2026-11-27 — El daemon asume install/sync y se retira el timer

**Implementado**
- Nuevo crate `apkstart-app` con la lógica compartida `install_apk`, `register_app_from_apk` y `sync_apps`; CLI y daemon ya no duplican el flujo.
- `apkstartd` incorpora `Install(path, launch)` y `Sync(refresh)` a D-Bus.
- `Manager` serializa install/sync con un lock; el daemon sincroniza cada 15 s y no falla si el runtime está apagado.
- `apkstart setup` instala/activa el daemon y **elimina el timer antiguo**; si el daemon no está disponible, deja el timer como respaldo.
- `apkstart install`, `install --launch`, `sync`, `launch` y `stop` usan el daemon cuando está; si no, install/sync/launch caen al modo directo compartido.
- `packaging/install.sh` instala `apkstartd`, la unit de usuario y el service file D-Bus.
- La activación D-Bus usa `systemctl --user start apkstartd.service`, por lo que el daemon queda bajo systemd (no como proceso huérfano).

**Verificación**
- `apkstart setup` deja `apkstartd.service` activo y `apkstart-sync.timer` inactivo/eliminado.
- `busctl` muestra `Install`, `Sync`, `Launch`, `StopApp`.
- `apkstart sync` responde `Apps añadidas: 0; eliminadas: 0` por D-Bus.
- `apkstart install --launch /tmp/opencode/F-Droid.apk` instala y abre; `apkstart stop org.fdroid.fdroid` cierra la sesión del daemon.
- `cargo test --workspace` verde (11+7+19+2; app/daemon sin tests propios), `clippy -D warnings` limpio.

**Pendiente**
- Tabla `sessions` (v3) y adopción de scrcpy tras reiniciar el daemon.
- `Launch` con tamaño/título por D-Bus; stop de sesiones abiertas en modo directo.
- Tests de integración D-Bus y de sesiones.

### 2026-11-27 — Runtime y parada a través del daemon

- `apkstart runtime status/start/stop` usan `apkstartd` cuando está disponible; si no, mantienen el camino directo.
- `Manager` serializa start/stop/install/sync con un lock de operación para que el bucle de sync no compita con el arranque o la parada.
- `StopRuntime` limpia las sesiones scrcpy del daemon.
- El timeout de parada del emulador sube de 20 s a 60 s: con quickboot, guardar el snapshot puede tardar más. Verificado: parada real en ~15 s.
- Verificación: `runtime status` por D-Bus, `runtime start` (~10 s con quickboot), `runtime stop` (~15 s), `launch`/`stop` de F-Droid; todo por el daemon.
- Pendiente: tabla `sessions` v3, adopción de scrcpy y tests D-Bus.


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
| Icono de alta resolución / adaptive icons | Activo | Se usa el bitmap si existe; si solo hay vectorial, fallback PNG con inicial. Falta renderizar el adaptive real y elegir la máxima densidad |
| Timer de sincronización atado al binario | Resuelto en gran parte | El daemon hace el sync; el timer solo queda como respaldo si `apkstartd` no está. Repetir `setup` si se mueve el binario. |
| `./app.apk` requiere root y binfmt | Conocido | Script `packaging/binfmt/install.sh`; no se registró en esta sesión |
| Activity principal | Activo | `apk-info` devuelve la primera activity de launcher (`IconActivity` en F-Droid); `scrcpy --start-app` funciona, pero la metadata debe corregirse |
| Daemon y D-Bus | Parcial | `apkstartd` ya hace runtime/install/sync/launch/stop y sync loop; faltan tabla `sessions` y adopción de scrcpy |
| Android 15+ endurece displays virtuales | Conocido | Fijar API 34; imagen Google APIs para permitir `adb root` |
| Imagen Play no redistribuible | Conocido | No empaquetar; descarga y aceptación por el usuario |
| `FLAG_SECURE` / DRM / anticheat | Conocido | Documentar; no prometer compatibilidad |
| GPU AMD con `-gpu host` | Por validar | Fallback `swiftshader_indirect`; medir |
