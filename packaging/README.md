# Empaquetado de models-apk

## Regla de instalación

**Toda instalación del proyecto debe registrar `binfmt_misc` automáticamente.** No se debe pedir al usuario que ejecute a mano:

```bash
sudo packaging/binfmt/install.sh /ruta/apkstart
```

El instalador local ya lo hace:

```bash
cargo build --release --locked
sudo packaging/install.sh            # /usr/local
PREFIX=/usr sudo packaging/install.sh # /usr
```

## Qué debe hacer un paquete de distro

1. Instalar los binarios `apkstart` y `apkstartd` (por ejemplo `/usr/bin/`).
2. Instalar `LICENSE`.
3. Instalar `packaging/binfmt/install.sh`, `models-apk.conf` y `uninstall.sh` en `/usr/lib/models-apk/binfmt/`.
4. Instalar `apkstartd.service` en `/usr/lib/systemd/user/` y la activación D-Bus en `/usr/share/dbus-1/services/` (el instalador local los genera desde `packaging/systemd/` y `packaging/dbus/`).
5. En `postinst`/`post_install` ejecutar:
   ```bash
   /usr/lib/models-apk/binfmt/install.sh /usr/bin/apkstart
   ```
6. En `prerm`/`pre_remove` ejecutar:
   ```bash
   /usr/lib/models-apk/binfmt/uninstall.sh
   ```

El script de binfmt instala `/etc/binfmt.d/models-apk.conf` y `/usr/local/bin/apkstart-run`, y reinicia `systemd-binfmt.service`. Se ejecuta como root desde el gestor de paquetes.

## Integración por usuario

El registro MIME por defecto (`xdg-mime`) y el servicio `apkstartd` son por usuario. El paquete puede instalar el `.desktop` del handler en `/usr/share/applications/`, pero cada usuario debe ejecutar una vez:

```bash
apkstart setup
```

Eso registra el handler MIME, instala/activa `apkstartd.service` (D-Bus), retira el timer antiguo y sincroniza las apps ya instaladas. El daemon hace el sync cada 15 s.

## Verificación

```bash
cat /proc/sys/fs/binfmt_misc/models-apk
xdg-mime query default application/vnd.android.package-archive
systemctl --user status apkstartd.service
busctl --user call org.apkstart.Manager1 /org/apkstart/Manager1 org.apkstart.Manager1 GetVersion
```
