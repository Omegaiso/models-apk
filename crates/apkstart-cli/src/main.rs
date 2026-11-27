use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};
use apkstart_apk::{ApkInfo, read_apk, read_icon};
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
    /// Registra la integración con el escritorio y la asociación de APK.
    Setup,
    /// Sincroniza las apps instaladas dentro de Android con el escritorio Linux.
    Sync {
        /// Vuelve a registrar también las apps ya conocidas.
        #[arg(long)]
        refresh: bool,
        /// No imprime el resultado; para el timer de sincronización.
        #[arg(long)]
        quiet: bool,
        /// Sale sin error si el runtime está apagado.
        #[arg(long)]
        if_running: bool,
    },
    /// Gestiona el runtime Android headless.
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    /// Instala un APK y lo registra en la base de datos.
    Install {
        /// Ruta al archivo .apk.
        apk: PathBuf,
        /// Abre la app en su ventana al terminar la instalación.
        #[arg(long)]
        launch: bool,
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
        Command::Setup => setup(&config).await,
        Command::Sync {
            refresh,
            quiet,
            if_running,
        } => sync(&config, refresh, quiet, if_running).await,
        Command::Runtime { command } => runtime(&config, command).await,
        Command::Install { apk, launch } => install(&config, &apk, launch).await,
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

async fn setup(config: &Config) -> Result<()> {
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("apkstart"));
    let paths = DesktopPaths::default();
    let handler = apkstart_core::desktop::install_mime_handler(&executable, &paths)
        .context("creando el handler de APK")?;
    let registered = apkstart_core::desktop::register_mime_default();
    apkstart_core::desktop::refresh_caches(&paths);

    println!("Integración de escritorio instalada");
    println!("  handler: {}", handler.display());
    println!("  MIME:    {}", apkstart_core::desktop::APK_MIME);
    if registered {
        println!("  doble clic en un APK: listo");
    } else {
        println!("  aviso: no se pudo fijar el handler con xdg-mime; revisa xdg-utils");
    }

    match install_sync_timer(&executable) {
        Ok(true) => println!("  sincronización automática: timer de 15 s activo"),
        _ => println!("  aviso: no se pudo activar el timer de sincronización de systemd --user"),
    }

    let emulator = AndroidEmulator::new(config.clone());
    if emulator.is_online().await {
        match sync_apps(config, false, false).await {
            Ok(report) if report.added > 0 || report.removed > 0 => println!(
                "Apps añadidas: {}; eliminadas: {}",
                report.added, report.removed
            ),
            Ok(_) => println!("No hay apps Android nuevas que registrar."),
            Err(error) => eprintln!("aviso: no se pudieron sincronizar las apps: {error:#}"),
        }
    } else {
        println!("Runtime apagado; ejecuta `apkstart sync` con Android encendido.");
    }

    println!();
    println!("Las apps instaladas aparecen en el menú y en el escritorio con su icono.");
    println!("Para ejecutar APKs como binarios (./app.apk), desde el repo:");
    println!(
        "  sudo packaging/binfmt/install.sh {}",
        executable.display()
    );
    println!("  chmod +x app.apk && ./app.apk");
    Ok(())
}

fn icon_for_apk(apk: &Path, info: &ApkInfo) -> Option<Vec<u8>> {
    match read_icon(apk, info) {
        Ok(Some(icon)) => return Some(icon),
        Ok(None) => {}
        Err(error) => eprintln!("aviso: no se pudo extraer el icono ({error})"),
    }
    match apkstart_core::desktop::fallback_icon(&info.package, &info.label) {
        Ok(icon) => Some(icon),
        Err(error) => {
            eprintln!("aviso: no se pudo generar el icono de respaldo ({error})");
            None
        }
    }
}

fn install_sync_timer(executable: &Path) -> Result<bool> {
    let Some(config_dir) = dirs::config_dir() else {
        return Ok(false);
    };
    let unit_dir = config_dir.join("systemd/user");
    fs::create_dir_all(&unit_dir)?;
    let exec = systemd_quote(&executable.to_string_lossy());
    let service = format!(
        "[Unit]\n\
         Description=Sincronizar apps Android instaladas (models-apk)\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart={exec} sync --if-running --quiet\n"
    );
    let timer = "[Unit]\n\
                 Description=Sincronizar apps Android instaladas (models-apk)\n\
                 \n\
                 [Timer]\n\
                 OnBootSec=20s\n\
                 OnUnitActiveSec=15s\n\
                 AccuracySec=2s\n\
                 Unit=apkstart-sync.service\n\
                 \n\
                 [Install]\n\
                 WantedBy=timers.target\n";
    fs::write(unit_dir.join("apkstart-sync.service"), service)?;
    fs::write(unit_dir.join("apkstart-sync.timer"), timer)?;

    let reload = ProcessCommand::new("systemctl")
        .args(["--user", "daemon-reload"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !reload {
        return Ok(false);
    }
    let enabled = ProcessCommand::new("systemctl")
        .args(["--user", "enable", "--now", "apkstart-sync.timer"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    Ok(enabled)
}

fn systemd_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
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
            match sync_apps(config, false, false).await {
                Ok(report) if report.added > 0 || report.removed > 0 => {
                    println!(
                        "Apps actualizadas en el escritorio: +{} / -{}",
                        report.added, report.removed
                    );
                }
                Ok(_) => {}
                Err(error) => eprintln!("aviso: no se pudieron sincronizar las apps: {error:#}"),
            }
        }
        RuntimeCommand::Stop => {
            emulator.stop().await.context("deteniendo el runtime")?;
            println!("Runtime detenido");
        }
    }
    Ok(())
}

async fn install(config: &Config, apk: &Path, launch_after: bool) -> Result<()> {
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

    let app = register_app_from_apk(config, apk, &info, false)?;
    apkstart_core::desktop::refresh_caches(&DesktopPaths::default());

    println!("Instalado: {}", app.package);
    println!("Lánzalo con: apkstart launch {}", app.package);

    if launch_after {
        launch(config, app.package, Some(app.label), None, None).await?;
    }
    Ok(())
}

fn register_app_from_apk(config: &Config, apk: &Path, info: &ApkInfo, quiet: bool) -> Result<App> {
    let db = Db::open(&config.db_path()).context("abriendo la base de datos")?;
    let previous = db.get_app(&info.package).ok().flatten();
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
        categories: info.categories.clone(),
        orientation: None,
        installed_at: previous
            .as_ref()
            .map(|app| app.installed_at)
            .unwrap_or_else(now_unix),
        last_launched_at: previous.as_ref().and_then(|app| app.last_launched_at),
        enabled: true,
    };

    let icon = icon_for_apk(apk, info);
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("apkstart"));
    let paths = DesktopPaths::default();
    match apkstart_core::desktop::install(&app, icon.as_deref(), &executable, &paths) {
        Ok(entry) => {
            app.icon_path = entry.icon_file;
            if !quiet {
                println!("Acceso de escritorio: {}", entry.desktop_file.display());
            }
        }
        Err(error) => {
            eprintln!("aviso: no se pudo crear la entrada de escritorio: {error}");
        }
    }

    db.upsert_app(&app).context("registrando la app")?;
    Ok(app)
}

