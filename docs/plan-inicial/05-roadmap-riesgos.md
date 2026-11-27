# 05 — Roadmap, pruebas y riesgos

## 1. Enfoque de entrega

El proyecto se divide en una **fase de viabilidad** que puede matar el diseño antes de invertir meses, y luego un MVP incremental.

```text
Fase 0  →  Fase 1  →  Fase 2  →  Fase 3  →  Fase 4 (opcional)
spike      MVP        integración  pulido     backends alternativos
2–3 sem.   6–8 sem.   6–8 sem.     4–6 sem.   continuo
```

Estimación realista:

- **Equipo de 2–3 personas**: 5–7 meses hasta una versión usable.
- **Una persona**: 9–12 meses para algo pulido.
- **Solo spike**: 2–3 semanas.
- No es un proyecto de fin de semana; el display virtual + viewer + integración es donde está el trabajo.

---

## 2. Fase 0 — Spike de viabilidad

### Objetivo

Demostrar que Android completo en QEMU puede dar ventanas por app en Wayland sin mostrar el escritorio Android.

### Entregables

- Script reproducible para:
  - arrancar Android Emulator headless con KVM;
  - crear display virtual con scrcpy;
  - lanzar una app;
  - redimensionar;
  - input/audio/portapapeles.
- Medición de latencia y CPU/GPU.
- Prueba en Hyprland, GNOME y KDE.
- Documento de resultados y decisión GO/NO-GO.

### Criterios GO

- App real visible y usable en ventana Wayland.
- Sin escritorio Android visible.
- `--flex-display` funciona o hay plan B.
- Input de teclado/ratón/scroll.
- Audio y portapapeles.
- Latencia < 80 ms percibidos.
- 3 compositores funcionando.
- Limitaciones documentadas (DRM, audio por app, etc.).

### Criterios NO-GO / pivote

- El display virtual no funciona con la imagen objetivo.
- No se puede mantener una latencia aceptable.
- El input es inestable.
- El coste de mantener un viewer propio es desproporcionado.
- En ese caso: Waydroid (contenedor) o una sola ventana de emulador; replantear requisitos.

---

## 3. Fase 1 — MVP

### Alcance

- Daemon Rust + CLI.
- Ajustes GTK4.
- Parser APK (ZIP/AXML/ARSC/iconos).
- Instalación simple y splits.
- `.desktop` + iconos + MIME.
- Runtime Android Emulator headless.
- Una app por ventana (scrcpy wrapper o viewer mínimo).
- Tamaño por app.
- Red NAT + DNS/proxy básico.
- Audio + portapapeles.
- Carpeta compartida básica.
- Diagnóstico.

### Entregables

- `apkstartd`, `apkstart`, `apkstart-settings`.
- Paquete AppImage o deb para pruebas.
- Documentación de instalación.
- Tests unitarios y de integración básicos.

### Criterios de aceptación

- Instalar un APK desde el gestor de archivos.
- Aparece icono y `.desktop` correctos.
- Abrir la app desde el menú del compositor.
- Ventana con título/icono; sin escritorio Android.
- Redimensionar y cerrar.
- Reabrir y conservar tamaño.
- Desinstalar limpia.
- Funciona en Hyprland, GNOME y KDE.
- No requiere root.

---

## 4. Fase 2 — Integración

### Alcance

- Multi-app con límite.
- Notificaciones bridge.
- Cámara/micrófono/gamepad.
- Red avanzada (proxy, VPN, puertos, LAN).
- Snapshot/suspensión/recuperación.
- Diagnóstico completo.
- Actualizaciones firmadas.

### Criterios de aceptación

- Dos apps abiertas a la vez sin bloquear el host.
- Notificaciones Android aparecen en el escritorio y abren la app.
- Portapapeles bidireccional estable.
- Micrófono/cámara funcionan en una app de prueba.
- Gamepad funciona en un juego de prueba.
- Proxy/VPN del host se reflejan en el guest.
- Recuperación tras matar el runtime.
- Actualización verificada.

---

## 5. Fase 3 — Pulido

### Alcance

- Empaquetado Flatpak/AppImage/deb/rpm/Arch.
- Accesibilidad, i18n, temas.
- Documentación de usuario.
- Rendimiento y consumo.
- Iconos VectorDrawable.
- Multi-monitor, tiling WMs.

### Criterios de aceptación

