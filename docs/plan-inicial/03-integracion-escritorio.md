# 03 — Integración con el escritorio Linux

## 1. Principio

El usuario no debe entrar a los ajustes de Android ni ver un escritorio Android. Todo lo que la app necesita del sistema se configura desde Linux y se traduce al guest mediante ADB, QEMU o el bridge.

```text
Ajustes Linux (GTK)  →  apkstartd  →  ADB / QEMU / bridge  →  Android
```

---

## 2. Red

### 2.1 Modelo por defecto: NAT del host

- QEMU/emulator usa `-netdev user` (slirp) o el NAT del emulador.
- Android obtiene una IP tipo `10.0.2.15`; el host es `10.0.2.2`.
- El tráfico sale por los sockets del host, por lo que **la VPN, el firewall, el proxy y las rutas del host se aplican**.
- No requiere root ni crear interfaces virtuales.
- Es el modo recomendado para el usuario normal y el único que funciona sin permisos especiales.

### 2.2 DNS

- Usar el DNS del host (`/etc/resolv.conf` o systemd-resolved).
- Si el host usa DNS por VPN, el guest hereda la resolución porque las consultas salen por el host.
- Configurar `-netdev user,dns=...` o el DNS del emulador.
- Ajuste “usar DNS del sistema” (por defecto) y “DNS manual”.
- Evitar DNS públicos hardcodeados sin opción.

### 2.3 Proxy

- Leer `http_proxy`, `https_proxy`, `no_proxy` del entorno del usuario.
- Aplicar proxy global de Android:
  ```bash
  adb shell settings put global http_proxy host:puerto
  ```
- Para apps que usan su propio stack, documentar que el proxy de Android puede no aplicar.
- Proxy autenticado: no guardar contraseñas en SQLite; usar Secret Service.
- CA corporativa: ofrecer instalar un certificado de usuario en Android; no hacerlo automáticamente sin consentimiento.

### 2.4 VPN

- No crear una VPN dentro de Android por defecto.
- El tráfico del guest sale por el host; si el host está en VPN, el guest también.
- Si la VPN del host usa split tunneling, el guest respeta las rutas del host.
- Para apps que exigen “Wi-Fi” o “red móvil”, el guest ve una interfaz virtual; documentar que no es una red móvil real.

### 2.5 LAN, mDNS y dispositivos

- Slirp permite conexiones salientes a IPs de la LAN a través del host, pero **no** es transparente para broadcast/mDNS/UPnP.
- Chromecast, DLNA, impresoras locales y descubrimiento mDNS pueden no funcionar.
- Opciones avanzadas (fase 2):
  - `tap`/`bridge`/`macvtap` con NetworkManager y polkit;
  - `passt` u otro user-mode networking si se valida;
  - reflector mDNS/avahi.
- Requieren permisos elevados; no ser el modo por defecto.
- UI: “Red del sistema (recomendada)” vs “Red local avanzada (requiere permisos)”.

### 2.6 Puertos

- Reenvío de puertos host → guest para servidores locales:
  ```bash
  -netdev user,hostfwd=tcp:127.0.0.1:8080-:8080
  ```
- UI para añadir/eliminar reglas.
- No abrir puertos a `0.0.0.0` por defecto.

### 2.7 Estado y diagnóstico

- Mostrar IP del guest, DNS, proxy, VPN detectada, conectividad.
- Botón “probar red” con `ping`/HTTP a un endpoint del host.
- No mostrar los ajustes Wi-Fi de Android.

---

## 3. Audio

### 3.1 Salida

- QEMU/emulator con backend PipeWire (compatibilidad PulseAudio) o PulseAudio directo.
- El audio de todas las apps se mezcla en una sola salida del guest; no hay volumen por app sin trabajo extra.
- El host controla:
  - dispositivo de salida (PipeWire/WirePlumber);
  - volumen del stream del emulador;
  - mute.
- Android controla el volumen multimedia global; la UI de ajustes puede exponerlo.

### 3.2 Micrófono

- QEMU audio input o emulador con micrófono.
- Permiso vía xdg-desktop-portal/PipeWire.
- UI: dispositivo de entrada, nivel, mute.
- Probar en apps de voz; documentar latencia.

### 3.3 Sincronización A/V

