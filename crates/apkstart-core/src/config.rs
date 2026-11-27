use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Configuración de ApkStart.
///
/// Se lee de `~/.config/apkstart/config.toml` y se puede sobreescribir con
/// variables de entorno `APKSTART_*`. Los valores por defecto asumen el SDK en
/// `~/Android/Sdk` y scrcpy instalado en el sistema o en `~/.local/opt/scrcpy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sdk_root: PathBuf,
    pub avd_name: String,
    pub adb_path: PathBuf,
    pub emulator_path: PathBuf,
    pub scrcpy_path: PathBuf,
    pub scrcpy_server_path: Option<PathBuf>,
    pub emulator_port: u16,
    pub adb_port: u16,
    pub guest_ram_mb: u32,
    pub guest_cores: u8,
    pub display_width: u32,
    pub display_height: u32,
    pub display_dpi: u32,
    pub gpu_mode: String,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub boot_timeout_secs: u64,
    pub allow_tcg: bool,
    pub cold_boot: bool,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let sdk_root = env::var_os("ANDROID_SDK_ROOT")
            .or_else(|| env::var_os("ANDROID_HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("Android/Sdk"));

        let data_dir = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("apkstart");

        let state_dir = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"))
            .join("apkstart");

        let scrcpy_path = env::var_os("APKSTART_SCRCPY")
            .map(PathBuf::from)
            .or_else(|| find_in_path("scrcpy"))
            .or_else(|| {
                let local = home.join(".local/opt/scrcpy/usr/bin/scrcpy");
                local.is_file().then_some(local)
            })
            .unwrap_or_else(|| PathBuf::from("scrcpy"));

        let scrcpy_server_path = env::var_os("APKSTART_SCRCPY_SERVER")
            .map(PathBuf::from)
            .or_else(|| {
                let local = home.join(".local/opt/scrcpy/usr/share/scrcpy/scrcpy-server");
                if local.is_file() {
                    Some(local)
                } else {
                    let system = PathBuf::from("/usr/share/scrcpy/scrcpy-server");
                    system.is_file().then_some(system)
                }
            });

        Self {
            adb_path: sdk_root.join("platform-tools/adb"),
            emulator_path: sdk_root.join("emulator/emulator"),
            sdk_root,
            avd_name: "apkstart".to_string(),
            scrcpy_path,
            scrcpy_server_path,
            emulator_port: 5556,
            adb_port: 5038,
            guest_ram_mb: 3072,
            guest_cores: 4,
            display_width: 900,
            display_height: 1600,
            display_dpi: 240,
            gpu_mode: "host".to_string(),
            data_dir,
            state_dir,
            boot_timeout_secs: 180,
            allow_tcg: false,
            cold_boot: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let mut config = if path.is_file() {
            let text = fs::read_to_string(&path)?;
            toml::from_str::<Self>(&text)?
        } else {
            Self::default()
        };

        config.apply_env();
        config.expand_tilde();
        config.normalize_tools();
        config.ensure_dirs()?;
        Ok(config)
    }

    pub fn config_path() -> PathBuf {
        env::var_os("APKSTART_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("apkstart/config.toml")
            })
    }

    pub fn serial(&self) -> String {
        format!("emulator-{}", self.emulator_port)
    }

    pub fn display_spec(&self) -> String {
        format!(
            "{}x{}/{}",
            self.display_width, self.display_height, self.display_dpi
        )
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("apkstart.db")
    }

    pub fn emulator_log_path(&self) -> PathBuf {
        self.state_dir.join("emulator.log")
    }

    pub fn session_log_path(&self, package: &str) -> PathBuf {
        let safe: String = package
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.state_dir.join("sessions").join(format!("{safe}.log"))
    }

    pub fn avd_ini_path(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".android/avd")
            .join(format!("{}.ini", self.avd_name))
    }

    pub fn avd_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".android/avd")
            .join(format!("{}.avd", self.avd_name))
    }

    pub fn avd_config_path(&self) -> PathBuf {
        self.avd_dir().join("config.ini")
    }

    pub fn avd_exists(&self) -> bool {
        self.avd_ini_path().is_file()
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(self.state_dir.join("sessions"))?;
        Ok(())
    }

    fn apply_env(&mut self) {
        if let Some(value) = env::var_os("APKSTART_SDK_ROOT") {
            self.sdk_root = PathBuf::from(value);
        }
        if let Some(value) = env::var_os("APKSTART_AVD") {
            self.avd_name = value.to_string_lossy().into_owned();
        }
        if let Some(value) =
            env::var_os("APKSTART_PORT").and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
        {
            self.emulator_port = value;
        }
        if let Some(value) =
            env::var_os("APKSTART_ADB_PORT").and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
        {
            self.adb_port = value;
        }
        if let Some(value) =
            env::var_os("APKSTART_RAM_MB").and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
        {
            self.guest_ram_mb = value;
        }
        if let Some(value) =
            env::var_os("APKSTART_CORES").and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
        {
            self.guest_cores = value;
        }
        if let Some(value) = env::var_os("APKSTART_GPU") {
            self.gpu_mode = value.to_string_lossy().into_owned();
        }
        if let Some(value) = env::var_os("APKSTART_SCRCPY") {
            self.scrcpy_path = PathBuf::from(value);
        }
        if let Some(value) = env::var_os("APKSTART_SCRCPY_SERVER") {
            self.scrcpy_server_path = Some(PathBuf::from(value));
        }
        if let Ok(value) = env::var("APKSTART_ALLOW_TCG") {
            self.allow_tcg = matches!(value.as_str(), "1" | "true" | "yes");
        }
        if let Ok(value) = env::var("APKSTART_COLD_BOOT") {
            self.cold_boot = matches!(value.as_str(), "1" | "true" | "yes");
        }
    }

    fn expand_tilde(&mut self) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        self.sdk_root = expand_path(&self.sdk_root, &home);
        self.adb_path = expand_path(&self.adb_path, &home);
        self.emulator_path = expand_path(&self.emulator_path, &home);
        self.scrcpy_path = expand_path(&self.scrcpy_path, &home);
        self.data_dir = expand_path(&self.data_dir, &home);
        self.state_dir = expand_path(&self.state_dir, &home);
        if let Some(server) = &self.scrcpy_server_path {
            self.scrcpy_server_path = Some(expand_path(server, &home));
        }
    }

    fn normalize_tools(&mut self) {
        let sdk_adb = self.sdk_root.join("platform-tools/adb");
        let sdk_emulator = self.sdk_root.join("emulator/emulator");
        if !self.adb_path.is_file() && sdk_adb.is_file() {
            self.adb_path = sdk_adb;
        }
        if !self.emulator_path.is_file() && sdk_emulator.is_file() {
            self.emulator_path = sdk_emulator;
        }
        if !self.scrcpy_path.is_file()
            && let Some(found) = find_in_path("scrcpy")
        {
            self.scrcpy_path = found;
        }
        let server_missing = self
            .scrcpy_server_path
            .as_ref()
            .map(|path| !path.is_file())
            .unwrap_or(true);
        if server_missing {
            self.scrcpy_server_path = find_scrcpy_server();
        }
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn find_scrcpy_server() -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    [
        home.join(".local/opt/scrcpy/usr/share/scrcpy/scrcpy-server"),
        PathBuf::from("/usr/share/scrcpy/scrcpy-server"),
        PathBuf::from("/usr/local/share/scrcpy/scrcpy-server"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn expand_path(path: &Path, home: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home.join(rest);
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_and_display_defaults() {
        let config = Config::default();
        assert_eq!(
            config.serial(),
            format!("emulator-{}", config.emulator_port)
        );
        assert_eq!(
            config.display_spec(),
            format!(
                "{}x{}/{}",
                config.display_width, config.display_height, config.display_dpi
            )
        );
    }

    #[test]
    fn expands_tilde() {
        let home = Path::new("/home/tester");
        assert_eq!(
            expand_path(Path::new("~/Android/Sdk"), home),
            home.join("Android/Sdk")
        );
        assert_eq!(
            expand_path(Path::new("/opt/sdk"), home),
            PathBuf::from("/opt/sdk")
        );
    }

    #[test]
    fn toml_uses_defaults_for_missing_fields() {
        let config: Config = toml::from_str(r#"avd_name = "prueba""#).unwrap();
        assert_eq!(config.avd_name, "prueba");
        assert_eq!(config.emulator_port, 5556);
        assert_eq!(config.adb_port, 5038);
        assert_eq!(config.display_dpi, 240);
    }
}
