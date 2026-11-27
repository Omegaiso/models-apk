use std::collections::HashMap;
use std::path::PathBuf;

use apkstart_app::SyncReport;
use apkstart_core::{App, Config, Db, RuntimeStatus};
use apkstart_engine::{AndroidEmulator, ScrcpyOptions, ScrcpySession};

/// Estado compartido del daemon.
///
/// Guarda la configuración y las sesiones scrcpy abiertas por package. Las
/// operaciones del emulador (arranque, boot, parada) son largas y se ejecutan
/// sin tomar el lock de sesiones para no bloquear otras llamadas.
pub struct Manager {
    config: Config,
    launcher: PathBuf,
    sessions: tokio::sync::Mutex<HashMap<String, ScrcpySession>>,
    operation: tokio::sync::Mutex<()>,
}

impl Manager {
    pub fn new(config: Config) -> Self {
        let launcher = std::env::current_exe()
            .ok()
            .map(|exe| exe.with_file_name("apkstart"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("apkstart"));
        Self {
            config,
            launcher,
            sessions: tokio::sync::Mutex::new(HashMap::new()),
            operation: tokio::sync::Mutex::new(()),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Estado actual del runtime Android.
    pub async fn runtime_status(&self) -> RuntimeStatus {
        AndroidEmulator::new(self.config.clone()).status().await
    }

    /// Arranca el emulador y espera el boot completo. Operación larga.
    pub async fn start_runtime(&self) -> Result<RuntimeStatus, String> {
        let _operation = self.operation.lock().await;
        let emulator = AndroidEmulator::new(self.config.clone());
        emulator.start().await.map_err(|error| error.to_string())?;
        Ok(self.runtime_status().await)
    }

    /// Detiene el emulador; las sesiones scrcpy mueren al perder ADB.
    pub async fn stop_runtime(&self) -> Result<RuntimeStatus, String> {
        let _operation = self.operation.lock().await;
        let emulator = AndroidEmulator::new(self.config.clone());
        emulator.stop().await.map_err(|error| error.to_string())?;
        let mut sessions = self.sessions.lock().await;
        for (_, mut session) in sessions.drain() {
            let _ = session.kill().await;
        }
        drop(sessions);
        Ok(self.runtime_status().await)
    }

    /// Apps registradas en SQLite.
    pub async fn list_apps(&self) -> Result<Vec<App>, String> {
        let db = Db::open(&self.config.db_path()).map_err(|error| error.to_string())?;
        db.list_apps().map_err(|error| error.to_string())
    }

    /// Instala un APK y opcionalmente abre su ventana.
    pub async fn install(&self, path: &str, launch_after: bool) -> Result<String, String> {
        let _operation = self.operation.lock().await;
        let installed = apkstart_app::install_apk(
            &self.config,
            std::path::Path::new(path),
            &self.launcher,
            true,
        )
        .await
        .map_err(|error| format!("{error:#}"))?;
        let package = installed.app.package;
        if launch_after {
            self.launch(&package).await?;
        }
        Ok(package)
    }

    /// Sincroniza apps Android con el escritorio. No falla si el runtime está apagado.
    pub async fn sync(&self, refresh: bool) -> Result<SyncReport, String> {
        let _operation = self.operation.lock().await;
        let emulator = AndroidEmulator::new(self.config.clone());
        if !emulator.is_online().await {
            return Ok(SyncReport::default());
        }
        apkstart_app::sync_apps(&self.config, refresh, true, &self.launcher)
            .await
            .map_err(|error| format!("{error:#}"))
    }

    /// Abre la ventana scrcpy de un package y devuelve el pid de scrcpy.
    ///
    /// Si ya hay una sesión viva devuelve su pid sin abrir otra. Si la sesión
    /// guardada murió (pid reaped por tokio), se reemplaza con una nueva.
    pub async fn launch(&self, package: &str) -> Result<u32, String> {
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(pid) = sessions.get(package).and_then(ScrcpySession::pid) {
                return Ok(pid);
            }
            // Entrada muerta (o ausente): se descarta y se relanza abajo.
            sessions.remove(package);
        }

        // Título de ventana: label registrado; si no hay registro, el package.
        let title = Db::open(&self.config.db_path())
            .ok()
            .and_then(|db| db.get_app(package).ok().flatten())
            .map(|app| app.label)
            .unwrap_or_else(|| package.to_string());

        // Preparar el runtime puede tardar minutos en el primer arranque.
        let emulator = AndroidEmulator::new(self.config.clone());
        let adb = emulator
            .ensure_running()
            .await
            .map_err(|error| error.to_string())?;

        let options = ScrcpyOptions::from_config(&self.config, package, title);
        let mut session = ScrcpySession::spawn(&self.config, adb.serial(), &options)
            .await
            .map_err(|error| error.to_string())?;

        let Some(pid) = session.pid() else {
            let _ = session.kill().await;
            return Err("scrcpy se lanzó pero no reportó pid".to_string());
        };

        let mut sessions = self.sessions.lock().await;
        match sessions.get(package).and_then(ScrcpySession::pid) {
            // Otra llamada lanzó la misma app mientras preparábamos esta
            // sesión: descartamos la duplicada y devolvemos el pid vivo.
            Some(existing_pid) => {
                let _ = session.kill().await;
                tracing::warn!(package = %package, pid = existing_pid, "sesión duplicada");
                Ok(existing_pid)
            }
            None => {
                sessions.insert(package.to_string(), session);
                tracing::info!(package = %package, pid, "sesión scrcpy abierta");
                Ok(pid)
            }
        }
    }

    /// Cierra la sesión scrcpy de un package.
    ///
    /// Devuelve `true` si existía una sesión y `false` si no la había.
    pub async fn stop_app(&self, package: &str) -> Result<bool, String> {
        let mut sessions = self.sessions.lock().await;
        let Some(mut session) = sessions.remove(package) else {
            return Ok(false);
        };
        drop(sessions);
        session.kill().await.map_err(|error| error.to_string())?;
        tracing::info!(package = %package, "sesión scrcpy cerrada");
        Ok(true)
    }
}
