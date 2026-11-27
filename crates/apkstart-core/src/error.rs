use std::path::PathBuf;

/// Error común de la capa core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuración inválida: {0}")]
    Config(String),
    #[error("error de TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("base de datos: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("estado inválido: {0}")]
    InvalidState(String),
    #[error("ruta no encontrada: {}", .0.display())]
    NotFound(PathBuf),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
