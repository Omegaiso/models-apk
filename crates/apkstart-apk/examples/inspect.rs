use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("uso: inspect <archivo.apk>")?;
    let info = apkstart_apk::read_apk(&path)?;
    println!("{info:#?}");
    let icon = apkstart_apk::read_icon(&path, &info)?;
    println!("icon bytes: {:?}", icon.as_ref().map(Vec::len));
    Ok(())
}
