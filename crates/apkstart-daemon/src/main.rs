//! `apkstartd`: daemon D-Bus de ApkStart.
//!
//! Expone `org.apkstart.Manager1` en el bus de sesión con el estado del
//! runtime Android y el control de las ventanas scrcpy. Fase mínima: se lanza
//! a mano y todavía no instala unidades de systemd.

use std::sync::Arc;
use std::time::Duration;

use apkstart_core::Config;
use apkstart_daemon::dbus::Daemon;
use apkstart_daemon::manager::Manager;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    let manager = Arc::new(Manager::new(config));
    let daemon = Daemon::new(Arc::clone(&manager));

    let sync_manager = Arc::clone(&manager);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(error) = sync_manager.sync(false).await {
                tracing::warn!(%error, "sync automático falló");
            }
        }
    });

    // Sin esta referencia la conexión (y el nombre) se liberan al salir de aquí.
    let _connection = zbus::connection::Builder::session()
        .name("org.apkstart.Manager1")
        .serve_at("/org/apkstart/Manager1", daemon)
        .build()
        .await?;

    tracing::info!("apkstartd escuchando en org.apkstart.Manager1 (bus de sesión)");

    tokio::signal::ctrl_c().await?;
    tracing::info!("terminando");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
