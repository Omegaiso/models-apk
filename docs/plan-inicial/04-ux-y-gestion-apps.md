# 04 — UX y gestión de apps

## 1. Principios de experiencia

1. **La app Android se siente como una app Linux**: ventana normal, título, icono, tamaño recordado, menú del compositor.
2. **No hay escritorio Android**: nunca se muestra el launcher, la barra de estado Android ni los ajustes Android en el flujo normal.
3. **La única interfaz de administración es Ajustes**: instalar, desinstalar, permisos, tamaño, red y periféricos se gestionan ahí.
4. **El escritorio manda**: los `.desktop` abren apps; el menú del compositor las lista; la bandeja es opcional.
5. **Errores explicables**: si una app no puede abrirse, se dice por qué y qué hacer; nunca un spinner infinito.
6. **Sin magia**: cuando algo es limitado (DRM, anticheat, audio por app), se documenta en la UI.

---

## 2. Flujo de primera ejecución

Objetivo: dejar el runtime listo sin abrir un escritorio Android.

1. El usuario instala el paquete Linux y abre **ApkStart Ajustes**.
2. Wizard:
   - Comprobar KVM, GPU, VA-API, PipeWire, Wayland, ADB.
   - Elegir motor: Android Emulator (recomendado) o QEMU puro (avanzado).
   - Aceptar licencias de la imagen (Google/AOSP).
   - Descargar runtime/imagen con progreso y verificación SHA-256.
   - Configurar carpeta compartida (opt-in).
   - Configurar notificaciones/portapapeles (opt-in).
   - Probar: arrancar Android headless y abrir una app de prueba (Settings/Ajustes Android solo para el test interno, no visible al usuario).
3. Al terminar:
   - Estado “Runtime listo”.
   - Botón “Instalar APK”.
   - No se abre ninguna ventana Android.

### Requisitos del wizard

- Descarga reanudable, con espejo y fallback.
- No bloquear la UI; progreso en segundo plano.
- Diagnóstico si falla: KVM, permisos, GPU, disco, red.
- Opción “ya tengo una imagen” (ruta local) para usuarios avanzados.

---

## 3. Instalación de APK

### 3.1 Entradas

- Doble clic en `.apk` desde el gestor de archivos.
- Arrastrar un APK a la ventana de Ajustes.
- `apkstart install archivo.apk`.
- Asociación MIME:
  ```
  application/vnd.android.package-archive
  ```
- `.apks`/`.xapk`: instalar splits con `adb install-multiple`; guardar el bundle para futuras reparaciones.

### 3.2 Flujo

1. Validar que es un APK/ZIP legible.
2. Parsear offline:
   - package, label, versionName, versionCode;
   - minSdk/targetSdk;
   - activity de launcher;
   - icono (PNG/WebP/adaptive/vector);
   - ABIs;
   - orientación declarada;
   - firma (hash del certificado).
3. Mostrar confirmación breve:
   - nombre, icono, versión, permisos solicitados, ABI.
   - Advertencia si es ARM-only o si minSdk > imagen.
4. Instalar:
   - `adb install -r -t` (sin `-g` por defecto).
   - Si es split, `install-multiple`.
   - Si falla por ABI, ofrecer cambiar a imagen Play/ARM; no fingir que se reintenta.
5. Resolver activity real:
   ```bash
   adb shell cmd package resolve-activity --brief <pkg>
   ```
6. Extraer/convertir icono:
   - PNG/WebP → PNG en varios tamaños.
   - Adaptive icon: componer fondo + foreground si es posible.
   - VectorDrawable: renderizar subset o usar el PNG legacy; si no, placeholder.
7. Crear `.desktop`:
   ```
   [Desktop Entry]
   Type=Application
   Name=<label>
   Comment=<pkg> — aplicación Android
   Exec=apkstart launch <pkg>
   Icon=io.github.apkstart.<pkg>
   StartupWMClass=io.github.apkstart.<pkg>
   Categories=Utility;
   ```
8. Guardar icono en `hicolor`:
   ```
   ~/.local/share/icons/hicolor/16x16/apps/io.github.apkstart.<pkg>.png
   ... 24, 32, 48, 64, 128, 256
   ```
9. Actualizar caché de iconos y menú.
10. Notificar “Instalado” con acción “Abrir”.

### 3.3 Reinstalación/actualización

- Comparar versionCode.
- Si la firma cambia, avisar y pedir desinstalar (Android no permite actualizar con otra firma).
- Conservar datos por defecto.
- Actualizar icono/.desktop.
- No duplicar entradas.

