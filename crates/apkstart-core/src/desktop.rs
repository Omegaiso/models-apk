use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::model::App;
use crate::{Error, Result};

pub const APK_MIME: &str = "application/vnd.android.package-archive";
pub const MIME_HANDLER_ID: &str = "io.github.apkstart.install";
const HICOLOR_SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

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

    pub fn hicolor_root(&self) -> PathBuf {
        self.data_home.join("icons/hicolor")
    }

    pub fn hicolor_dir(&self, size: u32) -> PathBuf {
        self.hicolor_root().join(format!("{size}x{size}/apps"))
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

/// Genera un icono PNG con la inicial de la app cuando el APK no trae un bitmap.
pub fn fallback_icon(package: &str, label: &str) -> Result<Vec<u8>> {
    const PALETTE: [[u8; 3]; 8] = [
        [30, 136, 229],
        [67, 160, 71],
        [245, 124, 0],
        [142, 36, 170],
        [229, 57, 53],
        [0, 137, 123],
        [94, 53, 177],
        [255, 112, 67],
    ];
    const SIZE: usize = 256;
    const SCALE: usize = 22;

    let mut hash = 2_166_136_261u32;
    for byte in package.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    let [red, green, blue] = PALETTE[(hash as usize) % PALETTE.len()];

    let mut pixels = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE / 2) as i32;
    let radius = 122i32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - center;
            let dy = y as i32 - center;
            if dx * dx + dy * dy <= radius * radius {
                put_pixel(&mut pixels, x, y, [red, green, blue, 255]);
            }
        }
    }

    let letter = label
        .chars()
        .next()
        .map(|character| character.to_ascii_uppercase())
        .unwrap_or('?');
    let glyph = glyph(letter);
    let glyph_width = 5 * SCALE;
    let glyph_height = 7 * SCALE;
    let start_x = (SIZE - glyph_width) / 2;
    let start_y = (SIZE - glyph_height) / 2;
    for (row, bits) in glyph.iter().enumerate() {
        for column in 0..5 {
            if bits & (1 << (4 - column)) == 0 {
                continue;
            }
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    put_pixel(
                        &mut pixels,
                        start_x + column * SCALE + dx,
                        start_y + row * SCALE + dy,
                        [255, 255, 255, 255],
                    );
                }
            }
        }
    }

    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, SIZE as u32, SIZE as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| Error::Png(error.to_string()))?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| Error::Png(error.to_string()))?;
    }
    Ok(output)
}

fn put_pixel(pixels: &mut [u8], x: usize, y: usize, color: [u8; 4]) {
    let index = (y * 256 + x) * 4;
    pixels[index..index + 4].copy_from_slice(&color);
}

fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        _ => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
    }
}

pub fn install(
    app: &App,
    icon: Option<&[u8]>,
    executable: &Path,
    paths: &DesktopPaths,
) -> Result<DesktopEntry> {
    let app_id = app_id(&app.package);
    remove_icons(&app_id, paths)?;
    let icon_file = match icon {
        Some(bytes) => write_icon(bytes, &app_id, paths)?,
        None => None,
    };
    let desktop_file = write_desktop_entry(app, &app_id, icon_file.is_some(), executable, paths)?;
    Ok(DesktopEntry {
        app_id,
        desktop_file,
        icon_file,
    })
}

pub fn remove(package: &str, paths: &DesktopPaths) -> Result<()> {
    let app_id = app_id(package);
    remove_if_exists(&paths.applications_dir().join(format!("{app_id}.desktop")))?;
    remove_icons(&app_id, paths)?;
    Ok(())
}

fn remove_icons(app_id: &str, paths: &DesktopPaths) -> Result<()> {
    for size in HICOLOR_SIZES {
        for extension in ["png", "webp", "jpg", "jpeg"] {
            remove_if_exists(
                &paths
                    .hicolor_dir(size)
                    .join(format!("{app_id}.{extension}")),
            )?;
        }
    }
    Ok(())
}

/// Crea el handler que abre un APK con doble clic desde el gestor de archivos.
pub fn install_mime_handler(executable: &Path, paths: &DesktopPaths) -> Result<PathBuf> {
    let dir = paths.applications_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{MIME_HANDLER_ID}.desktop"));
    let exec = format!(
        "\"{}\" install --launch %f",
        escape_exec(&executable.to_string_lossy())
    );
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Instalar APK de Android\n\
         Comment=Instalar y abrir una aplicación Android con models-apk\n\
         Exec={exec}\n\
         Icon=system-software-install\n\
         Terminal=true\n\
         NoDisplay=true\n\
         Categories=Utility;\n\
         StartupNotify=true\n\
         MimeType={APK_MIME};\n\
         X-ApkStart-Installer=true\n"
    );
    fs::write(&path, content)?;
    Ok(path)
}

