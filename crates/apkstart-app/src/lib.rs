use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use apkstart_apk::{ApkInfo, read_apk, read_icon};
use apkstart_core::{App, Config, Db, DesktopPaths, now_unix};
use apkstart_engine::{Adb, AndroidEmulator};

#[derive(Debug, Clone)]
pub struct InstalledApp {
    pub info: ApkInfo,
    pub app: App,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncReport {
    pub added: usize,
    pub removed: usize,
}

/// Instala un APK, lo registra en SQLite y crea su entrada de escritorio.
pub async fn install_apk(
    config: &Config,
    apk: &Path,
    launcher: &Path,
    quiet: bool,
) -> Result<InstalledApp> {
    if !apk.is_file() {
        bail!("no existe el APK: {}", apk.display());
    }
    let info = read_apk(apk).with_context(|| format!("leyendo {}", apk.display()))?;

    let emulator = AndroidEmulator::new(config.clone());
    let adb = emulator
        .ensure_running()
        .await
        .context("preparando el runtime Android")?;
    adb.install(apk).await.context("instalando el APK")?;

    let app = register_app_from_apk(config, apk, &info, launcher, quiet)?;
    apkstart_core::desktop::refresh_caches(&DesktopPaths::default());
    Ok(InstalledApp { info, app })
}

pub fn register_app_from_apk(
    config: &Config,
    apk: &Path,
    info: &ApkInfo,
    launcher: &Path,
    quiet: bool,
) -> Result<App> {
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
    let paths = DesktopPaths::default();
    match apkstart_core::desktop::install(&app, icon.as_deref(), launcher, &paths) {
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

/// Sincroniza las apps Android `-3` con SQLite y el escritorio.
///
/// Si Android no reporta paquetes pero sí hay apps conocidas, no borra nada:
/// evita perder entradas por un fallo transitorio de ADB.
pub async fn sync_apps(
    config: &Config,
    refresh: bool,
    quiet: bool,
    launcher: &Path,
) -> Result<SyncReport> {
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
        match register_app_from_apk(config, &local, &info, launcher, quiet) {
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
