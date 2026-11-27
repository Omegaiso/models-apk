use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use apkstart_apk::{read_apk, read_resource};
use apkstart_core::{App, Config, Db, DesktopPaths, now_unix};
use apkstart_engine::{Adb, AndroidEmulator, Check, ScrcpyOptions, ScrcpySession};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "apkstart",
    version,
    about = "Gestor de aplicaciones Android para Linux",
    long_about = "ApkStart arranca Android en un emulador headless y abre cada aplicación en su propia ventana Wayland."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Comprueba KVM, SDK, emulador, scrcpy y la sesión de escritorio.
    Doctor,
    /// Gestiona el runtime Android headless.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Instala un APK y lo registra en la base de datos.
    Install {
        /// Ruta al archivo .apk.
        apk: PathBuf,
    },
    /// Lanza una app instalada en su propia ventana.
    Launch {
        /// Package Android, por ejemplo org.fdroid.fdroid.
        package: String,
        /// Título de ventana; por defecto, el label del APK registrado.
        #[arg(long)]
        title: Option<String>,
        /// Tamaño inicial en píxeles, por ejemplo 900x1600.
        #[arg(long)]
        size: Option<String>,
        /// Densidad del display virtual; por defecto, la de la configuración.
        #[arg(long)]
        dpi: Option<u32>,
    },
    /// Lista las apps registradas o, si no hay registro, las instaladas en Android.
    List {
        #[arg(long)]
        json: bool,
        /// Incluye paquetes del sistema al consultar Android.
        #[arg(long)]
        all: bool,
    },
    /// Desinstala una app de Android y de la base de datos.
    Uninstall {
        package: String,
        #[arg(long)]
        keep_data: bool,
    },
    /// Muestra la ruta de configuración y el estado de la UI de ajustes.
    Settings,
    /// Exporta logs y diagnóstico a un directorio.
    ExportLog,
}

#[derive(Debug, Subcommand)]
enum RuntimeCommand {
    /// Estado actual del emulador.
    Status,
    /// Arranca el emulador y espera a que Android termine de bootear.
    Start,
    /// Detiene el emulador.
    Stop,
}

#[tokio::main]
async fn main() {
    init_tracing();
    let cli = Cli::parse();
    if let Err(error) = run(cli).await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = Config::load().context("cargando configuración")?;
    match cli.command {
        Command::Doctor => doctor(&config).await,
        Command::Runtime { command } => runtime(&config, command).await,
        Command::Install { apk } => install(&config, &apk).await,
        Command::Launch {
            package,
            title,
            size,
            dpi,
        } => launch(&config, package, title, size, dpi).await,
        Command::List { json, all } => list(&config, json, all).await,
        Command::Uninstall { package, keep_data } => uninstall(&config, &package, keep_data).await,
        Command::Settings => settings(&config),
        Command::ExportLog => export_log(&config).await,
    }
}

async fn doctor(config: &Config) -> Result<()> {
    let emulator = AndroidEmulator::new(config.clone());
    let checks = emulator.checks();
    println!("ApkStart doctor");
    println!("  configuración: {}", Config::config_path().display());
    println!("  SDK:           {}", config.sdk_root.display());
    println!();
    let mut critical_failure = false;
    for check in &checks {
        print_check(check);
        critical_failure |= check.critical && !check.ok;
    }

    let status = emulator.status().await;
    println!();
    print_runtime_status(&status);
    if critical_failure {
        println!("\nFaltan requisitos críticos. Corrige los [FAIL] antes de continuar.");
        std::process::exit(1);
    }
    Ok(())
}

fn print_check(check: &Check) {
    let tag = if check.ok {
        "[OK]  "
    } else if check.critical {
        "[FAIL]"
    } else {
        "[WARN]"
    };
    println!("{tag} {:<16} {}", check.name, check.detail);
}

fn print_runtime_status(status: &apkstart_core::RuntimeStatus) {
    println!(
        "Runtime: {}  serial={}  boot_completed={}",
        status.state.as_str(),
        status.serial.as_deref().unwrap_or("-"),
        status.boot_completed
    );
    if let Some(message) = &status.message {
        println!("  {message}");
    }
}

async fn runtime(config: &Config, command: RuntimeCommand) -> Result<()> {
    let emulator = AndroidEmulator::new(config.clone());
    match command {
        RuntimeCommand::Status => {
            let status = emulator.status().await;
            print_runtime_status(&status);
            if status.boot_completed {
                let adb = Adb::new(config);
                let packages = adb.list_packages(true).await.unwrap_or_default();
                println!("Apps de terceros instaladas: {}", packages.len());
            }
        }
        RuntimeCommand::Start => {
            println!("Arrancando Android headless (esto puede tardar)...");
            emulator.start().await.context("arrancando el runtime")?;
            println!("Runtime listo: {}", config.serial());
            println!("Log del emulador: {}", config.emulator_log_path().display());
        }
        RuntimeCommand::Stop => {
            emulator.stop().await.context("deteniendo el runtime")?;
            println!("Runtime detenido");
        }
    }
    Ok(())
}

