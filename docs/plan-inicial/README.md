# ApkStart Linux — Plan técnico completo

> **Estado:** planificación. No hay código de implementación en este documento.
> **Objetivo:** construir un gestor nativo de aplicaciones Android para Linux, con Android completo dentro de QEMU, ventanas nativas por app en Wayland, sin mostrar el escritorio Android y con integración de red y periféricos con el sistema anfitrión.

---

## 1. Veredicto de viabilidad

**Es viable, pero hay una condición técnica que define todo el proyecto:**

Con Android completo en QEMU no existe una superficie gráfica nativa por aplicación que el compositor de Linux pueda usar directamente. Android renderiza a través de SurfaceFlinger y expone *displays*, no ventanas individuales de apps. Por tanto, para que cada app sea una ventana normal del escritorio Linux hay que:

1. Crear un **display virtual por app** dentro de Android.
2. Lanzar la app en ese display.
3. Capturar y codificar ese display (scrcpy).
4. Decodificar el vídeo en el host y mostrarlo en una **ventana Wayland** (`xdg-shell`).
5. Inyectar teclado, ratón, táctil, scroll, gamepad y portapapeles en ese display.

Eso no es una superficie nativa: es **streaming local de baja latencia**. Bien hecho se siente como una ventana nativa (scrcpy declara 35–70 ms de latencia y 30–120 fps), pero no es lo mismo que Waydroid, que sí crea ventanas Wayland nativas porque es un contenedor, no una VM completa.

**Consecuencia:**

- Si el requisito “Android completo en QEMU” es inflexible, el diseño correcto es **display virtual + scrcpy + ventana nativa del host**.
- Si se exige superficie 100 % nativa sin streaming, hay que abandonar QEMU y usar Waydroid/redroid (contenedor), lo cual contradice el requisito 2.
- Recomendación: aceptar el modelo de streaming local para el MVP y dejar Waydroid como backend opcional futuro para apps compatibles.

---

## 2. Requisitos y solución propuesta

| # | Requisito | Solución propuesta |
|---|---|---|
| 1 | Sin Electron; gestor nativo en Rust/C++ | Núcleo y UI en **Rust**; GTK4/libadwaita para ajustes; GStreamer/libav para vídeo; nada de Node/Electron en el host |
| 2 | Android completo sobre QEMU | **Android Emulator oficial** (QEMU-based) como backend principal; backend **QEMU puro + imagen Android-x86/LineageOS** como alternativa |
| 3 | Compatible con Wayland, Hyprland, GNOME, KDE, etc. | Ventanas **xdg-shell** estándar; `app_id` único por app; sin layer-shell para ventanas de apps; reglas de compositor documentadas |
| 4 | Experiencia tipo capa de compatibilidad: solo ajustes, sin escritorio Android, red/periféricos del host | Arranque headless, display virtual por app, sin launcher; ajustes en GTK; NAT del host; PipeWire; portales; notificaciones/portapapeles puenteados; carpeta compartida |
| 5 | Cada app recuerda el último tamaño | SQLite `window_state`; el viewer reporta `configure`; se restaura ancho/alto/estado; en Wayland no se puede restaurar posición absoluta |

---

## 3. Decisiones recomendadas

### Stack

- **Rust estable** para daemon, CLI, parser de APK, integración y viewers.
- **GTK4 + libadwaita** para la app de ajustes y, si se implementa, el viewer propio.
- **GStreamer** (o libavcodec + `gtk4paintablesink`) para decodificar el vídeo de scrcpy.
- **SQLite (WAL)** para apps, ajustes, sesiones y estados de ventana.
- **zbus** para D-Bus; **tokio** para el daemon; **systemd --user** para el servicio.
- **scrcpy 5.x** (Apache-2.0) como motor de display virtual, entrada, audio y portapapeles en la primera etapa.
- **Android bridge APK** propio (Kotlin) para notificaciones, portapapeles avanzado, archivos e intents.

### Motor Android

1. **Fase 1 (recomendado): Android Emulator oficial de Google.**
   - Es QEMU por dentro, con aceleración KVM, snapshots y GPU.
   - Imagen Android 14 (API 34) x86_64 con Play Store, suministrada por el usuario.
   - Mejor compatibilidad con GMS y con apps ARM-only.
   - Contras: componente propietario, licencia de imagen Play y traducción ARM limitada a desarrollo/depuración; no se puede redistribuir.
2. **Fase 2 (opcional): QEMU puro + imagen Android-x86/LineageOS.**
   - 100 % open source, sin GMS ni traducción ARM.
   - Compatibilidad de apps y GPU mucho menor; los displays virtuales pueden no funcionar igual.
3. **Backend futuro opcional: Waydroid.**
   - Contenedor, ventanas Wayland nativas y mucho más rápido, pero no es QEMU ni Android completo aislado. Útil como “modo rápido” para apps compatibles.

