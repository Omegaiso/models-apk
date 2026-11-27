# Instalación y diagnóstico

## 1. Instalación desde fuente

```bash
git clone https://github.com/Omegaiso/models-apk.git
cd models-apk
cargo build --release --locked

sudo packaging/install.sh              # /usr/local
# o: PREFIX=/usr sudo packaging/install.sh
```

`packaging/install.sh`:

1. instala `apkstart`, `apkstartd` y `apkstart-settings` en `$PREFIX/bin`;
2. instala `LICENSE`, las unidades de systemd y la activación D-Bus;
3. instala los `.desktop` del handler APK y de ajustes;
4. registra **automáticamente** `binfmt_misc` para `./app.apk`;
5. recuerda ejecutar `apkstart setup` como usuario.

Después, una vez por usuario:

```bash
apkstart setup
apkstart doctor
```

`setup` registra el handler MIME con `xdg-mime`, instala/activa
`apkstartd.service`, retira el timer antiguo y sincroniza las apps existentes.

## 2. SDK, emulador y AVD

El proyecto **no** descarga ni redistribuye el SDK de Google. Instálalo aparte:

```bash
export ANDROID_SDK_ROOT="$HOME/Android/Sdk"
# cmdline-tools en $ANDROID_SDK_ROOT/cmdline-tools/latest
sdkmanager --install \
  "platform-tools" "emulator" \
  "system-images;android-34;google_apis;x86_64"

avdmanager create avd \
  -n apkstart \
  -k "system-images;android-34;google_apis;x86_64" \
  -d pixel_7
```

### `config.ini` del AVD

Ruta: `~/.android/avd/apkstart.avd/config.ini`.

```ini
hw.ramSize=3072
hw.cpu.ncore=4
hw.gpu.enabled=yes
hw.gpu.mode=host
disk.dataPartition.size=6G
userdata.useQcow2=yes
```

**Crítico:** elimina cualquier línea

```ini
disk.dataPartition.path=<temp>
```

Si existe, la partición de datos es temporal y los APK instalados desaparecen al
reiniciar el emulador. `apkstart doctor` comprueba y avisa.

El primer arranque de un AVD nuevo es en frío. A partir de ahí se usa quickboot
(snapshot) para conservar apps y datos. `APKSTART_COLD_BOOT=1` fuerza un arranque
en frío para recuperación; si el snapshot está corrupto, borra
`~/.android/avd/apkstart.avd/snapshots/`.

## 3. scrcpy

Instala scrcpy 5.x desde la distro. Si usas una copia local:

```bash
export APKSTART_SCRCPY="$HOME/.local/opt/scrcpy/usr/bin/scrcpy"
export APKSTART_SCRCPY_SERVER="$HOME/.local/opt/scrcpy/usr/share/scrcpy/scrcpy-server"
```

Ponlas en `~/.config/apkstart/config.toml` si no quieres exportarlas en cada
sesión:

```toml
scrcpy_path = "/home/tu-usuario/.local/opt/scrcpy/usr/bin/scrcpy"
scrcpy_server_path = "/home/tu-usuario/.local/opt/scrcpy/usr/share/scrcpy/scrcpy-server"
```

## 4. Verificación

```bash
apkstart doctor
apkstart runtime start
apkstart install --launch /ruta/app.apk
```

Comprobaciones manuales:

```bash
# El servidor ADB del proyecto es el 5038, no el 5037 del usuario
ANDROID_ADB_SERVER_PORT=5038 adb devices

# Daemon y D-Bus
systemctl --user status apkstartd.service
busctl --user call org.apkstart.Manager1 /org/apkstart/Manager1 \
  org.apkstart.Manager1 GetVersion

# Handler MIME de APK
xdg-mime query default application/vnd.android.package-archive

# binfmt de ./app.apk
cat /proc/sys/fs/binfmt_misc/models-apk

# Logs
tail -f ~/.local/state/apkstart/emulator.log
tail -f ~/.local/state/apkstart/sessions/org.fdroid.fdroid.log
journalctl --user -u apkstartd -f
```