- scrcpy puede reenviar audio con el vídeo; para apps de vídeo, usar el audio de scrcpy para sincronía.
- Para el resto, audio del emulador.
- Evitar reproducir dos veces el mismo audio (scrcpy + QEMU).

---

## 4. Cámara

- Opción 1: cámara virtual del emulador (virtual scene) para pruebas.
- Opción 2: webcam del host:
  - emulador: `-camera-back webcam0` si la versión lo soporta;
  - QEMU: `v4l2`/`usb-host` con permisos;
  - portal de cámara si se usa un pipeline PipeWire.
- UI: ninguna cámara, cámara virtual, webcam del host.
- Resolución/fps configurables.
- No garantizar cámara en apps con DRM/seguridad.

---

## 5. Gamepad y entrada

- scrcpy soporta gamepad (UHID); usarlo si el backend lo permite.
- Alternativa: leer `/dev/input/event*` con `evdev` y traducir a eventos Android vía scrcpy control o bridge.
- Mapeo:
  - sticks → ejes;
  - gatillos → botones;
  - dpad;
  - vibración (si el gamepad la soporta).
- UI: probar botones, calibrar, deadzone.
- Teclado:
  - layout del host → eventos Android;
  - IME chino/otros: soportar composición; coordinar con `InputConnection` del guest.
- Ratón:
  - clic izquierdo = toque;
  - rueda = scroll;
  - botón derecho = atrás (configurable);
  - botón medio = home (configurable);
  - arrastrar = swipe.
- Táctil:
  - gestos multi-touch en pantallas táctiles del host (si aplica);
  - emulación de pinch con teclado/ratón opcional.

---

## 6. Portapapeles

- scrcpy sincroniza portapapeles por defecto; se puede desactivar.
- Evitar bucles:
  - hash del contenido + origen;
  - no reenviar si el contenido ya está en el otro lado.
- UI:
  - “compartir portapapeles” on/off;
  - “limpiar al cerrar app” opcional.
- Privacidad: no sincronizar automáticamente contraseñas; respetar el toggle.
- Formatos: texto y URLs en MVP; imágenes en fase 2.

---

## 7. Notificaciones

### 7.1 Problema

Android no expone notificaciones al host por sí solo. Hay que instalar un listener.

### 7.2 Solución: `apkstart-bridge`

- APK Kotlin con `NotificationListenerService`.
- Se instala una vez en la imagen del usuario.
- Se concede el permiso de listener:
  ```bash
  adb shell cmd notification allow_listener io.github.apkstart.bridge/.NotificationListener
  ```
  (el comando exacto depende de la versión; validar).
- El bridge envía por ADB forward (socket TCP local) o por `am broadcast` + archivo:
  ```json
  {
    "id": 123,
    "package": "com.example.app",
    "title": "...",
    "text": "...",
    "icon": "base64...",
    "actions": [{"id": "reply", "title": "Responder"}],
    "timestamp": 1730000000
  }
  ```
- El daemon publica en D-Bus `org.freedesktop.Notifications`.
- Click en la notificación:
  - abre la ventana de la app;
  - o ejecuta la acción Android correspondiente.
- Agrupación por app; icono de la app.
- Si el bridge no está o no tiene permiso: no mostrar notificaciones; no bloquear apps.

### 7.3 Alternativas

- `dumpsys notification` por polling: frágil, sin acciones, no recomendado.
- `scrcpy` no reenvía notificaciones.
- Bridge es la opción correcta.

---

## 8. Archivos y carpeta compartida

### 8.1 Objetivo

Que la app pueda leer/escribir archivos del host sin que el usuario vea el almacenamiento Android.

### 8.2 Implementación MVP

- Carpeta del host: `~/ApkStartShared` (configurable).
- Carpeta del guest: `/sdcard/ApkStartShared`.
- Sincronización bidireccional con:
  - `inotify` en el host;
  - `FileObserver` en el bridge o polling en el guest;
  - hash SHA-256 + tamaño + mtime.
- No usar el sync ingenuo de ApkStart (sin borrados y con pérdida de cambios del mismo tamaño).

### 8.3 Semántica

- **Subida**: host → guest; si el archivo cambió en ambos lados, crear copia de conflicto:
  ```
  foto.jpg
  foto.conflict-2026-11-27-120000-host.jpg
  foto.conflict-2026-11-27-120000-guest.jpg
  ```
