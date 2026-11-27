use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("ADB: {0}")]
    Adb(String),
    #[error("emulador: {0}")]
    Emulator(String),
    #[error("scrcpy: {0}")]
    Scrcpy(String),
    #[error("tiempo de espera agotado: {0}")]
    Timeout(String),
    #[error("configuración: {0}")]
    Config(String),
    #[error("binario no encontrado: {}", .0.display())]
    MissingBinary(PathBuf),
    #[error("KVM no disponible: {0}")]
    Kvm(String),
}

pub type Result<T> = std::result::Result<T, Error>;