- Instalación en Ubuntu/Fedora/Arch sin pasos manuales.
- Accesibilidad básica.
- Documentación de limitaciones.
- Sin regresiones en compositores.

---

## 6. Fase 4 — Backends alternativos

- QEMU puro + Android-x86/LineageOS.
- Waydroid opcional.
- Imágenes AOSP sin GMS.
- Modo “una sola ventana” para GPUs/compositores problemáticos.
- Posible integración con `passt`/red local avanzada.

---

## 7. Plan de pruebas

### 7.1 Unitarias

- ZIP: stored/deflate, ZIP64, data descriptors, zip-slip.
- AXML: string pools UTF-8/UTF-16, namespaces, atributos tipados.
- ARSC: configs, densidades, sparse/offset16, referencias.
- APK: package, label, versión, activity, ABI, orientación.
- Iconos: PNG/WebP/adaptive/vector; tamaños; fallback.
- DB: migraciones, window_state, conflictos.
- Red: parseo de proxy/DNS, reglas de puertos.
- Sync: tombstones, conflictos, mismo tamaño.

### 7.2 Integración

- Emulador headless + KVM.
- Instalar APK real (F-Droid/NewPipe/Tusky).
- Abrir en display virtual.
- Captura de pantalla para verificar primer frame.
- Input: teclado, ratón, scroll, gamepad.
- Resize y persistencia.
- Cerrar/reabrir.
- Notificaciones, portapapeles, archivos.
- Multi-app.
- Suspender/reanudar.
- Matar runtime y recuperar.

CI:

- GitHub Actions Linux no siempre tiene `/dev/kvm`; usar runner self-hosted para integración.
- Unitarias en CI normal.
- Integración nightly con emulador.

### 7.3 Compositores

Matriz mínima:

| Compositor | Estado |
|---|---|
| Hyprland | Objetivo principal |
| Sway/wlroots | Objetivo |
| GNOME Mutter | Objetivo |
| KDE KWin | Objetivo |
| X11/XWayland | Best-effort |

Probar:

- `app_id` y reglas;
- tamaño inicial;
- resize;
- maximizar/fullscreen;
- multi-monitor;
- tiling vs floating.

### 7.4 Apps de prueba

- F-Droid.
- NewPipe.
- Tusky.
- AntennaPod.
- Firefox.
- VLC.
- Un juego 3D.
- Una app con `FLAG_SECURE` (esperado: no capturable).
- Una app ARM-only.
- Una app sin GMS.
- Una app de banca/anticheat (esperado: limitación).

### 7.5 Rendimiento

- Latencia input-to-photon.
- FPS estables.
- CPU/GPU/RAM.
- Tiempo de arranque frío/caliente.
- Consumo en idle.
- Tamaño de snapshot.

### 7.6 Seguridad

- Fuzz del parser APK.
- Zip-slip.
- APK malformado.
- Manifest con package inválido.
- Permisos.
- Actualización con hash/firma incorrectos.
- Logs sin secretos.

---

## 8. Riesgos

| Riesgo | Prob. | Impacto | Mitigación |
|---|---|---|---|
| Display virtual no funciona en la imagen/motor elegido | Media | Alto | Spike temprano; API 34; plan B Waydroid/ventana única |
| Latencia/calidad de streaming no convence | Media | Alto | VA-API, H.265, resolución adaptativa; medir; aceptar límite |
| `FLAG_SECURE`/DRM bloquea apps populares | Alta | Medio | Documentar; no prometer; modo alternativo |
| Anti-emulación/anticheat | Alta | Medio | Documentar; no soportar; no vender como universal |
| Imágenes Play/ARM con licencia restrictiva | Alta | Alto | No redistribuir; usuario acepta; alternativa AOSP |
| Android 15/16 endurece displays virtuales | Media | Alto | Fijar API 34; validar; AOSP userdebug |
| Falta de KVM en el host | Media | Alto | Diagnóstico; TCG solo demo; recomendar KVM |
| GPU/driver rompe `-gpu host` | Media | Medio | Fallback software; matriz de drivers |
| Audio por app no aislado | Alta | Bajo | Mezcla global; documentar |
| Notificaciones requieren bridge/permiso | Media | Medio | Bridge desde fase 2; fallback silencioso |
| Carpeta compartida pierde datos | Media | Alto | Tombstones, conflictos, tests; no sobrescribir |
| Wayland ignora tamaño/posición | Alta | Bajo | Guardar tamaño; no posición; reglas |
| Tiling WM rompe UX | Media | Bajo | Reglas; documentar; no forzar |
| Flatpak bloquea KVM/ADB/USB | Media | Medio | Paquetes por distro primero; portals después |
| Viewer propio se vuelve enorme | Media | Alto | MVP con scrcpy; viewer incremental; medir |
| scrcpy cambia protocolo | Baja | Medio | Fijar versión; fork si es necesario; tests |
| QEMU puro no soporta displays virtuales | Alta | Medio | No prometer paridad; backend experimental |
| Consumo de RAM excesivo | Media | Medio | Límites, snapshots, cierre por inactividad |
| Competencia (Waydroid/emuladores) | Alta | Medio | Diferenciar: Android completo + GMS + integración nativa |

