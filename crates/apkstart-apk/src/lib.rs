use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use apk_info::Apk;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("APK inválido: {0}")]
    InvalidApk(String),
    #[error("el APK no declara un package válido")]
    MissingPackage,
}

/// Metadatos que ApkStart necesita para instalar y lanzar una app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApkInfo {
    pub path: PathBuf,
    pub package: String,
    pub label: String,
    pub version_name: Option<String>,
    pub version_code: i64,
    pub main_activity: Option<String>,
    pub icon_resource: Option<String>,
    pub min_sdk: Option<i64>,
    pub target_sdk: Option<i64>,
    pub abis: Vec<String>,
    pub permissions: Vec<String>,
    /// Valor crudo de `android:appCategory` del manifiesto, si está declarado.
    pub android_category: Option<String>,
    /// Categorías freedesktop derivadas de la clasificación.
    pub categories: Vec<String>,
}

impl ApkInfo {
    /// Componente `paquete/activity` si el APK declara una activity de launcher.
    pub fn main_component(&self) -> Option<String> {
        self.main_activity.as_ref().map(|activity| {
            if activity.starts_with('.') {
                format!("{}/{}", self.package, activity)
            } else if activity.contains('/') {
                activity.clone()
            } else {
                format!("{}/{}", self.package, activity)
            }
        })
    }
}

/// Extrae un recurso (por ejemplo, el icono resuelto del manifiesto) del APK.
pub fn read_resource(path: &Path, resource: &str) -> Result<Vec<u8>, Error> {
    let apk = Apk::new(path).map_err(|error| Error::InvalidApk(error.to_string()))?;
    let (bytes, _) = apk
        .read(resource)
        .map_err(|error| Error::InvalidApk(format!("no se pudo leer {resource}: {error}")))?;
    Ok(bytes)
}

/// Extrae el icono de lanzador, resolviendo también adaptive icons XML.
pub fn read_icon(path: &Path, info: &ApkInfo) -> Result<Option<Vec<u8>>, Error> {
    let Some(resource) = info.icon_resource.as_deref() else {
        return Ok(None);
    };
    let apk = Apk::new(path).map_err(|error| Error::InvalidApk(error.to_string()))?;
    let (bytes, _) = apk
        .read(resource)
        .map_err(|error| Error::InvalidApk(format!("no se pudo leer {resource}: {error}")))?;
    if is_image(&bytes) {
        return Ok(Some(bytes));
    }

    if !resource.ends_with(".xml") {
        return Ok(None);
    }
    let (arsc_bytes, _) = apk
        .read("resources.arsc")
        .map_err(|error| Error::InvalidApk(format!("no se pudo leer resources.arsc: {error}")))?;
    let mut arsc_slice = arsc_bytes.as_slice();
    let arsc = apk_info::ARSC::new(&mut arsc_slice)
        .map_err(|error| Error::InvalidApk(format!("resources.arsc inválido: {error:?}")))?;
    let mut xml_slice = bytes.as_slice();
    let axml = apk_info::AXML::new(&mut xml_slice, Some(&arsc))
        .map_err(|error| Error::InvalidApk(format!("adaptive icon inválido: {error:?}")))?;

    for tag in ["foreground", "background"] {
        let Some(resolved) = axml.get_attribute_value(tag, "drawable", Some(&arsc)) else {
            continue;
        };
        if resolved.starts_with('#') || resolved.starts_with('@') {
            continue;
        }
        if let Ok((candidate, _)) = apk.read(&resolved)
            && is_image(&candidate)
        {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]))
        || bytes.starts_with(b"\xff\xd8\xff")
}

pub fn read_apk(path: &Path) -> Result<ApkInfo, Error> {
    let apk = Apk::new(path).map_err(|error| Error::InvalidApk(error.to_string()))?;
    let package = apk.get_package_name().ok_or(Error::MissingPackage)?;
    let label = apk
        .get_application_label()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| package.clone());
    let version_name = apk.get_version_name();
    let version_code = apk
        .get_version_code()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let main_activity = apk.get_main_activity().map(str::to_string);
    let icon_resource = apk.get_application_icon();
    let min_sdk = apk
        .get_min_sdk_version()
        .and_then(|value| value.parse::<i64>().ok());
    let target_sdk = Some(i64::from(apk.get_target_sdk_version()));
    let abis = apk
        .get_native_libraries()
        .filter_map(abi_from_library_path)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let permissions: Vec<String> = apk.get_permissions().map(str::to_string).collect();
    let android_category = apk
        .get_application_category()
        .map(|category| category.to_string());
    let categories =
        classify_categories(&package, &label, android_category.as_deref(), &permissions);

    Ok(ApkInfo {
        path: path.to_path_buf(),
        package,
        label,
        version_name,
        version_code,
        main_activity,
        icon_resource,
        min_sdk,
        target_sdk,
        abis,
        permissions,
        android_category,
        categories,
    })
}

