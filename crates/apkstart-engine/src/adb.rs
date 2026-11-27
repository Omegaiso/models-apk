use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use apkstart_core::Config;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::{sleep, timeout};

use crate::{Error, Result};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdbDevice {
    pub serial: String,
    pub state: String,
}

pub struct Adb {
    path: PathBuf,
    serial: String,
    port: u16,
}

impl Adb {
    pub fn new(config: &Config) -> Self {
        Self {
            path: config.adb_path.clone(),
            serial: config.serial(),
            port: config.adb_port,
        }
    }

    pub fn with_serial(config: &Config, serial: impl Into<String>) -> Self {
        Self {
            path: config.adb_path.clone(),
            serial: serial.into(),
            port: config.adb_port,
        }
    }

    fn apply_port(command: &mut Command, port: u16) {
        command.env("ANDROID_ADB_SERVER_PORT", port.to_string());
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    async fn run(&self, args: &[&str]) -> Result<Output> {
        let mut command = Command::new(&self.path);
        command.arg("-s").arg(&self.serial).args(args);
        Self::apply_port(&mut command, self.port);
        let output = timeout(COMMAND_TIMEOUT, command.output())
            .await
            .map_err(|_| Error::Timeout(format!("adb {}", args.join(" "))))??;
        Ok(output)
    }

    pub async fn shell(&self, args: &[&str]) -> Result<String> {
        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.push("shell");
        full_args.extend_from_slice(args);
        let output = self.run(&full_args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!(
                "`adb shell {}` falló: {}",
                args.join(" "),
                stderr.trim()
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::debug!(serial = %self.serial, args = ?args, stdout = %stdout, "adb shell");
        Ok(stdout)
    }

    pub async fn devices(path: &Path, port: u16) -> Result<Vec<AdbDevice>> {
        let mut command = Command::new(path);
        command.arg("devices");
        Self::apply_port(&mut command, port);
        let output = timeout(COMMAND_TIMEOUT, command.output())
            .await
            .map_err(|_| Error::Timeout("adb devices".to_string()))??;
        if !output.status.success() {
            return Err(Error::Adb(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(parse_devices(&String::from_utf8_lossy(&output.stdout)))
    }

    pub async fn get_state(&self) -> Result<String> {
        let output = self.run(&["get-state"]).await?;
        if !output.status.success() {
            return Err(Error::Adb(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn is_online(&self) -> bool {
        matches!(self.get_state().await.as_deref(), Ok("device"))
    }

    pub async fn wait_for_device(&self, limit: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + limit;
        loop {
            if matches!(self.get_state().await.as_deref(), Ok("device")) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "esperando a que {} esté online",
                    self.serial
                )));
            }
            sleep(Duration::from_millis(500)).await;
        }
    }

    pub async fn wait_boot(&self, limit: Duration) -> Result<()> {
        self.wait_for_device(limit).await?;
        let deadline = tokio::time::Instant::now() + limit;
        loop {
            match self.shell(&["getprop", "sys.boot_completed"]).await {
                Ok(value) if value.trim() == "1" => return Ok(()),
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "esperando sys.boot_completed en {}",
                    self.serial
                )));
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn install(&self, apk: &Path) -> Result<()> {
        let apk = apk.to_string_lossy().into_owned();
        let output = self.run(&["install", "-r", &apk]).await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() && stdout.contains("Success") {
            return Ok(());
        }
        Err(Error::Adb(format!(
            "adb install falló: {} {}",
            stdout.trim(),
            stderr.trim()
        )))
    }

    pub async fn uninstall(&self, package: &str, keep_data: bool) -> Result<()> {
        let args = if keep_data {
            vec!["uninstall", "-k", package]
        } else {
            vec!["uninstall", package]
        };
        let output = self.run(&args).await?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Success") {
            Ok(())
        } else {
            Err(Error::Adb(format!(
                "adb uninstall falló: {}{}",
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    pub async fn list_packages(&self, third_party: bool) -> Result<Vec<String>> {
        let output = if third_party {
            self.shell(&["pm", "list", "packages", "-3"]).await?
        } else {
            self.shell(&["pm", "list", "packages"]).await?
        };
        let mut packages = output
            .lines()
            .filter_map(|line| line.strip_prefix("package:"))
            .map(str::to_string)
            .collect::<Vec<_>>();
        packages.sort();
        packages.dedup();
        Ok(packages)
    }

    pub async fn force_stop(&self, package: &str) -> Result<()> {
        self.shell(&["am", "force-stop", package]).await?;
        Ok(())
    }

    /// Devuelve la ruta del APK base instalado para un package.
    pub async fn package_apk_path(&self, package: &str) -> Result<String> {
        let output = self.shell(&["pm", "path", package]).await?;
        let paths = output
            .lines()
            .filter_map(|line| line.strip_prefix("package:"))
            .collect::<Vec<_>>();
        paths
            .iter()
            .find(|path| path.ends_with("/base.apk"))
            .or_else(|| paths.first())
            .map(|path| (*path).to_string())
            .ok_or_else(|| Error::Adb(format!("no se encontró el APK de {package}")))
    }

    pub async fn pull(&self, remote: &str, local: &Path) -> Result<()> {
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let local = local.to_string_lossy().into_owned();
        let output = self.run(&["pull", remote, &local]).await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Adb(format!(
                "adb pull falló: {}{}",
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    pub async fn emu_kill(&self) -> Result<()> {
        let output = self.run(&["emu", "kill"]).await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Adb(format!(
                "adb emu kill falló: {}{}",
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }
}

fn parse_devices(output: &str) -> Vec<AdbDevice> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next().unwrap_or("unknown").to_string();
            Some(AdbDevice { serial, state })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_devices;

    #[test]
    fn parses_adb_devices_output() {
        let output = "List of devices attached\nemulator-5556\tdevice\n1234567890\tunauthorized\n";
        let devices = parse_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5556");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[1].state, "unauthorized");
    }

    #[test]
    fn ignores_empty_output() {
        assert!(parse_devices("List of devices attached\n\n").is_empty());
    }
}