async fn install(config: &Config, apk: &Path) -> Result<()> {
    if !apk.is_file() {
        bail!("no existe el APK: {}", apk.display());
    }
    let info = read_apk(apk).with_context(|| format!("leyendo {}", apk.display()))?;
    println!(
        "APK: {} ({}) {}",
        info.label,
        info.package,
        info.version_name.as_deref().unwrap_or("sin versión")
    );
    if let Some(activity) = &info.main_activity {
        println!("  activity: {activity}");
    }
    if !info.abis.is_empty() {
        println!("  ABIs: {}", info.abis.join(", "));
    }
    if let Some(min_sdk) = info.min_sdk {
        println!("  minSdk: {min_sdk}");
    }

    let emulator = AndroidEmulator::new(config.clone());
    let adb = emulator
        .ensure_running()
        .await
        .context("preparando el runtime Android")?;
    adb.install(apk).await.context("instalando el APK")?;

    let db = Db::open(&config.db_path()).context("abriendo la base de datos")?;
    let mut app = App {
        package: info.package.clone(),
        label: info.label.clone(),
        version_name: info.version_name.clone(),
        version_code: info.version_code,
        activity: info.main_activity.clone(),
        icon_path: None,
        apk_path: Some(apk.canonicalize().unwrap_or_else(|_| apk.to_path_buf())),
        min_sdk: info.min_sdk,
        target_sdk: info.target_sdk,
        abis: info.abis.clone(),
        orientation: None,
        installed_at: now_unix(),
        last_launched_at: None,
        enabled: true,
    };

    let icon =
        info.icon_resource
            .as_deref()
            .and_then(|resource| match read_resource(apk, resource) {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    eprintln!("aviso: no se pudo extraer el icono ({error})");
                    None
                }
            });
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("apkstart"));
    let paths = DesktopPaths::default();
    match apkstart_core::desktop::install(&app, icon.as_deref(), &executable, &paths) {
        Ok(entry) => {
            app.icon_path = entry.icon_file.clone();
            println!("Acceso de escritorio: {}", entry.desktop_file.display());
        }
        Err(error) => {
            eprintln!("aviso: no se pudo crear la entrada de escritorio: {error}");
        }
    }

    db.upsert_app(&app).context("registrando la app")?;

    println!("Instalado: {}", info.package);
    println!("Lánzalo con: apkstart launch {}", info.package);
    Ok(())
}

async fn launch(
    config: &Config,
    package: String,
    title: Option<String>,
    size: Option<String>,
    dpi: Option<u32>,
) -> Result<()> {
    let db = Db::open(&config.db_path()).ok();
    let app = db
        .as_ref()
        .and_then(|db| db.get_app(&package).ok().flatten());
    let window_title = title
        .or_else(|| app.as_ref().map(|app| app.label.clone()))
        .unwrap_or_else(|| package.clone());

    let (mut width, mut height) = (config.display_width, config.display_height);
    if let Some(size) = size {
        let (w, h) = parse_size(&size)?;
        width = w;
        height = h;
    }

    let emulator = AndroidEmulator::new(config.clone());
    let adb = emulator
        .ensure_running()
        .await
        .context("preparando el runtime Android")?;

    let options = ScrcpyOptions {
        package: package.clone(),
        title: window_title,
        width,
        height,
        dpi: dpi.unwrap_or(config.display_dpi),
    };
    let session = ScrcpySession::spawn(config, adb.serial(), &options)
        .await
        .context("lanzando la ventana de la app")?;

    if let Some(db) = &db {
        let _ = db.touch_launched(&package, now_unix());
    }

    println!(
        "Ventana lanzada: {} (pid={}, log={})",
        package,
        session
            .pid()
            .map_or_else(|| "-".to_string(), |pid| pid.to_string()),
        session.log_path().display()
    );
    Ok(())
}

async fn list(config: &Config, json: bool, all: bool) -> Result<()> {
    let apps = Db::open(&config.db_path())
        .ok()
        .and_then(|db| db.list_apps().ok())
        .unwrap_or_default();

    if !apps.is_empty() {
        if json {
            println!("{}", serde_json::to_string_pretty(&apps)?);
        } else {
            println!(
                "{:<40} {:<28} {:<10} ACTIVITY",
                "PACKAGE", "LABEL", "VERSIÓN"
            );
            for app in apps {
                println!(
                    "{:<40} {:<28} {:<10} {}",
                    app.package,
                    app.label,
                    app.version_name.as_deref().unwrap_or("-"),
                    app.activity.as_deref().unwrap_or("-")
                );
            }
        }
        return Ok(());
    }

    let emulator = AndroidEmulator::new(config.clone());
    if !emulator.is_online().await {
        println!("No hay apps registradas y el runtime está detenido.");
        return Ok(());
    }
    let adb = Adb::new(config);
    let packages = adb
        .list_packages(!all)
        .await
        .context("listando paquetes de Android")?;
    if json {
        let values = packages
            .into_iter()
            .map(|package| serde_json::json!({ "package": package }))
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&values)?);
    } else {
        for package in packages {
            println!("{package}");
        }
    }
    Ok(())
}