#[derive(Debug, Default)]
struct SyncReport {
    added: usize,
    removed: usize,
}

async fn sync(config: &Config, refresh: bool, quiet: bool, if_running: bool) -> Result<()> {
    let emulator = AndroidEmulator::new(config.clone());
    if !emulator.is_online().await {
        if if_running {
            return Ok(());
        }
        bail!("runtime detenido; arranca con `apkstart runtime start`");
    }
    let report = sync_apps(config, refresh, quiet).await?;
    if !quiet {
        println!(
            "Apps añadidas: {}; eliminadas: {}",
            report.added, report.removed
        );
    }
    Ok(())
}

async fn sync_apps(config: &Config, refresh: bool, quiet: bool) -> Result<SyncReport> {
    let adb = Adb::new(config);
    let packages = adb
        .list_packages(true)
        .await
        .context("listando apps de Android")?;
    let installed = packages.iter().cloned().collect::<HashSet<_>>();
    let db = Db::open(&config.db_path()).context("abriendo la base de datos")?;
    let known_apps = db.list_apps()?;
    if packages.is_empty() && !known_apps.is_empty() {
        let total = adb
            .shell(&["pm", "list", "packages"])
            .await
            .unwrap_or_default();
        if total.is_empty() {
            eprintln!("aviso: Android no reportó paquetes; no se borran entradas");
            return Ok(SyncReport::default());
        }
    }
    let known = known_apps
        .iter()
        .map(|app| app.package.clone())
        .collect::<HashSet<_>>();
    let mut report = SyncReport::default();

    for package in packages {
        if !refresh && known.contains(&package) {
            continue;
        }
        let remote = match adb.package_apk_path(&package).await {
            Ok(path) => path,
            Err(error) => {
                eprintln!("aviso: no se pudo localizar el APK de {package}: {error}");
                continue;
            }
        };
        let local = config.data_dir.join("cache").join(format!("{package}.apk"));
        if let Err(error) = adb.pull(&remote, &local).await {
            eprintln!("aviso: no se pudo extraer {package}: {error}");
            continue;
        }
        let info = match read_apk(&local) {
            Ok(info) => info,
            Err(error) => {
                eprintln!("aviso: APK inválido para {package}: {error}");
                continue;
            }
        };
        match register_app_from_apk(config, &local, &info, quiet) {
            Ok(app) => {
                if !quiet {
                    println!("Registrada: {} ({})", app.label, app.package);
                }
                report.added += 1;
            }
            Err(error) => eprintln!("aviso: no se pudo registrar {package}: {error:#}"),
        }
    }

    for app in known_apps {
        if installed.contains(&app.package) {
            continue;
        }
        let paths = DesktopPaths::default();
        if let Err(error) = apkstart_core::desktop::remove(&app.package, &paths) {
            eprintln!(
                "aviso: no se pudo borrar la entrada de {}: {error}",
                app.package
            );
            continue;
        }
        db.delete_app(&app.package)
            .context("borrando una app desinstalada")?;
        if !quiet {
            println!("Eliminada del escritorio: {} ({})", app.label, app.package);
        }
        report.removed += 1;
    }

    if report.added > 0 || report.removed > 0 {
        apkstart_core::desktop::refresh_caches(&DesktopPaths::default());
    }
    Ok(report)
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
        let paths = DesktopPaths::default();
        apkstart_core::desktop::remove(package, &paths)
            .context("borrando la entrada de escritorio")?;
        apkstart_core::desktop::refresh_caches(&paths);
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
    fn cli_parses_setup() {
        let cli = Cli::try_parse_from(["apkstart", "setup"]).unwrap();
        assert!(matches!(cli.command, Command::Setup));
    }

    #[test]
    fn cli_parses_install_launch() {
        let cli = Cli::try_parse_from(["apkstart", "install", "--launch", "/tmp/app.apk"]).unwrap();
        match cli.command {
            Command::Install { apk, launch } => {
                assert_eq!(apk, PathBuf::from("/tmp/app.apk"));
                assert!(launch);
            }
            _ => panic!("comando inesperado"),
        }
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
