use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Aplicación Android registrada por ApkStart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    pub package: String,
    pub label: String,
    pub version_name: Option<String>,
    pub version_code: i64,
    pub activity: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub apk_path: Option<PathBuf>,
    pub min_sdk: Option<i64>,
    pub target_sdk: Option<i64>,
    pub abis: Vec<String>,
    /// Categorías freedesktop para colocar la app en el menú del escritorio.
    pub categories: Vec<String>,
    pub orientation: Option<String>,
    pub installed_at: i64,
    pub last_launched_at: Option<i64>,
    pub enabled: bool,
}

/// Estado de ventana persistido. En Wayland no se guarda la posición absoluta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub package: String,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub fullscreen: bool,
    pub output: Option<String>,
    pub scale: f64,
    pub updated_at: i64,
}

impl WindowState {
    pub fn new(package: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            package: package.into(),
            width,
            height,
            maximized: false,
            fullscreen: false,
            output: None,
            scale: 1.0,
            updated_at: now_unix(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Failed,
}

impl RuntimeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub state: RuntimeState,
    pub serial: Option<String>,
    pub boot_completed: bool,
    pub message: Option<String>,
}

impl RuntimeStatus {
    pub fn stopped(serial: Option<String>) -> Self {
        Self {
            state: RuntimeState::Stopped,
            serial,
            boot_completed: false,
            message: None,
        }
    }

    pub fn running(serial: String) -> Self {
        Self {
            state: RuntimeState::Running,
            serial: Some(serial),
            boot_completed: true,
            message: None,
        }
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_positive() {
        assert!(now_unix() > 1_700_000_000);
    }
}
