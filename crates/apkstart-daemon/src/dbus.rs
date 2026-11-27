use std::sync::Arc;

use apkstart_core::{App, RuntimeStatus};
use zbus::fdo;

use crate::manager::Manager;

/// Interfaz D-Bus `org.apkstart.Manager1`.
///
/// Fase mínima: los valores compuestos viajan como JSON embebido en strings
/// (`s`) y los errores se mapean a `org.freedesktop.DBus.Error.Failed`.
pub struct Daemon {
    manager: Arc<Manager>,
}

impl Daemon {
    pub fn new(manager: Arc<Manager>) -> Self {
        Self { manager }
    }
}

#[zbus::interface(name = "org.apkstart.Manager1")]
impl Daemon {
    /// Versión del crate del daemon.
    async fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Estado del runtime Android como JSON de `RuntimeStatus`.
    async fn get_runtime_status(&self) -> String {
        status_json(&self.manager.runtime_status().await)
    }

    /// Arranca el emulador (operación larga) y devuelve el estado como JSON.
    async fn start_runtime(&self) -> Result<String, fdo::Error> {
        let status = self.manager.start_runtime().await.map_err(failed)?;
        Ok(status_json(&status))
    }

    /// Detiene el emulador y devuelve el estado como JSON.
    async fn stop_runtime(&self) -> Result<String, fdo::Error> {
        let status = self.manager.stop_runtime().await.map_err(failed)?;
        Ok(status_json(&status))
    }

    /// Apps registradas como JSON array de `App`.
    async fn list_apps(&self) -> Result<String, fdo::Error> {
        let apps = self.manager.list_apps().await.map_err(failed)?;
        apps_json(&apps)
    }

    /// Instala un APK; devuelve el package instalado.
    async fn install(&self, path: String, launch: bool) -> Result<String, fdo::Error> {
        self.manager.install(&path, launch).await.map_err(failed)
    }

    /// Sincroniza apps Android; devuelve (añadidas, eliminadas).
    async fn sync(&self, refresh: bool) -> Result<(u32, u32), fdo::Error> {
        let report = self.manager.sync(refresh).await.map_err(failed)?;
        Ok((report.added as u32, report.removed as u32))
    }

    /// Abre la ventana de un package; devuelve el pid de scrcpy.
    async fn launch(&self, package: String) -> Result<u32, fdo::Error> {
        self.manager.launch(&package).await.map_err(failed)
    }

    /// Cierra la sesión scrcpy de un package; `true` si existía.
    async fn stop_app(&self, package: String) -> Result<bool, fdo::Error> {
        self.manager.stop_app(&package).await.map_err(failed)
    }
}

/// Estado del runtime como JSON compacto (`RuntimeStatus` no puede fallar).
fn status_json(status: &RuntimeStatus) -> String {
    match serde_json::to_string(status) {
        Ok(json) => json,
        Err(_) => "{}".to_string(),
    }
}

/// Apps registradas como JSON array compacto.
fn apps_json(apps: &[App]) -> Result<String, fdo::Error> {
    serde_json::to_string(apps).map_err(|error| failed(error.to_string()))
}

/// Los errores del manager viajan como `org.freedesktop.DBus.Error.Failed`.
fn failed(error: String) -> fdo::Error {
    fdo::Error::Failed(error)
}
