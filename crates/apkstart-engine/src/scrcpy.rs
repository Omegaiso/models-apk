use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use apkstart_core::Config;
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::time::sleep;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrcpyOptions {
    pub package: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    #[serde(default)]
    pub gamepad_uhid: bool,
}

impl ScrcpyOptions {
    pub fn from_config(
        config: &Config,
        package: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            title: title.into(),
            width: config.display_width,
            height: config.display_height,
            dpi: config.display_dpi,
            gamepad_uhid: config.gamepad_uhid,
        }
    }

    pub fn display_spec(&self) -> String {
        format!("{}x{}/{}", self.width, self.height, self.dpi)
    }
}

pub struct ScrcpySession {
    child: Child,
    log_path: PathBuf,
}

impl ScrcpySession {
    pub async fn spawn(config: &Config, adb_serial: &str, options: &ScrcpyOptions) -> Result<Self> {
        if !config.scrcpy_path.is_file() {
            return Err(Error::Config(format!(
                "no se encontró scrcpy en {}",
                config.scrcpy_path.display()
            )));
        }

        fs::create_dir_all(config.state_dir.join("sessions"))?;
        let log_path = config.session_log_path(&options.package);
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = log.try_clone()?;

        let mut command = Command::new(&config.scrcpy_path);
        command
            .arg("-s")
            .arg(adb_serial)
            .arg(format!("--new-display={}", options.display_spec()))
            .arg(format!("--start-app={}", options.package))
            .arg("--flex-display")
            .arg("--display-ime-policy=local")
            .arg("--keep-active")
            .arg("--no-vd-system-decorations")
            .arg(if options.gamepad_uhid {
                "--gamepad=uhid"
            } else {
                "--gamepad=disabled"
            })
            .arg(format!("--window-title={}", options.title))
            .arg("--max-fps=60")
            .arg("--video-bit-rate=8M")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(false);

        if let Some(server) = &config.scrcpy_server_path
            && server.is_file()
        {
            command.env("SCRCPY_SERVER_PATH", server);
        }

        // SDL3 usa este ID para el app_id de Wayland y la clase de X11.
        command.env("SDL_APP_ID", apkstart_core::app_id(&options.package));
        command.env("SDL_APP_NAME", &options.title);
        command.env("ANDROID_ADB_SERVER_PORT", config.adb_port.to_string());

        // scrcpy invoca `adb` desde PATH; el SDK puede no estar en el PATH del usuario.
        if let Some(adb_dir) = config.adb_path.parent() {
            let mut paths = vec![adb_dir.to_path_buf()];
            if let Some(existing) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            if let Ok(path) = std::env::join_paths(paths) {
                command.env("PATH", path);
            }
        }

        tracing::info!(
            package = %options.package,
            display = %options.display_spec(),
            log = %log_path.display(),
            "lanzando scrcpy"
        );

        let mut child = command
            .spawn()
            .map_err(|error| Error::Scrcpy(format!("no se pudo lanzar scrcpy: {error}")))?;

        sleep(Duration::from_secs(1)).await;
        if let Some(status) = child.try_wait()? {
            let tail = log_tail(&log_path, 20);
            return Err(Error::Scrcpy(format!(
                "scrcpy terminó al arrancar ({status}). Últimas líneas:\n{tail}"
            )));
        }

        Ok(Self { child, log_path })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn log_path(&self) -> &std::path::Path {
        &self.log_path
    }

    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        Ok(self.child.wait().await?)
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

fn log_tail(path: &std::path::Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return "(sin log)".to_string();
    };
    let all = text.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}
