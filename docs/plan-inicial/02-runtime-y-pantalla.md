# 02 — Runtime Android, QEMU y pantalla

## 1. Decisión de motor

### 1.1 Android Emulator oficial (recomendado para el MVP)

| Aspecto | Detalle |
|---|---|
| Base | QEMU + Android Emulator de Google |
| Aceleración | KVM en Linux (`/dev/kvm`), fallback TCG lento |
| Imagen | Android 14 (API 34) x86_64, Play Store o AOSP |
| Ventajas | GMS/Play, buena compatibilidad x86, traducción ARM en imágenes Google, snapshots, GPU, audio, cámara, ADB maduro |
| Desventajas | Emulador propietario; imagen Play no redistribuible; traducción ARM limitada a desarrollo/depuración; no es 100 % open source |
| Rol | Backend principal del MVP |

Comando base (headless, sin ventana del emulador):

```bash
emulator -avd apkstart \
  -no-window \
  -no-boot-anim \
  -no-metrics \
  -crash-report-mode disabled \
  -gpu host \
  -accel on \
  -no-snapshot-load \
  -netdelay none -netspeed full \
  -no-audio   # solo si el audio se gestiona de otra forma; por defecto NO usar
```

Notas:

- `-no-window` es obligatorio para que no aparezca el escritorio Android.
- `-gpu host` con fallback a `swiftshader_indirect` si el driver falla.
- `-no-snapshot-load` en el primer arranque; después, snapshot para arranque rápido.
- `-no-audio` solo si se usa otro camino de audio; para el MVP dejar audio del emulador.
- No usar `-qt-hide-window` como única solución: puede impedir que se cree la ventana que algunos paths necesitan; con `-no-window` no aplica.
- ADB en puerto propio (por ejemplo 5038) para no chocar con el ADB del usuario; usar `ANDROID_ADB_SERVER_PORT`.

### 1.2 QEMU puro + Android-x86/LineageOS (fase 2)

| Aspecto | Detalle |
|---|---|
| Base | `qemu-system-x86_64` + imagen suministrada por el usuario |
| Ventajas | 100 % open source; control total de virtio; sin GMS/licencias Google |
| Desventajas | Android-x86/LineageOS suelen ser versiones antiguas; sin GMS; sin traducción ARM; drivers GPU limitados; soporte de displays virtuales incierto |
| Rol | Backend alternativo para usuarios avanzados; no prometer paridad |

Bosquejo:

```bash
qemu-system-x86_64 \
  -enable-kvm -machine q35 -cpu host -smp 4 -m 4096 \
  -drive file=android.img,if=virtio,format=qcow2 \
  -device virtio-vga-gl -display none \
  -device virtio-net-pci,netdev=n0 \
  -netdev user,id=n0,hostfwd=tcp:127.0.0.1:5555-:5555 \
  -audiodev pa,id=snd0 -device intel-hda -device hda-duplex,audiodev=snd0 \
  -device virtio-9p-pci,fsdev=fs0,mount_tag=hostshare \
  -fsdev local,id=fs0,path=/home/user/ApkStartShared,security_model=none
```

Problemas a validar:

- Que la imagen soporte `DisplayManager` y displays virtuales (necesario para ventanas por app).
- Que scrcpy pueda conectarse y crear displays virtuales sin root.
- Que la GPU virtual (virtio-gpu/Venus) funcione con apps reales.
- Que la imagen tenga ADB habilitado y red.
- Sin GMS, muchas apps no arrancan; documentarlo.

### 1.3 Waydroid (opcional futuro)

No es QEMU; es un contenedor con el kernel del host. Ofrece ventanas Wayland nativas y mejor rendimiento, pero no cumple el requisito de Android completo aislado. Se puede ofrecer como “modo rápido” para apps compatibles, nunca como sustituto silencioso del backend QEMU.

---

## 2. Imagen Android

### Requisitos

- Android 14 (API 34) recomendado para el MVP.
- x86_64.
- Play Store (para GMS) o AOSP (para evitar licencias).
- `adb` habilitado; sin diálogo de autorización en cada arranque (clave ADB preautorizada en la imagen del emulador).
- Soporte de múltiples displays / displays virtuales.
- Densidad y resolución configurables.

