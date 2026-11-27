# Dependencias

## 1. Cadena de compilación

| Requisito | Versión | Notas |
|---|---|---|
| Rust | **1.95+** | Edición 2024; `rustc`/`cargo` del toolchain estable. |
| GTK4 | **4.10+** | La UI usa APIs de GTK 4.10 (`GtkFileDialog`, `gtk_v4_10`). Probado con 4.24.2. |
| libadwaita | **1.4+** | `AdwOverlaySplitView`, `AdwBreakpointBin`, `AdwClamp`, `AdwMessageDialog`. Probado con 1.10.0. |
| pkg-config | cualquiera reciente | Para localizar GTK/libadwaita. |
| C/C++ toolchain | gcc/clang | `rusqlite` va con SQLite embebido; GTK viene de la distro. |

Paquetes habituales:

```bash
# Arch Linux
sudo pacman -S --needed base-devel rust gtk4 libadwaita

# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel
```

La feature `gtk_v4_10` de `libadwaita` activa `gtk/v4_10`; sin ella no existen
`OverlaySplitView` ni `BreakpointBin` en los bindings.

## 2. Dependencias Rust

Versiones declaradas en `Cargo.toml` (workspace). La UI fija
`libadwaita = { features = ["v1_4", "gtk_v4_10"] }`.

| Crate | Versión | Uso | Licencia |
|---|---|---|---|
| `apk-info` | 1.1 | Parseo de APK/AXML/ARSC | Apache-2.0 |
| `clap` | 4.6 | CLI con derive | MIT/Apache-2.0 |
| `dirs` | 7.0 | Rutas XDG | MIT/Apache-2.0 |
| `gtk4` | 0.11 | Bindings GTK4 | MIT |
| `libadwaita` | 0.9 | Bindings libadwaita | MIT |
| `png` | 0.18 | Iconos de respaldo | MIT/Apache-2.0 |
| `rusqlite` | 0.43 | SQLite (feature `bundled`) | MIT |
| `serde` / `serde_json` | 1.0 | JSON de D-Bus y persistencia | MIT/Apache-2.0 |
| `tempfile` | 3.0 | Tests | MIT/Apache-2.0 |
| `thiserror` | 2.0 | Errores tipados | MIT/Apache-2.0 |
| `tokio` | 1.54 | Runtime async, procesos, timeouts | MIT |
| `toml` | 1.2 | `config.toml` | MIT/Apache-2.0 |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | Logs | MIT |
| `zbus` | 6.5 | D-Bus de sesión | MIT |
| `anyhow` | 1.0 | Errores de aplicación | MIT/Apache-2.0 |

`rusqlite` usa `bundled`, así que no hace falta `libsqlite3-dev`. `apk-info`
arrastra su propio parser; no se reimplementa AXML.

## 3. Dependencias del sistema en ejecución

| Componente | Versión probada | Obligatorio | Uso |
|---|---|---|---|
| Linux x86_64 | — | Sí | Host. |
| `/dev/kvm` | — | **Sí** | Aceleración; sin KVM el rendimiento es inaceptable (TCG solo como rescate). |
| Android SDK | cmdline-tools 19.0 | Sí | `sdkmanager`, `avdmanager`. |
| `platform-tools` (adb) | 37.0.3 | Sí | Servidor ADB propio en `:5038`. |
| Android Emulator | 37.3.10 | Sí | QEMU oficial headless. |
| Imagen API 34 | `system-images;android-34;google_apis;x86_64` (4,2 GB) | Sí | Android 14 con APIs de Google. |
| `scrcpy` | **5.0.1** | Sí | Display virtual, ventana SDL3, audio, portapapeles. |
| `scrcpy-server` | el de scrcpy 5.0.1 | Sí | Se empuja al guest en cada sesión. |
| Wayland + compositor | KDE Plasma 6 probado | Recomendado | Ventanas nativas; X11/XWayland best-effort. |
| PipeWire o PulseAudio | — | Para audio | Salida de audio del guest. |
| systemd (usuario) | — | Sí para el daemon | `apkstartd.service`. |
| D-Bus de sesión | — | Sí para el daemon | `org.apkstart.Manager1`. |
| `xdg-mime` | — | Para integración | Handler MIME de APK. |
| `binfmt_misc` | módulo del kernel | Para `./app.apk` | Registro en `/proc/sys/fs/binfmt_misc`. |

El emulador incluye QEMU; **no** se instala QEMU del sistema. El SDK y la imagen
de Google no se redistribuyen: cada usuario los descarga y acepta sus licencias.

## 4. Versiones del host de desarrollo

Datos medidos, no mínimos garantizados:

- CachyOS/Arch Linux, kernel con KVM, 14 GB de RAM, 12 hilos.
- AMD Rembrandt iGPU (`1002:1681`) + RX 6400 (`1002:743f`); VA-API.
- KDE Plasma Wayland; `wayland-1`; PipeWire.
- GTK 4.24.2, libadwaita 1.10.0, Rust estable 1.95.
- Emulador con `-no-window -gpu host -accel on`; guest de 3 GB y 4 vCPU.
- scrcpy 5.0.1 con decodificación VA-API.

## 5. Licencias

| Componente | Licencia | ¿Se redistribuye? |
|---|---|---|
| Código de models-apk | **MIT** | Sí. |
| `apk-info` | Apache-2.0 | Sí, como dependencia. |
| `scrcpy` / `scrcpy-server` | Apache-2.0 | Sí, como dependencia externa. |
| QEMU (dentro del emulador) | GPL-2.0 | No se empaqueta. |
| Android Emulator | Propietario de Google | No. |
| Imágenes `google_apis` / Play | Propietario de Google | No; el usuario las instala y acepta. |
| GTK4 / libadwaita | LGPL-2.1 | Se enlazan dinámicamente. |
| Crates Rust | MIT/Apache-2.0 (salvo indicación) | Sí. |

El proyecto no incluye APK, imágenes de Android, emulador ni binarios de Google
en el repositorio. `*.apk` está en `.gitignore`.
