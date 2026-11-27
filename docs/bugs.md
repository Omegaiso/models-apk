# Bugs, limitaciones y riesgos

Última actualización: **2026-11-27**.

Estados:

- **Resuelto**: causa identificada y corrección verificada.
- **Abierto**: reproducido o reportado; pendiente de corrección.
- **Parcial**: funciona con límites conocidos.
- **Asumido**: limitación de diseño del MVP; no se corrige por ahora.

---

## 1. Bugs abiertos

### Ventana negra al abrir una app

**Síntoma.** Al abrir una app (observado con F-Droid) la ventana de scrcpy puede
quedarse completamente negra. Cerrar la ventana por completo y volver a abrirla
suele resolverlo. El proceso scrcpy puede seguir vivo mientras la ventana está
negra, por lo que el daemon la considera una sesión válida y no la relanza.

**Estado.** **Abierto.** Reportado por el usuario durante las pruebas de captura
del 2026-11-27. No se ha reproducido de forma determinista; en la última prueba
de ciclo cerrar/abrir la ventana renderizó correctamente.

**Causas plausibles.**

1. **Carrera de lanzamiento en `Manager::launch`.** El método comprueba el mapa
   de sesiones, suelta el lock y luego lanza scrcpy. Dos llamadas concurrentes al
   mismo package (doble clic, reintento del CLI o de la UI) pueden crear dos
   procesos scrcpy; uno queda huérfano y puede mostrar una ventana negra. La
   comprobación de PID vivo no detecta que la ventana sea inutilizable.