## 5. `./app.apk` y doble clic

- El `.desktop` del handler usa `Exec=apkstart install --launch %f` y
  `Terminal=true` para mostrar el progreso.
- `binfmt_misc` registra el APK como ejecutable con el wrapper
  `/usr/local/bin/apkstart-run`, que llama al CLI **como el usuario**.
- En una instalación completa el registro es automático. Para desarrollo:

```bash
sudo packaging/binfmt/install.sh /ruta/a/target/debug/apkstart
chmod +x app.apk
./app.apk
```

Si el APK no tiene permiso de ejecución, `chmod +x` es necesario. No ejecutes el
instalador como root para lanzar apps: el wrapper baja privilegios.

## 6. Desinstalación

```bash
# Integración de usuario
systemctl --user disable --now apkstartd.service
xdg-mime default '' application/vnd.android.package-archive 2>/dev/null || true

# binfmt y binarios del sistema
sudo packaging/binfmt/uninstall.sh
sudo rm -f /usr/local/bin/apkstart /usr/local/bin/apkstartd \
           /usr/local/bin/apkstart-settings

# Datos del usuario (opcional; borra apps, iconos y configuración)
rm -rf ~/.local/share/apkstart ~/.local/state/apkstart \
       ~/.config/apkstart
rm -f ~/.local/share/applications/io.github.apkstart.*.desktop
```

Los paquetes de distro deben ejecutar el hook de binfmt en `postinst` y
`prerm`; ver [`packaging/README.md`](../packaging/README.md).

## 7. Errores y resolución

| Error / síntoma | Causa | Solución |
|---|---|---|
| `no se puede abrir /dev/kvm` | Permisos o virtualización desactivada | `ls -l /dev/kvm`; `sudo usermod -aG kvm $USER`; reiniciar sesión; comprobar VT-x/AMD-V en la BIOS. |
| `falta el AVD apkstart` | AVD no creado | Sección 2; `avdmanager list avd`. |
| `no se encontró scrcpy` | Ruta incorrecta | `APKSTART_SCRCPY`/`APKSTART_SCRCPY_SERVER` o instalarlo en `PATH`. |
| `scrcpy terminó al arrancar` | scrcpy-server incompatible, ADB o codec | Revisar el log de la sesión; usar scrcpy 5.x y su server. |
| Apps desaparecen al reiniciar | `disk.dataPartition.path=<temp>` | Quitarlo del `config.ini`; `doctor`. |
| Arranque eterno / snapshot corrupto | Quickboot inválido | `APKSTART_COLD_BOOT=1 apkstart runtime start`; borrar snapshots. |
| `adb devices` no ve el emulador | Puerto ADB equivocado | Usar `ANDROID_ADB_SERVER_PORT=5038`. |
| Ventana negra al abrir una app | Bug abierto (display virtual / carrera de sesión) | Cerrar la ventana y reabrir; ver [bugs.md](bugs.md#ventana-negra-al-abrir-una-app). |
| `./app.apk: permiso denegado` | binfmt sin registrar o sin `chmod +x` | `sudo packaging/install.sh`; `chmod +x`. |
| Sin audio | PipeWire/PulseAudio o sesión muda | Probar `apkstart doctor`; revisar el log de scrcpy; comprobar el mezclador. |
| La ventana se abre en otro monitor | KWin decide la posición en Wayland | Moverla manualmente; foco multi-monitor pendiente. |
| UI de ajustes no abre | Falta GTK/libadwaita o el binario | `cargo build -p apkstart-settings`; ejecutar desde terminal para ver el error. |

Para un informe de bug, adjunta:

```bash
apkstart doctor > doctor.txt
apkstart export-log
journalctl --user -u apkstartd -n 200 --no-pager
```