### 3.4 Desinstalación

- Preguntar:
  - desinstalar app + datos;
  - conservar datos (si Android lo permite).
- Borrar `.desktop`, iconos y metadatos.
- Borrar copia APK si el usuario lo pide.
- No dejar ventanas huérfanas.

---

## 4. Ajustes: estructura de UI

Ventana única GTK4/libadwaita con navegación lateral.

```text
ApkStart
├─ Inicio
│  ├─ Estado del runtime
│  ├─ Apps recientes
│  └─ Acciones rápidas
├─ Aplicaciones
│  ├─ Lista/búsqueda
│  ├─ Instalar APK
│  └─ [detalle de app]
├─ Runtime
│  ├─ Motor / imagen
│  ├─ CPU, RAM, GPU
│  ├─ Snapshot / arranque
│  ├─ Almacenamiento
│  └─ Estado / reiniciar
├─ Pantalla
│  ├─ Escala / DPI
│  ├─ Tamaño por defecto
│  ├─ Orientación
│  └─ Códec / fps / calidad
├─ Red
│  ├─ NAT del sistema
│  ├─ DNS / proxy
│  ├─ VPN detectada
│  ├─ Puertos
│  └─ Red local avanzada
├─ Periféricos
│  ├─ Audio / micrófono
│  ├─ Cámara
│  ├─ Gamepad
│  ├─ Portapapeles
│  └─ USB
├─ Integración
│  ├─ Notificaciones
│  ├─ Carpeta compartida
│  ├─ Enlaces
│  ├─ Asociación .apk
│  └─ Inicio automático
├─ Diagnóstico
│  ├─ Comprobaciones
│  ├─ Logs
│  └─ Exportar informe
└─ Acerca de
```

No hay una página “Escritorio Android”.

---

## 5. Detalle de una app

Cada app instalada tiene una página:

- Icono, nombre, paquete, versión.
- Botones: Abrir, Detener, Desinstalar.
- **Tamaño de ventana**:
  - ancho × alto actual;
  - “Restablecer tamaño”;
  - “Usar tamaño por defecto”;
  - opción “recordar tamaño” (global y por app).
- **Orientación**:
  - automática (según app);
  - forzar vertical/horizontal.
- **DPI / escala**:
  - automático;
  - manual.
- **Pantalla**:
  - “Abrir en el monitor actual” / monitor preferido.
  - maximizado/fullscreen al abrir.
- **Permisos**:
  - lista de permisos solicitados;
  - conceder/revocar;
  - “Abrir permiso de Android” solo si no se puede con ADB.
- **Datos**:
  - tamaño;
  - borrar datos;
  - exportar/importar datos (fase 2).
- **Compatibilidad**:
  - ABI, minSdk, advertencias.
  - “Probar en modo seguro” (sin GMS/ARM).
- **Avanzado**:
  - actividad de lanzamiento;
  - argumentos de lanzamiento;
  - logcat de la app;
  - “Abrir ajustes Android” (escape hatch).

---

## 6. Tamaño de ventana por app

### Requisito

“Cada app recuerda el último tamaño utilizado.”

### Implementación

- El viewer reporta cada `configure` con debounce.
- Se guarda:
  - `width`, `height` lógicos;
  - `maximized`, `fullscreen`;
  - monitor/escala si se conoce.
- Al abrir:
  - el daemon pasa el tamaño al viewer;
  - el viewer fija `default_size`;
  - si estaba maximizado/fullscreen, lo solicita.
- En Wayland no se guarda posición; el compositor decide.
- En XWayland se puede guardar `x/y` opcionalmente.
- En tiling WMs el tamaño puede ser ignorado; se documenta y se ofrecen reglas.

### Casos especiales

- App con orientación fija: guardar ancho/alto en la orientación correcta; al restaurar, crear display con esa orientación.
- App que no soporta resize: mantener aspecto y letterbox; no deformar.
- Cambio de monitor con distinta escala: recalcular tamaño lógico; no guardar píxeles físicos.
- Multi-app: cada app tiene su propio estado.

---

## 7. Comportamiento de ventanas

### Apertura

- Título: nombre de la app.
- Icono: icono extraído.
- `app_id` único; el compositor lo usa para reglas/icono.
- Tamaño recordado; si no hay, tamaño por defecto (por ejemplo 900×1600 o 60% del monitor).
- No abrir en pantalla completa por defecto.
- Enfocar la ventana; no robar foco agresivamente.

### Redimensionado