2. **Display virtual en Doze / pantalla apagada.** scrcpy documenta que los
   displays virtuales pueden quedar negros cuando el dispositivo entra en Doze
   ([scrcpy#6787](https://github.com/Genymobile/scrcpy/issues/6787)); `--keep-active`
   mantiene despierto un dispositivo ya despierto, pero no despierta uno dormido
   en todas las versiones de Android. En Android 14 el emulador suele despertar
   al crear el display, pero no está garantizado.
3. **App reanudada en un display destruido.** Al cerrar scrcpy se destruye el
   display virtual, pero el proceso de la app puede sobrevivir (F-Droid lo hace).
   Un `--start-app` posterior puede reutilizar una task/actividad ligada al
   display antiguo y producir un primer frame negro.

**Workaround actual.** Cerrar la ventana con `apkstart stop <package>` (o el
botón Cerrar de la UI) y volver a abrir. El limpiador de sesiones cada 5 s retira
el PID muerto y el siguiente `launch` crea un display nuevo.

**Plan de corrección (próxima sesión).**

- Serializar `launch` por package durante todo el spawn (lock por package o
  placeholder en el mapa) para eliminar la carrera TOCTOU.
- Antes de crear el display, ejecutar `adb shell input keyevent KEYCODE_WAKEUP`
  y, en el emulador, fijar `svc power stayon true` +
  `settings put system screen_off_timeout 2147483647` al arrancar el runtime.
- Evaluar `am force-stop <package>` antes de `--start-app` para garantizar un
  arranque limpio (coste: se pierde el estado de navegación de la app).
- Añadir una comprobación de “primer frame” o un timeout de sesión y relanzar
  automáticamente si no llega vídeo.

**Referencias.** `crates/apkstart-daemon/src/manager.rs` (`launch`),
`crates/apkstart-engine/src/scrcpy.rs`, scrcpy
[#6787](https://github.com/Genymobile/scrcpy/issues/6787),
[#6974](https://github.com/Genymobile/scrcpy/issues/6974),
[#7019](https://github.com/Genymobile/scrcpy/pull/7019).

---

## 2. Bugs resueltos

### “Ya se abrió” y la ventana no se redespliega

- **Causa:** el mapa de sesiones conservaba el `Child`/PID de scrcpy aunque el
  proceso hubiera muerto al cerrar la ventana; `Child::id()` seguía devolviendo
  el PID antiguo.
- **Fix:** `launch` valida `/proc/<pid>/cmdline` (scrcpy + serial) antes de
  reutilizar; `stop_app` no mata PIDs muertos; limpiador cada 5 s.
- **Commit:** `71b2560`. Verificado con launch → cerrar → launch.

### Los APK desaparecen al reiniciar el emulador

- **Causa doble:** `disk.dataPartition.path=<temp>` en el AVD y arranque con
  `-no-snapshot-load`, que forzaba cold boot.
- **Fix:** AVD persistente (`disk.dataPartition.size=6G`,
  `userdata.useQcow2=yes`), arranque normal con quickboot y
  `APKSTART_COLD_BOOT=1` solo para recuperación. `doctor` avisa del AVD temporal.

### ADB no leía `sys.boot_completed`

- **Causa:** el cliente ADB ejecutaba el comando sin el subcomando `shell`.
- **Fix:** corregido en `apkstart-engine/src/adb.rs`.

### scrcpy no encontraba `adb`

- **Causa:** scrcpy invoca `adb` por nombre y el SDK no estaba en el `PATH`.
- **Fix:** el motor antepone el directorio de `platform-tools` al `PATH` del
  proceso scrcpy.

### La ventana se abría sin identidad propia

- **Causa:** SDL3 usaba la clase `scrcpy` para todas las apps.
- **Fix:** `SDL_APP_ID=io.github.apkstart.<package>`; el `.desktop` y
  `StartupWMClass` usan el mismo id.

### UI de ajustes: marca del sidebar con un hueco enorme

- **Causa:** el `GtkImage` interno tenía `hexpand/vexpand`, y GTK propagaba el
  expand al `GtkBox` de la marca; el sidebar le daba todo el espacio sobrante.
- **Fix:** `brand_mark` usa `GtkCenterBox` con un icono de tamaño fijo.
- **Commit:** `9f08b0b`.

### UI de ajustes: tarjetas que no reflotaban

- **Causa:** los labels largos de las tarjetas aumentaban su ancho natural y el
  `GtkFlowBox` solo podía colocar una por línea.
- **Fix:** `set_max_width_chars` en los labels de `info_card`/`check_card`.
- **Commit:** incluido en `9f08b0b`.

### `FileChooserNative` deprecado

- **Causa:** al activar `gtk_v4_10`, GTK marca la API antigua como deprecada.
- **Fix:** la UI usa `GtkFileDialog` con `gio::ListStore` de filtros.

---

## 3. Limitaciones parciales

| Área | Estado | Detalle |
|---|---|---|
| Foco/posición multi-monitor | **Parcial** | KWin decide la posición en Wayland; `--window-x/y` se ignoran. La ventana puede abrirse en el otro monitor. Falta activation token o regla KWin. |
| Iconos adaptativos | **Parcial** | Se usa el bitmap si existe; si solo hay adaptive vectorial, se genera un PNG con la inicial. Falta renderizar el adaptive real y elegir la densidad máxima. |
| Red | **Parcial** | NAT de slirp; LAN/mDNS/Chromecast no transparentes. Proxy/DNS/puertos avanzados, Fase 4. |
| Periféricos | **Parcial** | Audio y portapapeles funcionan; cámara, micrófono y gamepad pendientes de validación. |
| Persistencia de ventana | **Parcial** | Tabla `window_state` existe, pero el visor es scrcpy y no se aplica tamaño/posición al reabrir. |
| Activity principal | **Parcial** | `apk-info` devuelve la primera activity de launcher (F-Droid → `IconActivity`); `--start-app` funciona, pero la metadata no siempre es la principal. |
| Sesiones tras reiniciar el daemon | **Parcial** | Si `apkstartd` muere, los scrcpy hijos sobreviven pero el daemon los pierde; falta tabla `sessions` v3 y adopción. |

---

## 4. Limitaciones asumidas del MVP

| Limitación | Motivo | Consecuencia |
|---|---|---|
| Motor Android Emulator | Estabilidad y madurez; QEMU puro/Waydroid son experimentales | Requiere KVM, SDK e imagen de Google. |
| Imagen `google_apis` (no Play) | La imagen Play no es redistribuible | Sin Play Store; F-Droid y APK laterales sí. |
| Solo x86_64 | La imagen oficial x86_64 es la vía estable | Apps con binarios ARM-only no arrancan; traducción ARM pendiente. |
| `FLAG_SECURE` / DRM / anticheat | Límite del emulador y de scrcpy | Apps de banca, Netflix o juegos con anticheat pueden fallar. |
| GPU global, no por proceso | El kernel no atribuye uso de GPU a un PID | La UI muestra el agregado de sysfs. |
| NAT de slirp | Evita privilegios y configuración de red | Sin bridge, LAN/mDNS/Chromecast limitados. |
| Un solo AVD/emulador | Simplicidad del MVP | No hay aislamiento por app ni multi-instancia. |
| scrcpy como visor | Reutiliza una base madura | Posición/tamaño dependen del compositor; visor propio en Fase 3. |
| Sin Electron/Node/Java en el host | Requisito de producto | Más trabajo de integración nativa. |

---

## 5. Riesgos técnicos vigilados

| Riesgo | Impacto | Mitigación actual |
|---|---|---|
| RAM insuficiente en el host | El emulador mata el guest o usa swap | Guest de 3 GB; `doctor`; cerrar apps; 12 GB recomendados. |
| Snapshot quickboot corrupto | El runtime no arranca | `APKSTART_COLD_BOOT=1`; borrar snapshots. |
| `sudo` con contraseña | La instalación automation falla | Instalación local en `~/Android/Sdk`; paquetes cuando el usuario autorice. |
| ADB compartido con el usuario | Conflictos con dispositivos físicos | Servidor propio en `:5038`. |
| Android 15+ endurece displays virtuales | Puede romper `--new-display` | Fijar API 34; seguir upstream de scrcpy. |
| Input sintético en KDE Wayland | Automatización limitada | Validación manual; XWayland/uinput como alternativas. |

---

## 6. Cómo reportar

Incluye siempre:

```bash
apkstart doctor > doctor.txt
apkstart export-log
journalctl --user -u apkstartd -n 200 --no-pager
```

y describe: distro, sesión (Wayland/X11), GPU, versión de scrcpy, package de la
app y si la ventana estaba negra, vacía o mostraba un error. Los logs de sesión
están en `~/.local/state/apkstart/sessions/<package>.log`.