---

## 9. Licencias y legalidad

### Componentes

| Componente | Licencia | Nota |
|---|---|---|
| Rust crates | MIT/Apache mayormente | Auditar |
| GTK4/libadwaita | LGPL | Enlazado dinámico |
| GStreamer | LGPL | Plugins variables |
| QEMU | GPL-2.0 | Distribuir fuente si se distribuye binario |
| Android Emulator | Propietario Google | No redistribuir; descarga del usuario |
| Imagen Play | Propietaria Google | No redistribuir; términos |
| Traducción ARM Google | Propietaria | Solo desarrollo/depuración |
| scrcpy | Apache-2.0 | Atribución |
| Android bridge APK | MIT/Apache | Código propio |
| Imágenes AOSP/LineageOS | Apache/GPL varios | Revisar |

### Reglas

- No incluir imágenes ni binarios de Google en el repositorio.
- El instalador no debe descargar imágenes sin consentimiento.
- Mostrar licencia y enlace antes de descargar.
- Para uso comercial, consultar abogado; considerar AOSP/LineageOS.
- No prometer “Google Play” si el usuario no acepta licencias.

---

## 10. Alternativas y pivotes

### Waydroid

- Ventanas Wayland nativas, rendimiento alto, contenedor.
- No es Android completo aislado; depende del kernel del host.
- No cumple el requisito QEMU, pero puede ser el **modo rápido** para apps compatibles.
- Menos apps con GMS/DRM/anticheat.

### QEMU puro

- Más abierto, pero imágenes Android-x86/LineageOS antiguas, sin GMS/ARM, GPU limitada.
- Útil para usuarios técnicos; no para el usuario medio.

### Emulador de una sola ventana

- Mucho más simple: se muestra la ventana del emulador con la app en pantalla completa.
- Viola “sin escritorio Android” y “ventana por app”.
- Solo como fallback de accesibilidad.

### No hacer nada y usar Waydroid/emuladores existentes

- Si el objetivo es solo usar apps, Waydroid puede ser suficiente.
- Si el objetivo es Android completo + GMS + integración, este proyecto tiene sentido.

---

## 11. Go/No-Go

Antes de escribir el daemon completo, el spike debe responder:

1. ¿Funciona display virtual + app + input en Android 14 con KVM?
2. ¿Funciona resize flexible?
3. ¿Se ve bien en Hyprland, GNOME y KDE?
4. ¿La latencia es aceptable?
5. ¿El audio/portapapeles funcionan?
6. ¿El coste de mantener el viewer es asumible?
7. ¿Las licencias permiten el modelo de distribución?

Si la respuesta a 1, 2 o 3 es no, no continuar con la arquitectura de displays virtuales; pivotar.

---

## 12. Próximos pasos recomendados

1. Validar el spike en una máquina Linux con KVM.
2. Decidir viewer: scrcpy wrapper vs Rust propio.
3. Decidir imagen: Play vs AOSP.
4. Crear el esqueleto del workspace Rust.
5. Implementar el parser APK con tests reales.
6. Implementar el daemon mínimo y el primer display virtual.
7. Medir y ajustar.
8. Recién después, UI y integración.

---

## 13. Referencias

- scrcpy: <https://github.com/Genymobile/scrcpy>
- scrcpy virtual display: <https://github.com/Genymobile/scrcpy/blob/master/doc/virtual-display.md>
- Android Emulator: <https://developer.android.com/studio/run/emulator>
- QEMU: <https://www.qemu.org/>
- Wayland: <https://wayland.freedesktop.org/>
- PipeWire: <https://pipewire.org/>
- xdg-desktop-portal: <https://github.com/flatpak/xdg-desktop-portal>