fn abi_from_library_path(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    if parts.next()? != "lib" {
        return None;
    }
    let abi = parts.next()?;
    matches!(
        abi,
        "armeabi" | "armeabi-v7a" | "arm64-v8a" | "x86" | "x86_64" | "riscv64" | "mips" | "mips64"
    )
    .then(|| abi.to_string())
}

/// Clasifica una app Android en categorías freedesktop para que KDE/GNOME la
/// coloquen en el menú como una app nativa (Internet, Juegos, Multimedia...).
///
/// Prioridad: `android:appCategory` del manifiesto y, si no ayuda, heurísticas
/// por nombre de paquete y etiqueta. Los permisos solo se usan como último
/// recurso (servicio de accesibilidad). El resultado nunca es vacío, no tiene
/// duplicados y siempre incluye una categoría principal (`Network`, `Game`,
/// `AudioVideo`, `Graphics`, `Office`, `Development`, `System` o `Utility`).
pub fn classify_categories(
    package: &str,
    label: &str,
    android_category: Option<&str>,
    permissions: &[String],
) -> Vec<String> {
    if let Some(categories) = categories_from_android(android_category) {
        return categories;
    }
    if let Some(categories) = categories_from_heuristics(package, label) {
        return categories;
    }
    if permissions
        .iter()
        .any(|permission| permission.ends_with("BIND_ACCESSIBILITY_SERVICE"))
    {
        return vec!["Utility".to_string(), "Accessibility".to_string()];
    }
    vec!["Utility".to_string()]
}

/// Traduce los valores oficiales de `android:appCategory`; cualquier otro valor
/// se ignora y deja paso a las heurísticas.
fn categories_from_android(category: Option<&str>) -> Option<Vec<String>> {
    let categories: Vec<&str> = match category?.trim().to_ascii_lowercase().as_str() {
        "game" => vec!["Game"],
        "audio" => vec!["AudioVideo", "Audio"],
        "video" => vec!["AudioVideo", "Video"],
        "image" => vec!["Graphics", "Photography"],
        "social" => vec!["Network", "InstantMessaging"],
        "news" => vec!["Network", "News"],
        "productivity" => vec!["Office"],
        "maps" => vec!["Utility", "Maps"],
        "accessibility" => vec!["Utility", "Accessibility"],
        _ => return None,
    };
    Some(categories.into_iter().map(str::to_string).collect())
}

/// Heurísticas por paquete y etiqueta: `(keywords, categorías)`. El primer
/// grupo con una keyword que encaje decide la clasificación.
const HEURISTICS: &[(&[&str], &[&str])] = &[
    // Navegadores.
    (
        &[
            "firefox", "chrome", "chromium", "brave", "opera", "vivaldi", "browser",
        ],
        &["Network", "WebBrowser"],
    ),
    // Correo.
    (
        &["mail", "email", "thunderbird", "protonmail"],
        &["Network", "Email"],
    ),
    // Chat y mensajería.
    (
        &[
            "chat",
            "message",
            "messenger",
            "telegram",
            "signal",
            "whatsapp",
            "discord",
            "slack",
            "mastodon",
            "tusky",
            "element",
        ],
        &["Network", "InstantMessaging"],
    ),
    // Juegos.
    (
        &[
            "game",
            "sudoku",
            "chess",
            "puzzle",
            "minecraft",
            "emulator",
            "retro",
            "arcade",
        ],
        &["Game"],
    ),
    // Multimedia.
    (
        &[
            "player", "music", "audio", "video", "podcast", "spotify", "vlc", "mpv", "newpipe",
            "youtube", "netflix", "radio",
        ],
        &["AudioVideo"],
    ),
    // Gráficos y fotos.
    (
        &[
            "photo", "gallery", "camera", "image", "paint", "draw", "scan",
        ],
        &["Graphics"],
    ),
    // Oficina.
    (
        &[
            "office", "docs", "document", "pdf", "word", "excel", "sheet", "slide", "notes",
            "notion", "obsidian",
        ],
        &["Office"],
    ),
    // Desarrollo.
    (
        &[
            "code",
            "git",
            "terminal",
            "termux",
            "developer",
            "adb",
            "shell",
            "ide",
        ],
        &["Development"],
    ),
    // Sistema y tiendas de apps.
    (
        &[
            "settings",
            "files",
            "filemanager",
            "launcher",
            "keyboard",
            "vpn",
            "firewall",
            "magisk",
            "f-droid",
            "fdroid",
            "updater",
        ],
        &["System"],
    ),
];