async fn uninstall(config: &Config, package: &str, keep_data: bool) -> Result<()> {
    let emulator = AndroidEmulator::new(config.clone());
    if emulator.is_online().await {
        let adb = Adb::new(config);
        adb.uninstall(package, keep_data)
            .await
            .context("desinstalando de Android")?;
    }
    if !keep_data {
        if let Ok(db) = Db::open(&config.db_path()) {
            db.delete_app(package)
                .context("borrando de la base de datos")?;
        }
        apkstart_core::desktop::remove(package, &DesktopPaths::default())
            .context("borrando la entrada de escritorio")?;
    }
    println!("Desinstalado: {package}");
    Ok(())
}

fn settings(config: &Config) -> Result<()> {
    println!("Configuración: {}", Config::config_path().display());
    println!("SDK:           {}", config.sdk_root.display());
    println!("AVD:           {}", config.avd_name);
    println!("Runtime:       {}", config.serial());
    println!("Display:       {}", config.display_spec());
    println!("Datos:         {}", config.db_path().display());
    println!(
        "Logs:          {}",
        config.state_dir.join("sessions").display()
    );
    println!();
    println!("La UI GTK4/libadwaita está pendiente; de momento usa `apkstart doctor` y `runtime`.");
    Ok(())
}

async fn export_log(config: &Config) -> Result<()> {
    let destination = config.state_dir.join(format!("diagnostics-{}", now_unix()));
    fs::create_dir_all(&destination)?;

    let emulator_log = config.emulator_log_path();
    if emulator_log.is_file() {
        fs::copy(&emulator_log, destination.join("emulator.log"))?;
    }

    let sessions = config.state_dir.join("sessions");
    if sessions.is_dir() {
        let target = destination.join("sessions");
        fs::create_dir_all(&target)?;
        for entry in fs::read_dir(sessions)? {
            let entry = entry?;
            if entry.path().is_file() {
                let name = entry.file_name();
                fs::copy(entry.path(), target.join(name))?;
            }
        }
    }

    let emulator = AndroidEmulator::new(config.clone());
    let mut report = String::new();
    report.push_str(&format!("config: {}\n", Config::config_path().display()));
    report.push_str(&format!("sdk: {}\n", config.sdk_root.display()));
    report.push_str(&format!("serial: {}\n", config.serial()));
    report.push_str(&format!("display: {}\n\n", config.display_spec()));
    for check in emulator.checks() {
        report.push_str(&format!(
            "[{}] {}: {}\n",
            if check.ok { "OK" } else { "FAIL" },
            check.name,
            check.detail
        ));
    }
    fs::write(destination.join("doctor.txt"), report)?;

    println!("Diagnóstico exportado en {}", destination.display());
    Ok(())
}

fn parse_size(value: &str) -> Result<(u32, u32)> {
    let (width, height) = value
        .split_once('x')
        .or_else(|| value.split_once('X'))
        .with_context(|| format!("tamaño inválido `{value}`; usa WxH, por ejemplo 900x1600"))?;
    let width = width
        .parse::<u32>()
        .with_context(|| format!("ancho inválido en `{value}`"))?;
    let height = height
        .parse::<u32>()
        .with_context(|| format!("alto inválido en `{value}`"))?;
    if width < 320 || height < 480 {
        bail!("tamaño demasiado pequeño: {width}x{height}");
    }
    Ok((width, height))
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_size() {
        assert_eq!(parse_size("900x1600").unwrap(), (900, 1600));
        assert_eq!(parse_size("720X1280").unwrap(), (720, 1280));
    }

    #[test]
    fn rejects_invalid_size() {
        assert!(parse_size("900").is_err());
        assert!(parse_size("900x").is_err());
        assert!(parse_size("100x100").is_err());
    }

    #[test]
    fn cli_parses_doctor() {
        let cli = Cli::try_parse_from(["apkstart", "doctor"]).unwrap();
        assert!(matches!(cli.command, Command::Doctor));
    }

    #[test]
    fn cli_parses_launch_with_size() {
        let cli = Cli::try_parse_from([
            "apkstart",
            "launch",
            "org.fdroid.fdroid",
            "--size",
            "900x1600",
            "--dpi",
            "240",
        ])
        .unwrap();
        match cli.command {
            Command::Launch {
                package, size, dpi, ..
            } => {
                assert_eq!(package, "org.fdroid.fdroid");
                assert_eq!(size.as_deref(), Some("900x1600"));
                assert_eq!(dpi, Some(240));
            }
            _ => panic!("comando inesperado"),
        }
    }
}