### Por qué API 34

- scrcpy puede crear displays virtuales sin root en Android 14.
- Android 15/16 pueden exigir `ADD_TRUSTED_DISPLAY` y `adb root` en algunas builds; las imágenes Play no son rootables.
- Si se quiere Android 15, usar una imagen AOSP `userdebug` suministrada por el usuario y validar el permiso; no asumirlo.

### Licencias

- No incluir la imagen en el repositorio ni en el instalador.
- Descarga oficial + aceptación de licencia por el usuario.
- La traducción ARM de Google solo está licenciada para desarrollo/depuración en máquinas propias; no para un servicio comercial.
- Para distribución comercial: imagen AOSP/LineageOS + GApps suministradas por el usuario, o Waydroid.

---

## 3. Pipeline de pantalla por app

### 3.1 Idea

```text
Android (headless)
  └── DisplayManager
        ├── display virtual 1 → app A
        ├── display virtual 2 → app B
        └── ...
              │
              ▼
        scrcpy server (por display)
              │  H.264/H.265/AV1 + control + audio
              ▼
        apkstartd (proxy local)
              │
              ▼
        apkstart-viewer (Wayland)
              ├── decodifica VA-API
              ├── renderiza en GtkGLArea
              ├── envía input al display
              └── guarda tamaño/estado
```

### 3.2 Crear el display y lanzar la app

Con scrcpy 5.x:

```bash
scrcpy \
  --new-display=900x1600/240 \
  --flex-display \
  --start-app=com.example.app \
  --no-vd-system-decorations \
  --keep-active \
  --video-codec=h265 \
  --max-fps=60 \
  --video-bit-rate=8M \
  --window-title="Nombre de la app" \
  --no-window   # si el viewer propio consume el stream
```

Notas:

- `--new-display=WxH/DPI` define el display virtual.
- `--flex-display` permite que el display se redimensione con la ventana (scrcpy 4.0+).
- `--start-app` lanza la app directamente; sin esto, el display puede mostrar el launcher de Android.
- `--no-vd-system-decorations` evita barras/toolbars de Android en el display virtual.
- `--keep-active` evita que el display se apague.
- `--no-window` se usa si el viewer propio consume el stream; en el MVP se puede dejar la ventana de scrcpy y envolverla.

### 3.3 Viewer propio vs scrcpy CLI

| Criterio | scrcpy CLI | Viewer Rust propio |
|---|---|---|
| Velocidad de MVP | Alta | Media |
| Ventana nativa | SDL3, xdg-shell | GTK4, xdg-shell |
| `app_id` único por app | No (todas son `scrcpy`) | Sí |
| Icono por app en taskbar | Limitado | Sí, vía `.desktop` + app_id |
| Guardar tamaño en resize | No estándar | Sí, `configure` event |
| Control de errores/UX | Limitado | Total |
| Mantenimiento | Depende del upstream | Propio |

**Recomendación:**

1. **Spike:** usar scrcpy CLI para validar que el display virtual, flex resize, input y audio funcionan con el emulador headless.
2. **MVP:** si el spike sale bien, usar scrcpy CLI envuelto y aceptar que el tamaño se guarda al abrir/cerrar (no en cada resize) o parchear scrcpy con una opción de geometría.
3. **Fase 2:** viewer Rust propio para `app_id`, icono, tamaño en vivo y mejor integración.

### 3.4 Protocolo scrcpy

- El servidor scrcpy se ejecuta en Android vía `app_process` y se comunica por ADB.
- scrcpy abre sockets de vídeo, audio y control.
- El host envía eventos de input (touch, key, scroll, clipboard) por el socket de control.
- El viewer propio necesitará implementar:
  - handshake y versionado;
  - lectura de frames;
  - decodificación;
  - envío de eventos;
  - control de display (resize/flex);
  - clipboard;
  - audio (opcional).
- scrcpy es Apache-2.0; se puede reutilizar su servidor y su documentación. Si se fork-ea el cliente, mantener atribución y compatibilidad.

### 3.5 Ventana y `app_id`

- Cada app debe tener un `app_id` Wayland único:
  - `io.github.apkstart.<paquete>` o `apkstart.<paquete>`.