### Viewer

- **Fase 0 (spike):** usar el ejecutable `scrcpy` por app para validar display virtual, resize flexible, audio, input y latencia.
- **Fase 2:** decidir entre:
  - **fork de scrcpy** (C, Apache-2.0) con `app_id`/icono/geometría por app, o
  - **viewer propio en Rust** usando el protocolo de scrcpy.
- El viewer propio es más trabajo, pero da identidad de ventana, icono, tamaño persistente y integración fina sin parchear un proyecto externo.

### Imagen Android

- **Android 14 (API 34) x86_64** es el objetivo del MVP.
- Motivo: scrcpy puede crear displays virtuales sin root en Android 14; en Android 15 algunas builds exigen `ADD_TRUSTED_DISPLAY` y `adb root`, que no está disponible en imágenes Play.
- Imagen Play para compatibilidad; imagen AOSP/LineageOS para evitar licencias de Google y para usuarios que no quieran GMS.
- No incluir imágenes ni binarios de Google en el repositorio; descarga y aceptación por el usuario.

---

## 4. Arquitectura en una frase

```text
.desktop / CLI / archivo APK
        │
        ▼
   apkstartd (Rust, systemd --user, D-Bus)
        │
        ├── Motor: Android Emulator / QEMU (headless)
        ├── ADB + scrcpy server: display virtual por app
        ├── apkstart-viewer: ventana Wayland + vídeo + input + tamaño
        ├── apkstart-settings: GTK4, única interfaz de administración
        ├── apkstart-bridge (APK Kotlin): notificaciones, portapapeles, archivos
        └── SQLite: apps, ajustes, sesiones, window_state
```

---

## 5. Alcance

### MVP

- Instalar APK desde el gestor de archivos (MIME `application/vnd.android.package-archive`) o CLI.
- Parsear APK offline: paquete, nombre, versión, activity, icono, ABI, orientación.
- Instalar con `adb install` / `install-multiple`.
- Crear `.desktop`, icono en `hicolor` y entrada en el menú de aplicaciones.
- Arrancar Android headless y lanzar cada app en su propio display virtual.
- Una ventana Wayland por app, con título e icono correctos.
- Recordar tamaño y estado de ventana por app.
- Sin escritorio Android visible.
- Red por NAT del host; audio; portapapeles; carpeta compartida.
- Ajustes GTK: runtime, apps, red, periféricos, integración, diagnóstico.
- Funcionar en Hyprland, GNOME y KDE con `xdg-shell`.

### Fase 2

- Multi-app simultáneo con límite configurable.
- Notificaciones Android → D-Bus.
- Cámara, micrófono, gamepad, USB.
- Proxy/VPN, DNS, port forwarding, LAN.
- Snapshots, suspensión/reanudación, recuperación de fallos.
- Actualizaciones firmadas y verificación de imágenes.

### Fase 3

- Backend QEMU puro.
- Backend Waydroid opcional.
- Empaquetado Flatpak/AppImage/deb/rpm/Arch.
- Accesibilidad, i18n, temas, documentación.
- Renderizado de iconos VectorDrawable más completo.

---

## 6. Documentos del plan

- [01 — Arquitectura](01-arquitectura.md)
- [02 — Runtime Android, QEMU y pantalla](02-runtime-y-pantalla.md)
- [03 — Integración con el escritorio](03-integracion-escritorio.md)
- [04 — UX y gestión de apps](04-ux-y-gestion-apps.md)
- [05 — Roadmap, pruebas y riesgos](05-roadmap-riesgos.md)

---

## 7. Decisiones abiertas

1. **Motor inicial:** Android Emulator oficial (recomendado) o QEMU puro desde el día uno.
2. **Viewer:** fork de scrcpy o viewer Rust propio.
3. **Imagen:** Play Store (más compatibilidad, licencias Google) o AOSP/LineageOS (más abierto, menos apps).
4. **Notificaciones:** bridge APK propio desde el MVP o dejar notificaciones solo dentro de la app.
5. **Multi-app:** límite por defecto (recomendado 2–3) y política de cierre.
6. **Empaquetado:** Flatpak con permisos de KVM/ADB o paquetes por distro.
7. **X11:** soportar XWayland desde el inicio o solo Wayland nativo.

---

## 8. Advertencia legal

- Google Play Images y la traducción ARM de Google **no se pueden redistribuir** y están limitadas a desarrollo/depuración en máquinas propias.
- QEMU es GPL-2.0; scrcpy es Apache-2.0; GTK es LGPL; Android Emulator es propietario.
- Si el producto se distribuye comercialmente, hay que revisar licencias y probablemente usar imágenes AOSP/LineageOS suministradas por el usuario, o Waydroid.
- Las apps con DRM, `FLAG_SECURE`, banca, anticheat o detección de emulador no son compatibles de forma fiable; hay que documentarlo desde el primer día.