- **Borrado**: propagar con tombstones; no resucitar archivos.
- **Renombrado**: detectar inode/mtime; si no, tratar como borrado + alta.
- **Archivos grandes**: límite configurable; no bloquear el daemon; streaming.
- **Cola**: persistente, reintentos, estado visible en ajustes.
- **Conflictos**: nunca sobrescribir silenciosamente; avisar.

### 8.4 “Abrir con” del host

- El usuario abre un archivo en Linux y elige “Abrir con app Android”.
- El daemon copia el archivo a la carpeta compartida y envía un intent `ACTION_VIEW` al guest:
  ```bash
  adb shell am start -a android.intent.action.VIEW \
    -d "file:///sdcard/ApkStartShared/archivo.pdf" \
    -t "application/pdf"
  ```
- Para content:// y permisos, usar el bridge/FileProvider.
- Fase 2: DocumentsProvider para que las apps vean el host como almacenamiento.

### 8.5 “Compartir” de Android al host

- El bridge intercepta `ACTION_SEND` y copia el archivo a la carpeta compartida; el host notifica “Archivo recibido”.
- Alternativa: el usuario usa la carpeta compartida manualmente.

---

## 9. URLs y enlaces

- Las apps Android abren enlaces dentro del guest (Chrome Android, etc.).
- Para abrir en el navegador del host:
  - bridge intercepta `ACTION_VIEW` http/https y pide al host `xdg-open`.
  - Requiere que la app use el intent estándar; muchas lo hacen.
- UI: “Abrir enlaces en el navegador del host” on/off.
- Para OAuth/login, a veces se necesita el navegador Android; ofrecer elección.

---

## 10. USB y periféricos avanzados

- QEMU `usb-host` para pasar un dispositivo USB concreto.
- Requiere reglas udev/polkit y permiso del usuario.
- Riesgos: el host pierde el dispositivo mientras está pasado.
- UI: lista de dispositivos, conectar/desconectar.
- No es MVP; documentar.

---

## 11. Ajustes del sistema compartidos

| Ajuste del host | Cómo se aplica en Android |
|---|---|
| Zona horaria | `adb shell setprop persist.sys.timezone <tz>` o `cmd alarm` |
| Idioma/locale | `-change-locale` del emulador o `am`/`LocaleManager` |
| Escala de fuente | `settings put system font_scale` |
| Tema claro/oscuro | `cmd uimode night yes/no` |
| Rotación | `settings put system accelerometer_rotation` + orientación por app |
| Brillo | No aplica (el display virtual no tiene brillo físico) |
| Batería | Emulada; mostrar estado del host si se puede |
| GPS | `geo fix` del emulador o `geoclue` → bridge |
| Sensores | Emulados; no reales |
| Portapapeles | scrcpy/bridge |
| Notificaciones | bridge |
| Impresión | No soportado en MVP; Android Print Spooler → CUPS es complejo |

---

## 12. Sin ajustes Android visibles

- No se abre `com.android.settings` en el flujo normal.
- Ajustes Linux cubren lo que el usuario necesita.
- “Modo avanzado” opcional:
  - abrir Android Settings en una ventana (display virtual);
  - solo para diagnóstico, con advertencia.
- Permisos de apps:
  - se muestran en los ajustes de la app;
  - se pueden conceder/revocar vía ADB;
  - por defecto, no auto-conceder todo.
- Play Store:
  - se puede abrir como una app más si el usuario lo pide;
  - no es la vía principal de instalación.

---

## 13. Comparación con WSA

| Área | WSA | Este proyecto |
|---|---|---|
| Integración de ventanas | Ventanas Windows nativas | Ventanas Wayland vía streaming de display virtual |
| Notificaciones | Integradas | Bridge APK + D-Bus |
| Portapapeles | Integrado | scrcpy/bridge |
| Archivos | Integración Windows | Carpeta compartida + intents |
| Red | Compartida con Windows | NAT del host; VPN/proxy heredados; LAN limitada |
| Cámara/micrófono | Integrados | Portal/PipeWire + emulador |
| Google Play | No oficial | Imagen Play opcional |
| Rendimiento | Mejor integración | VM completa; más pesada |
| DRM/anticheat | Limitado | Limitado/negativo |
| Escritorio Android | Oculto | Oculto (headless + display virtual) |

La meta es **parecerse en uso**, no copiar la implementación de WSA.