- Ventana redimensionable; mínimo 320×480.
- `--flex-display` para que Android reflowe.
- Si la app no reflowea, letterbox.
- Guardar tamaño tras 500 ms de inactividad.

### Maximizar/fullscreen

- Soportar ambos.
- Guardar estado.
- En fullscreen, ocultar decoraciones; Escape para salir.
- No forzar fullscreen en apps de vídeo salvo preferencia.

### Cierre

- Cerrar ventana = cerrar app (por defecto) o enviar a segundo plano (ajuste).
- Si se cierra la última ventana, programar snapshot/cierre del runtime por inactividad.
- Si el usuario cierra el runtime, cerrar todas las ventanas con aviso.

### Multi-app

- Límite por defecto: 2–3 apps.
- Cada app en su display virtual.
- Si se supera el límite, ofrecer cerrar la menos usada.
- No prometer multitarea ilimitada.
- En el MVP, una app a la vez es aceptable si el resto está bien; el diseño debe permitir varias.

### Multi-monitor

- Abrir en el monitor donde está el cursor.
- Recordar monitor por app si el usuario lo fija.
- No usar posición absoluta en Wayland.
- Escala por monitor.

### Tiling WMs (Hyprland/Sway)

- Usar `xdg-shell` estándar; no layer-shell.
- Respetar las reglas del compositor.
- Documentar ejemplos:
  ```ini
  # Hyprland: forzar tamaño inicial para una app
  windowrulev2 = size 900 1600,class:^(io.github.apkstart.com.example.app)$
  windowrulev2 = float,class:^(io.github.apkstart.com.example.app)$
  ```
- No intentar secuestrar el layout.

---

## 8. Ciclo de vida y ajustes

| Acción | Comportamiento por defecto |
|---|---|
| Cerrar ventana | Detener app y destruir display |
| Minimizar | Mantener app viva; pausar render si el compositor lo permite |
| Cambiar de workspace | App sigue viva |
| Suspender el host | Pausar VM; al reanudar, reconectar viewers |
| Cerrar sesión | Snapshot y apagar runtime |
| Inactividad | Snapshot + apagar tras N minutos |
| Crash de app | Diálogo con reiniciar/limpiar datos/log |
| Crash del viewer | Cerrar sesión; app queda en segundo plano |
| Crash del runtime | Reintentar con backoff; avisar |

---

## 9. Diagnóstico y errores

- Estado del runtime en Inicio:
  - motor, imagen, KVM, GPU, ADB, memoria, disco.
- “Probar ahora”:
  - arrancar una app de prueba;
  - medir latencia;
  - verificar audio/portapapeles.
- Errores típicos:
  - KVM no disponible → instrucciones.
  - Imagen no encontrada → descargar/importar.
  - ABI no soportada → cambiar imagen.
  - App con `FLAG_SECURE` → explicar.
  - Anticheat/anti-emulación → explicar.
  - Disco lleno → liberar.
- Exportar informe `.tar.zst` con logs redactados.
- No enviar telemetría sin consentimiento.

---

## 10. Accesibilidad, idioma y temas

- GTK4/libadwaita: soporte de lector de pantalla, alto contraste, escala de texto.
- i18n: español, inglés, chino, etc.; no hardcodear textos.
- Tema claro/oscuro del sistema; no forzar.
- Iconos: hicolor; fallback si falta.
- Teclado: navegación completa; atajos configurables.
- No usar animaciones que bloqueen.

---

## 11. CLI y automatización

```bash
apkstart install app.apk
apkstart launch com.example.app
apkstart list --json
apkstart uninstall com.example.app --keep-data
apkstart doctor --json
apkstart size com.example.app 900x1600
apkstart size --reset com.example.app
```

La CLI habla con el daemon por D-Bus; no abre una segunda instancia del runtime.

---

## 12. Primer mockup textual

```text
┌──────────────────────────────────────────────┐
│ ApkStart                                     │
│                                              │
│  Runtime listo                               │
│  Android 14 · Emulator · KVM · 4 GB          │
│                                              │
│  [ Instalar APK ]   [ Abrir ajustes ]        │
│                                              │
│  Apps recientes                              │
│  ┌────────┐  ┌────────┐  ┌────────┐          │
│  │ NewPipe│  │ Tusky  │  │ F-Droid│          │
│  └────────┘  └────────┘  └────────┘          │
│                                              │
│  Red: NAT del sistema · VPN activa           │
│  Audio: PipeWire · Micrófono: apagado        │
└──────────────────────────────────────────────┘
```

La ventana de una app Android no tiene barra de Android: solo el marco del compositor y el contenido.