fn categories_from_heuristics(package: &str, label: &str) -> Option<Vec<String>> {
    let haystack = format!("{package} {label}").to_ascii_lowercase();
    let tokens = tokenize(&haystack);
    for (keywords, categories) in HEURISTICS {
        if keywords
            .iter()
            .any(|keyword| matches_keyword(&haystack, &tokens, keyword))
        {
            return Some(
                categories
                    .iter()
                    .map(|category| (*category).to_string())
                    .collect(),
            );
        }
    }
    None
}

/// Keywords que solo encajan como palabra o segmento completo: `git` no debería
/// encajar dentro de `digital`, ni `ide` dentro de `identity`, ni `word` dentro
/// de `password`.
const TOKEN_ONLY_KEYWORDS: &[&str] = &["git", "ide", "word"];

fn matches_keyword(haystack: &str, tokens: &[String], keyword: &str) -> bool {
    if TOKEN_ONLY_KEYWORDS.contains(&keyword) {
        tokens.iter().any(|token| token == keyword)
    } else {
        haystack.contains(keyword)
    }
}

/// Palabras o segmentos alfanuméricos: `org.fdroid.fdroid F-Droid` ->
/// `org`, `fdroid`, `fdroid`, `f`, `droid`.
fn tokenize(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_abi_from_native_library_path() {
        assert_eq!(
            abi_from_library_path("lib/x86_64/libfoo.so").as_deref(),
            Some("x86_64")
        );
        assert_eq!(
            abi_from_library_path("lib/arm64-v8a/libbar.so").as_deref(),
            Some("arm64-v8a")
        );
        assert_eq!(abi_from_library_path("classes.dex"), None);
        assert_eq!(abi_from_library_path("lib/desconocida/libfoo.so"), None);
    }

    #[test]
    fn rejects_non_apk_file() {
        let path = std::env::temp_dir().join(format!(
            "apkstart-no-apk-{}-{}.apk",
            std::process::id(),
            super::tests_helpers::unique_suffix()
        ));
        std::fs::write(&path, b"esto no es un zip").unwrap();
        let result = read_apk(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn classifies_firefox_as_webbrowser() {
        let categories = classify_categories("org.mozilla.firefox", "Firefox", None, &[]);
        assert_eq!(
            categories,
            vec!["Network".to_string(), "WebBrowser".to_string()]
        );
    }

    #[test]
    fn classifies_sudoku_game() {
        let categories = classify_categories("com.lantern.sudoku", "Lantern Sudoku", None, &[]);
        assert_eq!(categories, vec!["Game".to_string()]);
    }

    #[test]
    fn android_audio_category_maps_to_audiovideo() {
        let categories = classify_categories("com.example.app", "Example", Some("audio"), &[]);
        assert_eq!(categories.first().map(String::as_str), Some("AudioVideo"));
    }

    #[test]
    fn classifies_fdroid_as_system() {
        let categories = classify_categories("org.fdroid.fdroid", "F-Droid", None, &[]);
        assert_eq!(categories, vec!["System".to_string()]);
    }

    #[test]
    fn unknown_app_falls_back_to_utility() {
        let categories = classify_categories("com.example.app", "Example", None, &[]);
        assert_eq!(categories, vec!["Utility".to_string()]);
    }

    #[test]
    fn unknown_android_category_uses_heuristics() {
        let categories = classify_categories(
            "com.example.browser",
            "Example Browser",
            Some("desconocida"),
            &[],
        );
        assert_eq!(categories.first().map(String::as_str), Some("Network"));
    }

    #[test]
    fn accessibility_service_permission_maps_to_utility_accessibility() {
        let permissions = vec!["android.permission.BIND_ACCESSIBILITY_SERVICE".to_string()];
        let categories =
            classify_categories("com.example.a11y", "Example A11y", None, &permissions);
        assert_eq!(
            categories,
            vec!["Utility".to_string(), "Accessibility".to_string()]
        );
    }

    #[test]
    fn token_only_keywords_do_not_match_inside_other_words() {
        // `git` no debe encajar dentro de `digital`.
        let categories = classify_categories("com.example.digital", "Digital Wellbeing", None, &[]);
        assert_eq!(categories, vec!["Utility".to_string()]);
    }

    #[test]
    fn parses_real_apk_when_available() {
        let path = std::env::var_os("APKSTART_TEST_APK")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/opencode/F-Droid.apk"));
        if !path.is_file() {
            return;
        }
        let info = read_apk(&path).expect("F-Droid debería parsearse");
        assert_eq!(info.package, "org.fdroid.fdroid");
        assert_eq!(info.label, "F-Droid");
        assert!(info.main_activity.is_some());
        assert!(info.main_component().is_some());
        assert!(info.version_code > 0);
        assert_eq!(info.categories, vec!["System".to_string()]);
    }
}

#[cfg(test)]
mod tests_helpers {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub fn unique_suffix() -> u64 {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0);
        time ^ COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}