- El `app_id` debe coincidir con el nombre del `.desktop` para que el compositor asocie icono y ventana:
  - `~/.local/share/applications/io.github.apkstart.com.example.app.desktop`.
- El `.desktop` define `Name`, `Icon`, `Exec=apkstart launch <pkg>`, `StartupWMClass` (para XWayland), `MimeType` si aplica.
- Título de ventana: nombre de la app; opcional sufijo “— ApkStart” solo en ajustes, no en la app.
- Icono: PNG en `hicolor`; el compositor lo toma del `.desktop`.

### 3.6 Recordar tamaño

- El viewer escucha `configure`/`size-allocate` y reporta al daemon:
  - ancho, alto lógicos;
  - maximizado, fullscreen;
  - monitor/escala.
- Debounce de ~500 ms; escritura SQLite.
- Al abrir: el daemon pasa `width`/`height` al viewer; el viewer llama `gtk_window_set_default_size` y, si aplica, `maximize`.
- Wayland no permite posición absoluta; no se guarda `x/y`.
- En X11/XWayland se puede guardar `x/y` opcionalmente.
- En tiling WMs el compositor puede ignorar el tamaño; se documenta y se ofrecen reglas.

### 3.7 Redimensionado flexible

- Con `--flex-display`, Android recibe el nuevo tamaño del display y la app reflowea.
- Si la app no soporta resize (fixed orientation, games), el viewer puede:
  - mantener relación de aspecto y letterbox;
  - o reiniciar la app con el nuevo tamaño (peor UX; evitar).
- El viewer debe limitar tamaño mínimo (por ejemplo 320×480) y máximo (resolución del monitor).
- DPI: calcular a partir de la escala del monitor y del tamaño; permitir override por app.
- Orientación: portrait/landscape; al cambiar, recrear el display si Android no lo hace solo.

### 3.8 Sin escritorio Android

- El runtime arranca con `-no-window`.
- No se muestra el display principal.
- Cada app se lanza en su display virtual; si no hay app, el display no se muestra.
- Si la app abre el launcher (por ejemplo, botón Home), el viewer puede:
  - interceptar Home y volver a la app;
  - o mostrar un mensaje “La app salió a Android; ciérrala para volver”.
- No se ofrece un launcher Android en la UI principal.
- Ajustes de Android quedan en un “modo avanzado” opcional, no en el flujo normal.

---

## 4. Rendimiento y latencia

- scrcpy declara 35–70 ms de latencia y 30–120 fps; el objetivo del proyecto es **< 60 ms percibidos** y 60 fps estables en apps de UI.
- Decodificación por hardware:
  - VA-API (Intel/AMD), NVDEC (NVIDIA) vía GStreamer.
  - Fallback software (coste alto; avisar).
- Códecs:
  - H.264 por compatibilidad; H.265 para mejor calidad/bitrate; AV1 si el guest y el host lo soportan.
  - Bitrate adaptativo según tamaño/fps; no usar 4K por defecto.
- Resolución del display virtual:
  - Basada en el tamaño de ventana × escala, con límite (por ejemplo 1080p).
  - No renderizar 4K para una ventana de 800 px.
- FPS:
  - 60 para UI; 30 para apps de vídeo si el coste es alto; configurable.
- GPU:
  - `-gpu host` con Vulkan/OpenGL; fallback `swiftshader_indirect`.
  - En Linux, validar ANGLE/Vulkan con drivers Mesa/NVIDIA.
- CPU:
  - KVM obligatorio para uso real; TCG solo para diagnóstico.
  - vCPU 2–4 por defecto; no sobreasignar.
- Memoria:
  - 4 GB para Android + apps; prewarm opcional; snapshots.
- Almacenamiento:
  - Imagen + datos + snapshots; avisar del espacio; limpieza.
- Arranque:
  - Snapshot quickboot; si falla, cold boot.
  - No bloquear la UI; mostrar estado.

---

## 5. Limitaciones duras

1. **`FLAG_SECURE` / DRM**: la captura de pantalla sale negra. No se puede mostrar Netflix, banca con secure flag, etc. en una ventana scrcpy. Alternativas:
   - modo “ventana del emulador” (viola el requisito de no mostrar escritorio Android);
   - no soportar la app y explicarlo.
