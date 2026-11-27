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

1. Instalar el binario `apkstart` (por ejemplo `/usr/bin/apkstart`).
2. Instalar `LICENSE`.
3. Instalar `packaging/binfmt/install.sh`, `models-apk.conf` y `uninstall.sh` en `/usr/lib/models-apk/binfmt/`.
4. En `postinst`/`post_install` ejecutar:
   ```bash
   /usr/lib/models-apk/binfmt/install.sh /usr/bin/apkstart
   ```
5. En `prerm`/`pre_remove` ejecutar:
   ```bash
   /usr/lib/models-apk/binfmt/uninstall.sh
   ```

El script de binfmt instala `/etc/binfmt.d/models-apk.conf` y `/usr/local/bin/apkstart-run`, y reinicia `systemd-binfmt.service`. Se ejecuta como root desde el gestor de paquetes.

## Integración por usuario

El registro MIME por defecto (`xdg-mime`) y el timer de sincronización son por usuario. El paquete puede instalar el `.desktop` del handler en `/usr/share/applications/`, pero cada usuario debe ejecutar una vez:

```bash
apkstart setup
```

Eso registra el handler MIME, activa `apkstart-sync.timer` y sincroniza las apps ya instaladas. En una versión futura, `apkstartd` podrá hacerlo sin intervención.

## Verificación

```bash
cat /proc/sys/fs/binfmt_misc/models-apk
xdg-mime query default application/vnd.android.package-archive
systemctl --user status apkstart-sync.timer
```
