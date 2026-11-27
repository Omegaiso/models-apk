use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::model::App;

/// Rutas XDG usadas para integrar las apps con el escritorio.
#[derive(Debug, Clone)]
pub struct DesktopPaths {
    pub data_home: PathBuf,
}

impl Default for DesktopPaths {
    fn default() -> Self {
        Self {
            data_home: dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

impl DesktopPaths {
    pub fn applications_dir(&self) -> PathBuf {
        self.data_home.join("applications")
    }

    pub fn hicolor_256_dir(&self) -> PathBuf {
        self.data_home.join("icons/hicolor/256x256/apps")
    }

    pub fn hicolor_root(&self) -> PathBuf {
        self.data_home.join("icons/hicolor")
    }
}

#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub app_id: String,
    pub desktop_file: PathBuf,
    pub icon_file: Option<PathBuf>,
}

/// Identidad Wayland/X11 de la ventana de una app.
pub fn app_id(package: &str) -> String {
    format!("io.github.apkstart.{package}")
}

pub fn install(
    app: &App,
    icon: Option<&[u8]>,
    executable: &Path,
    paths: &DesktopPaths,
) -> Result<DesktopEntry> {
    let app_id = app_id(&app.package);
    let icon_file = match icon {
        Some(bytes) => write_icon(bytes, &app_id, paths)?,
        None => None,
    };
    let desktop_file = write_desktop_entry(app, &app_id, icon_file.is_some(), executable, paths)?;
    refresh_icon_cache(paths);
    Ok(DesktopEntry {
        app_id,
        desktop_file,
        icon_file,
    })
}

pub fn remove(package: &str, paths: &DesktopPaths) -> Result<()> {
    let app_id = app_id(package);
    remove_if_exists(&paths.applications_dir().join(format!("{app_id}.desktop")))?;
    for extension in ["png", "webp", "jpg", "jpeg"] {
        remove_if_exists(
            &paths
                .hicolor_256_dir()
                .join(format!("{app_id}.{extension}")),
        )?;
    }
    refresh_icon_cache(paths);
    Ok(())
}

fn write_icon(bytes: &[u8], app_id: &str, paths: &DesktopPaths) -> Result<Option<PathBuf>> {
    let Some(extension) = image_extension(bytes) else {
        return Ok(None);
    };
    let dir = paths.hicolor_256_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{app_id}.{extension}"));
    fs::write(&path, bytes)?;
    Ok(Some(path))
}

fn write_desktop_entry(
    app: &App,
    app_id: &str,
    has_icon: bool,
    executable: &Path,
    paths: &DesktopPaths,
) -> Result<PathBuf> {
    let dir = paths.applications_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{app_id}.desktop"));
    let name = sanitize_value(&app.label);
    let exec = exec_line(executable, &app.package);
    let icon = if has_icon {
        app_id.to_string()
    } else {
        "application-x-executable".to_string()
    };
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment=Aplicación Android gestionada por ApkStart\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupNotify=true\n\
         StartupWMClass={app_id}\n\
         X-ApkStart-Package={package}\n",
        package = app.package
    );
    fs::write(&path, content)?;
    Ok(path)
}

fn exec_line(executable: &Path, package: &str) -> String {
    let path = escape_exec(&executable.to_string_lossy());
    format!("\"{path}\" launch {package}")
}

fn escape_exec(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn sanitize_value(value: &str) -> String {
    value.replace(['\n', '\r'], " ").trim().to_string()
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
        Some("webp")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("jpg")
    } else {
        None
    }
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn refresh_icon_cache(paths: &DesktopPaths) {
    let root = paths.hicolor_root();
    if !root.is_dir() {
        return;
    }
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .arg("-f")
        .arg("-t")
        .arg(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{App, now_unix};

    fn sample_app() -> App {
        App {
            package: "org.example.app".to_string(),
            label: "Example App".to_string(),
            version_name: Some("1.0".to_string()),
            version_code: 1,
            activity: Some("org.example.app.MainActivity".to_string()),
            icon_path: None,
            apk_path: None,
            min_sdk: Some(34),
            target_sdk: Some(35),
            abis: vec![],
            orientation: None,
            installed_at: now_unix(),
            last_launched_at: None,
            enabled: true,
        }
    }

    #[test]
    fn writes_desktop_entry_and_icon() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let png = b"\x89PNG\r\n\x1a\nresto";
        let entry = install(&sample_app(), Some(png), Path::new("/opt/apkstart"), &paths).unwrap();
        assert!(entry.desktop_file.is_file());
        assert!(entry.icon_file.unwrap().is_file());
        let content = std::fs::read_to_string(&entry.desktop_file).unwrap();
        assert!(content.contains("Name=Example App"));
        assert!(content.contains("Exec=\"/opt/apkstart\" launch org.example.app"));
        assert!(content.contains("StartupWMClass=io.github.apkstart.org.example.app"));
    }

    #[test]
    fn removes_desktop_entry_and_icon() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let png = b"\x89PNG\r\n\x1a\nresto";
        install(&sample_app(), Some(png), Path::new("/opt/apkstart"), &paths).unwrap();
        remove("org.example.app", &paths).unwrap();
        assert!(
            !paths
                .applications_dir()
                .join("io.github.apkstart.org.example.app.desktop")
                .exists()
        );
        assert!(
            !paths
                .hicolor_256_dir()
                .join("io.github.apkstart.org.example.app.png")
                .exists()
        );
    }

    #[test]
    fn ignores_unknown_icon_format() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let entry = install(
            &sample_app(),
            Some(b"<xml>"),
            Path::new("/opt/apkstart"),
            &paths,
        )
        .unwrap();
        assert!(entry.icon_file.is_none());
        let content = std::fs::read_to_string(&entry.desktop_file).unwrap();
        assert!(content.contains("Icon=application-x-executable"));
    }

    #[test]
    fn escapes_exec_path() {
        assert_eq!(
            escape_exec("/ruta con espacios/apkstart"),
            "/ruta con espacios/apkstart"
        );
        assert_eq!(escape_exec("/ruta/$x/\"apk\""), "/ruta/\\$x/\\\"apk\\\"");
    }
}