2. **Anti-emulación/anticheat**: muchas apps de banca, juegos online y DRM detectan emulador; no garantizar.
3. **Audio por app**: Android mezcla el audio; no hay volumen independiente por app sin trabajo extra. Se controla el volumen multimedia global de Android.
4. **Cámara/micrófono**: dependen del emulador y de PipeWire/portal; no todas las imágenes soportan webcam; validar.
5. **USB**: pasar dispositivos USB a QEMU requiere permisos/udev; no es MVP.
6. **Notificaciones**: requieren bridge y permiso de listener; algunas apps no las exponen.
7. **Portapapeles**: scrcpy lo soporta, pero hay que evitar bucles y respetar privacidad.
8. **Wayland**: no hay posición absoluta; el tamaño puede ser ignorado por tiling WMs.
9. **Múltiples displays**: algunas apps/marcas no soportan displays secundarios; scrcpy virtual display puede fallar.
10. **Android 15+**: permisos de display virtual más estrictos; API 34 es el objetivo.

---

## 6. Spike de viabilidad (obligatorio antes de implementar)

### Objetivo

Demostrar en 2–3 semanas que el modelo funciona en Linux con KVM, Hyprland/GNOME/KDE, y medir latencia/calidad.

### Pasos

1. Instalar Android Emulator en Linux + imagen API 34 Play x86_64.
2. Arrancar headless con KVM:
   ```bash
   emulator -avd test -no-window -gpu host -no-boot-anim
   ```
3. Verificar ADB y `scrcpy` contra el emulador.
4. Crear display virtual y abrir una app:
   ```bash
   scrcpy --new-display=900x1600/240 --flex-display --start-app=org.mozilla.firefox
   ```
5. Comprobar:
   - No aparece escritorio Android.
   - La app se ve y se controla.
   - Resize flexible funciona.
   - Audio funciona.
   - Portapapeles funciona.
   - Cerrar la ventana cierra/destruye el display.
6. Probar en Hyprland, GNOME (Mutter) y KDE (KWin).
7. Medir latencia (input-to-photon aproximada) y CPU/GPU.
8. Probar una app ARM-only y una app con `FLAG_SECURE`.
9. Documentar resultados y decidir:
   - GO: viewer propio o scrcpy wrapper.
   - NO-GO: pivote a Waydroid o a una sola ventana.

### Criterios de aceptación del spike

- App real visible y usable en ventana Wayland.
- Sin escritorio Android visible.
- Resize y tamaño inicial correctos.
- Input de teclado/ratón/scroll.
- Audio y portapapeles.
- Latencia aceptable (< 80 ms percibidos).
- Funciona en al menos 3 compositores.
- Se entiende y documenta la limitación de `FLAG_SECURE`.

---

## 7. Configuración del host

### KVM

```bash
ls -l /dev/kvm
# el usuario debe pertenecer al grupo kvm o tener ACL
```

Si no hay KVM:

- Avisar; el rendimiento será malo.
- No fingir que funciona igual.
- Ofrecer diagnóstico y enlaces.

### GPU

- Detectar vendor: Intel/AMD/NVIDIA.
- Probar `-gpu host`; fallback a `-gpu swiftshader_indirect`.
- No usar `-gpu guest` salvo diagnóstico.
- VA-API: comprobar `vainfo`; NVDEC: `nvidia-smi`/GStreamer.

### Audio

- PipeWire con compatibilidad PulseAudio.
- QEMU/emulator con backend PA.
- Si no hay PipeWire, PulseAudio; si no, ALSA con aviso.

### Wayland

- `XDG_SESSION_TYPE=wayland`; `WAYLAND_DISPLAY` presente.
- Si solo hay X11, el viewer puede usar XWayland; se soporta como best-effort.
- No usar layer-shell para ventanas de apps.

---

## 8. Referencias técnicas

- scrcpy: <https://github.com/Genymobile/scrcpy> (Apache-2.0), virtual display, flex display, audio, input, clipboard.
- Android Emulator: <https://developer.android.com/studio/run/emulator> (propietario).
- Android `DisplayManager` / virtual displays.
- QEMU: <https://www.qemu.org/> (GPL-2.0).
- Wayland `xdg-shell`.
- PipeWire / xdg-desktop-portal.
