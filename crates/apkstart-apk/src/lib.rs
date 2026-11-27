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
    let permissions = apk.get_permissions().map(str::to_string).collect();

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
