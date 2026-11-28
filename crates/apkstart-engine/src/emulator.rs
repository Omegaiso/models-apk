use std::fs::{self, OpenOptions};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use apkstart_core::{Config, RuntimeState, RuntimeStatus};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::sleep;

use crate::adb::Adb;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub critical: bool,
}

pub struct AndroidEmulator {
    config: Config,
}

impl AndroidEmulator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn kvm_available() -> bool {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok()
    }

    fn avd_is_persistent(&self) -> bool {
        let Ok(text) = fs::read_to_string(self.config.avd_config_path()) else {
            return false;
        };
        !text.lines().any(|line| {
            let compact: String = line
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            compact == "disk.dataPartition.path=<temp>"
        })
    }

    pub fn checks(&self) -> Vec<Check> {
        let kvm = Self::kvm_available();
        let adb = self.config.adb_path.is_file();
        let emulator = self.config.emulator_path.is_file();
        let avd = self.config.avd_exists();
        let avd_persistent = self.avd_is_persistent();
        let scrcpy = self.config.scrcpy_path.is_file();
        let server = self
            .config
            .scrcpy_server_path
            .as_ref()
            .map(|path| path.is_file())
            .unwrap_or(false);
        let wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let session_bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some();

        vec![
            Check {
                name: "KVM".to_string(),
                ok: kvm,
                detail: if kvm {
                    "/dev/kvm accesible".to_string()
                } else {
                    "no se puede abrir /dev/kvm; el rendimiento será inaceptable".to_string()
                },
                critical: true,
            },
            Check {
                name: "adb".to_string(),
                ok: adb,
                detail: format!(
                    "{} (puerto de servidor {})",
                    self.config.adb_path.display(),
                    self.config.adb_port
                ),
                critical: true,
            },
            Check {
                name: "emulator".to_string(),
                ok: emulator,
                detail: self.config.emulator_path.display().to_string(),
                critical: true,
            },
            Check {
                name: "AVD".to_string(),
                ok: avd,
                detail: if avd {
                    format!(
                        "{} ({})",
                        self.config.avd_name,
                        self.config.avd_ini_path().display()
                    )
                } else {
                    format!("falta el AVD `{}`", self.config.avd_name)
                },
                critical: true,
            },
            Check {
                name: "AVD datos".to_string(),
                ok: avd_persistent,
                detail: if avd_persistent {
                    format!(
                        "partición persistente ({})",
                        self.config.avd_config_path().display()
                    )
                } else {
                    "config.ini usa datos temporales; los APK se perderán al reiniciar \
                     (elimina `disk.dataPartition.path=<temp>`)"
                        .to_string()
                },
                critical: false,
            },
            Check {
                name: "scrcpy".to_string(),
                ok: scrcpy,
                detail: self.config.scrcpy_path.display().to_string(),
                critical: true,
            },
            Check {
                name: "scrcpy-server".to_string(),
                ok: server,
                detail: self
                    .config
                    .scrcpy_server_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "no encontrado".to_string()),
                critical: true,
            },
            Check {
                name: "Wayland".to_string(),
                ok: wayland,
                detail: if wayland {
                    "sesión Wayland detectada".to_string()
                } else {
                    "no hay Wayland; se usará X11/XWayland como best-effort".to_string()
                },
                critical: false,
            },
            Check {
                name: "D-Bus de sesión".to_string(),
                ok: session_bus,
                detail: if session_bus {
                    "disponible".to_string()
                } else {
                    "no disponible; el daemon no podrá activarse por D-Bus".to_string()
                },
                critical: false,
            },
        ]
    }

    pub async fn status(&self) -> RuntimeStatus {
        let adb = Adb::new(&self.config);
        if adb.is_online().await {
            let booted = adb
                .shell(&["getprop", "sys.boot_completed"])
                .await
                .map(|value| value.trim() == "1")
                .unwrap_or(false);
            if booted {
                RuntimeStatus::running(self.config.serial())
            } else {
                RuntimeStatus {
                    state: RuntimeState::Starting,
                    serial: Some(self.config.serial()),
                    boot_completed: false,
                    message: Some("emulador online, Android aún arrancando".to_string()),
                }
            }
        } else {
            RuntimeStatus::stopped(Some(self.config.serial()))
        }
    }

    pub async fn is_online(&self) -> bool {
        Adb::new(&self.config).is_online().await
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_online().await {
            return Ok(());
        }

        require_file(&self.config.emulator_path, "emulator")?;
        require_file(&self.config.adb_path, "adb")?;
        if !self.config.avd_exists() {
            return Err(Error::Config(format!(
                "falta el AVD `{}` en {}",
                self.config.avd_name,
                self.config.avd_ini_path().display()
            )));
        }

        let kvm = Self::kvm_available();
        if !kvm && !self.config.allow_tcg {
            return Err(Error::Kvm(
                "no se puede abrir /dev/kvm; define APKSTART_ALLOW_TCG=1 para forzar TCG"
                    .to_string(),
            ));
        }
        if !kvm {
            tracing::warn!("KVM no disponible; arrancando con TCG (muy lento)");
        }

        fs::create_dir_all(&self.config.state_dir)?;
        let log_path = self.config.emulator_log_path();
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = log.try_clone()?;

        tracing::info!(
            avd = %self.config.avd_name,
            port = self.config.emulator_port,
            gpu = %self.config.gpu_mode,
            log = %log_path.display(),
            "arrancando Android Emulator"
        );

        let mut command = Command::new(&self.config.emulator_path);
        command
            .env("ANDROID_ADB_SERVER_PORT", self.config.adb_port.to_string())
            .arg("-avd")
            .arg(&self.config.avd_name)
            .arg("-no-window")
            .arg("-no-boot-anim")
            .arg("-gpu")
            .arg(&self.config.gpu_mode)
            .arg("-no-metrics")
            .arg("-camera-back")
            .arg(if self.config.camera_back.trim().is_empty() {
                "none"
            } else {
                &self.config.camera_back
            })
            .arg("-camera-front")
            .arg(if self.config.camera_front.trim().is_empty() {
                "none"
            } else {
                &self.config.camera_front
            })
            .arg("-accel")
            .arg(if kvm { "on" } else { "off" })
            .arg("-port")
            .arg(self.config.emulator_port.to_string())
            .arg("-netdelay")
            .arg("none")
            .arg("-netspeed")
            .arg("full")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(false);

        if self.config.host_audio_input {
            command.arg("-allow-host-audio");
            // El backend PulseAudio del emulador falla en algunos hosts con
            // PipeWire; ALSA usa la compatibilidad de PipeWire y permite la
            // entrada de micrófono. Se puede sobreescribir desde el entorno.
            if std::env::var_os("QEMU_AUDIO_IN_DRV").is_none() {
                command.env("QEMU_AUDIO_IN_DRV", "alsa");
            }
        }

        if self.config.cold_boot {
            command.arg("-no-snapshot-load");
        }

        let mut child = command
            .spawn()
            .map_err(|error| Error::Emulator(format!("no se pudo lanzar el emulador: {error}")))?;
        let pid = child.id();
        tracing::info!(pid = ?pid, "emulador lanzado");

        sleep(Duration::from_secs(3)).await;
        if let Some(status) = child.try_wait()? {
            let tail = log_tail(&log_path, 15);
            return Err(Error::Emulator(format!(
                "el emulador terminó al arrancar ({status}). Últimas líneas:\n{tail}"
            )));
        }

        let adb = Adb::new(&self.config);
        let boot = adb
            .wait_boot(Duration::from_secs(self.config.boot_timeout_secs))
            .await;
        if let Err(error) = boot {
            let _ = child.kill().await;
            let tail = log_tail(&log_path, 15);
            return Err(Error::Emulator(format!(
                "{error}. Últimas líneas del emulador:\n{tail}"
            )));
        }

        tracing::info!(serial = %self.config.serial(), "runtime Android listo");
        Ok(())
    }

    pub async fn ensure_running(&self) -> Result<Adb> {
        if self.is_online().await {
            let adb = Adb::new(&self.config);
            adb.wait_boot(Duration::from_secs(self.config.boot_timeout_secs))
                .await?;
            return Ok(adb);
        }
        self.start().await?;
        Ok(Adb::new(&self.config))
    }

    pub async fn stop(&self) -> Result<()> {
        let adb = Adb::new(&self.config);
        if !adb.is_online().await {
            return Ok(());
        }
        adb.emu_kill().await?;
        // El apagado con quickboot puede tardar en guardar el snapshot.
        for _ in 0..120 {
            if !adb.is_online().await {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        Err(Error::Timeout(
            "el emulador sigue online 60 s después de adb emu kill".to_string(),
        ))
    }
}

fn require_file(path: &Path, name: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "no se encontró {name} en {}",
            path.display()
        )))
    }
}

fn log_tail(path: &Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return "(sin log)".to_string();
    };
    let all = text.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}