/// Registra el handler como aplicación por defecto para APK a nivel de usuario.
pub fn register_mime_default() -> bool {
    let handler = format!("{MIME_HANDLER_ID}.desktop");
    Command::new("xdg-mime")
        .args(["default", &handler, APK_MIME])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Refresca iconos y menús; es best-effort y no falla si falta una herramienta.
pub fn refresh_caches(paths: &DesktopPaths) {
    let root = paths.hicolor_root();
    if root.is_dir() {
        let _ = Command::new("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(&root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let applications = paths.applications_dir();
    if applications.is_dir() {
        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    let _ = Command::new("kbuildsycoca6")
        .arg("--noincremental")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn write_icon(bytes: &[u8], app_id: &str, paths: &DesktopPaths) -> Result<Option<PathBuf>> {
    let Some(extension) = image_extension(bytes) else {
        return Ok(None);
    };
    let dir = paths.hicolor_dir(icon_size(bytes));
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{app_id}.{extension}"));
    fs::write(&path, bytes)?;
    Ok(Some(path))
}

/// `Categories=` de la entrada `.desktop`: categorías separadas por `;` y
/// terminadas en `;`. Si la app no tiene clasificación, cae a `Utility`.
/// Cada nombre se reduce a caracteres alfanuméricos para que un valor corrupto
/// de la base de datos no pueda inyectar `;` ni saltos de línea.
fn desktop_categories(categories: &[String]) -> String {
    let mut names = categories
        .iter()
        .map(|category| {
            category
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|category| !category.is_empty())
        .collect::<Vec<_>>();
    if names.is_empty() {
        names.push("Utility".to_string());
    }
    format!("{};", names.join(";"))
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
    let exec = format!(
        "\"{}\" launch {}",
        escape_exec(&executable.to_string_lossy()),
        app.package
    );
    let icon = if has_icon {
        app_id.to_string()
    } else {
        "application-x-executable".to_string()
    };
    let categories = desktop_categories(&app.categories);
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment=Aplicación Android gestionada por models-apk\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories={categories}\n\
         StartupNotify=true\n\
         StartupWMClass={app_id}\n\
         X-ApkStart-Package={package}\n",
        package = app.package
    );
    fs::write(&path, content)?;
    Ok(path)
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

fn icon_size(bytes: &[u8]) -> u32 {
    let Some((width, height)) = png_dimensions(bytes) else {
        return 256;
    };
    if width != height {
        return 256;
    }
    HICOLOR_SIZES
        .iter()
        .copied()
        .filter(|size| *size <= width)
        .max()
        .unwrap_or(16)
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() < 24 {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
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
            categories: vec![],
            orientation: None,
            installed_at: now_unix(),
            last_launched_at: None,
            enabled: true,
        }
    }

    fn fake_png(size: u32) -> Vec<u8> {
        let mut bytes = Vec::from(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&size.to_be_bytes());
        bytes.extend_from_slice(&size.to_be_bytes());
        bytes
    }

    #[test]
    fn writes_desktop_entry_and_icon() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let entry = install(
            &sample_app(),
            Some(&fake_png(48)),
            Path::new("/opt/apkstart"),
            &paths,
        )
        .unwrap();
        assert!(entry.desktop_file.is_file());
        let icon = entry.icon_file.unwrap();
        assert!(icon.is_file());
        assert!(icon.to_string_lossy().contains("48x48"));
        let content = std::fs::read_to_string(&entry.desktop_file).unwrap();
        assert!(content.contains("Name=Example App"));
        assert!(content.contains("Exec=\"/opt/apkstart\" launch org.example.app"));
        assert!(content.contains("StartupWMClass=io.github.apkstart.org.example.app"));
        assert!(content.contains("Categories=Utility;"));
    }

    #[test]
    fn writes_categories_from_app() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let mut app = sample_app();
        app.categories = vec!["Network".to_string(), "WebBrowser".to_string()];
        let entry = install(&app, None, Path::new("/opt/apkstart"), &paths).unwrap();
        let content = std::fs::read_to_string(&entry.desktop_file).unwrap();
        assert!(content.contains("Categories=Network;WebBrowser;"));
    }

    #[test]
    fn formats_desktop_categories() {
        assert_eq!(desktop_categories(&[]), "Utility;");
        assert_eq!(desktop_categories(&["Game".to_string()]), "Game;");
        assert_eq!(
            desktop_categories(&["Network".to_string(), "WebBrowser".to_string()]),
            "Network;WebBrowser;"
        );
        // Valores corruptos no inyectan `;` ni saltos de línea.
        assert_eq!(
            desktop_categories(&["Game\n".to_string(), "".to_string()]),
            "Game;"
        );
    }

    #[test]
    fn removes_desktop_entry_and_icon() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let entry = install(
            &sample_app(),
            Some(&fake_png(48)),
            Path::new("/opt/apkstart"),
            &paths,
        )
        .unwrap();
        let icon = entry.icon_file.unwrap();
        remove("org.example.app", &paths).unwrap();
        assert!(
            !paths
                .applications_dir()
                .join("io.github.apkstart.org.example.app.desktop")
                .exists()
        );
        assert!(!icon.exists());
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
    fn writes_mime_handler() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DesktopPaths {
            data_home: temp.path().to_path_buf(),
        };
        let path = install_mime_handler(Path::new("/opt/apkstart"), &paths).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("MimeType=application/vnd.android.package-archive;"));
        assert!(content.contains("install --launch %f"));
        assert!(content.contains("NoDisplay=true"));
    }

    #[test]
    fn fallback_icon_is_png() {
        let icon = fallback_icon("org.example.app", "Example").unwrap();
        assert_eq!(png_dimensions(&icon), Some((256, 256)));
    }

    #[test]
    fn detects_png_size() {
        assert_eq!(png_dimensions(&fake_png(48)), Some((48, 48)));
        assert_eq!(icon_size(&fake_png(72)), 64);
        assert_eq!(icon_size(&fake_png(48)), 48);
        assert_eq!(icon_size(b"<xml>"), 256);
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
