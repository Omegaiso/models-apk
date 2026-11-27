# Paquetes, CI y actualizaciones (Fase 2)

> **Estado: diseño pendiente de implementación.** Nada de esta página está
> publicado todavía. El repositorio será público y los artefactos se publicarán
> en GitHub Releases.

## 1. Objetivos

1. Publicar instaladores nativos para las tres familias principales:
   `.deb`, `.rpm` y paquetes de Arch (`.pkg.tar.zst`).
2. Permitir una instalación de una línea:
   `curl -fsSL https://raw.githubusercontent.com/Omegaiso/models-apk/main/install.sh | sh`.
3. Detectar versiones nuevas desde la propia app y, si el usuario lo autoriza,
   actualizar sin salir de models-apk.
4. Repetir la misma lógica de `binfmt_misc` que `packaging/install.sh`, para que
   `./app.apk` siga funcionando tras una actualización.

## 2. Versionado y releases

- Semver: `MAJOR.MINOR.PATCH`, alineado con `workspace.package.version` en
  `Cargo.toml`.
- Tag `vX.Y.Z`; el workflow de release se dispara con tags `v*`.
- Cada release incluye:
  - `models-apk_X.Y.Z_amd64.deb`
  - `models-apk-X.Y.Z-1.x86_64.rpm`
  - `models-apk-X.Y.Z-1-x86_64.pkg.tar.zst`
  - `models-apk-X.Y.Z-x86_64.tar.gz` (binarios + unidades + licencia)
  - `SHA256SUMS`
  - `install.sh`
  - notas de release generadas desde `CHANGELOG.md` o commits.

## 3. GitHub Actions

Borrador del workflow (`.github/workflows/release.yml`):

```yaml
name: release
on:
  push:
    tags: ["v*"]
  workflow_dispatch:

jobs:
  test:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get update && sudo apt-get install -y libgtk-4-dev libadwaita-1-dev
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace

  package-deb-rpm:
    needs: test
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: sudo apt-get update && sudo apt-get install -y libgtk-4-dev libadwaita-1-dev
      - run: cargo build --release --locked
      # nfpm genera .deb y .rpm desde el mismo manifiesto
      - run: ./packaging/ci/nfpm.sh
      - uses: actions/upload-artifact@v4
        with: { name: deb-rpm, path: dist/*.deb, if-no-files-found: error }
```

Pendiente de decidir:

- **Herramienta:** `nfpm` (un manifiesto YAML para `.deb` y `.rpm`) frente a
  `cargo-deb` + `cargo-generate-rpm`. `nfpm` simplifica el `postinst` común.
- **Arch:** construir en un contenedor `archlinux:base-devel` con un `PKGBUILD`
  en `packaging/arch/`. El paquete debe declarar dependencias `gtk4`,
  `libadwaita`, `scrcpy` y ejecutar el hook de binfmt en `.INSTALL`.
- **Firma:** `SHA256SUMS` es obligatorio; la firma GPG/minisign es deseable.
- **Runner:** los binarios se compilan una vez en Ubuntu 24.04 (glibc moderna).
  Si se necesita compatibilidad con Debian estable, usar un contenedor con la
  glibc objetivo.

### Contenido de cada paquete

| Ruta | Contenido |
|---|---|
| `/usr/bin/apkstart`, `apkstartd`, `apkstart-settings` | Binarios. |
| `/usr/lib/systemd/user/apkstartd.service` | Servicio de usuario. |
| `/usr/share/dbus-1/services/org.apkstart.Manager1.service` | Activación D-Bus. |
| `/usr/share/applications/io.github.apkstart.Settings.desktop` | Ajustes en el menú. |
| `/usr/share/applications/io.github.apkstart.install.desktop` | Handler MIME de APK. |
| `/usr/lib/models-apk/binfmt/*` | Hook de `./app.apk`. |
| `/usr/share/licenses/models-apk/LICENSE` | Licencia MIT. |

