pub mod config;
pub mod db;
pub mod desktop;
pub mod error;
pub mod model;

pub use config::Config;
pub use db::Db;
pub use desktop::{DesktopEntry, DesktopPaths, app_id};
pub use error::{Error, Result};
pub use model::{App, RuntimeState, RuntimeStatus, WindowState, now_unix};