`postinst`/`post_install` ejecuta `/usr/lib/models-apk/binfmt/install.sh
/usr/bin/apkstart`; `prerm`/`pre_remove` ejecuta el `uninstall.sh`. Cada usuario
sigue necesitando `apkstart setup` una vez (MIME + daemon).

## 4. `install.sh` universal

Responsabilidades:

1. Detectar arquitectura (`x86_64`) y distro (`/etc/os-release`).
2. Consultar la última release pública:
   `https://api.github.com/repos/Omegaiso/models-apk/releases/latest`.
3. Elegir el asset según el gestor:
   - `apt` → `.deb`
   - `dnf`/`yum` → `.rpm`
   - `pacman` → `.pkg.tar.zst`
   - fallback → tarball a `~/.local` (sin root, sin binfmt del sistema).
4. Descargar a un directorio temporal, verificar `SHA256SUMS` y instalar con
   `sudo apt install`, `sudo dnf install` o `sudo pacman -U`.
5. Ejecutar el hook de binfmt (los paquetes ya lo hacen; el fallback debe
   pedirlo explícitamente).
6. No ejecutar binarios descargados sin verificar; no usar `curl | sh` para
   nada que no sea este script fijado por URL HTTPS.

Modos:

```bash
curl -fsSL .../install.sh | sh            # instala la última release
curl -fsSL .../install.sh | sh -s -- --version 0.2.0
curl -fsSL .../install.sh | sh -s -- --dry-run
```

## 5. Detección de actualizaciones

### CLI

```bash
apkstart update --check      # muestra versión actual y disponible
apkstart update              # descarga e instala (pide confirmación)
apkstart update --yes        # sin confirmación (scripts/CI)
```

- `--check` consulta la API pública de GitHub Releases y compara semver.
- El resultado se guarda en `settings` (`last_update_check`,
  `skipped_version`) para no repetir la petición en cada arranque.
- Sin conexión o rate limit: se informa y no se bloquea nada.

### UI de ajustes

- Bloque “Actualizaciones” en **Acerca de** o **Integración**:
  - versión instalada y última disponible;
  - botón **Buscar actualizaciones**;
  - changelog resumido;
  - botón **Actualizar** (solo si hay asset compatible);
  - toggle **Avisarme automáticamente** (opt-in).
- La instalación se delega a un helper con `pkexec`/`sudo`; la UI nunca ejecuta
  comandos de red sin mostrar antes el origen y la versión.

### Actualización automática

- Comprobación diaria desde `apkstartd` (o al abrir Ajustes) si el usuario la
  activó.
- **Nunca** instalar en silencio: notificar y pedir confirmación.
- Descarga con HTTPS, verificación SHA256 y, cuando exista, firma minisign/GPG.
- Si el paquete falla, conservar la versión anterior y registrar el error.
- En Arch/`.deb`/`.rpm` se usa el gestor de la distro; el tarball se reemplaza
  con backup y rollback.

## 6. Seguridad

- El repositorio y las releases son públicos; la API de GitHub no requiere token
  para consultar la última versión (límite de 60 peticiones/hora por IP).
- Checksums en `SHA256SUMS`; firma del maintainer recomendada.
- `install.sh` se sirve desde `main` con HTTPS. Para producción conviene fijar
  una release concreta y publicar su hash en la documentación.
- No se redistribuyen SDK, emulador ni imágenes de Google.

## 7. Checklist de implementación

- [ ] Hacer público el repositorio.
- [ ] `packaging/ci/nfpm.yaml` + script de build.
- [ ] `packaging/arch/PKGBUILD` + `.INSTALL`.
- [ ] `install.sh` en la raíz con detección de distro y checksum.
- [ ] `.github/workflows/release.yml` con test + matrix de paquetes.
- [ ] `CHANGELOG.md` y notas de release.
- [ ] `apkstart update --check` (cliente GitHub + semver).
- [ ] Bloque de actualizaciones en la UI y ajuste persistido.
- [ ] Firma/checksum y documentación de rollback.
- [ ] Probar instalación limpia en Debian, Fedora y Arch (VM/contenedor).
