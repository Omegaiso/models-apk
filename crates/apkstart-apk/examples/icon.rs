use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("uso: icon <archivo.apk>")?;
    let info = apkstart_apk::read_apk(&path)?;
    let resource = info.icon_resource.ok_or("sin icono")?;
    println!("resource={resource}");
    let apk = apk_info::Apk::new(&path)?;
    let (xml, _) = apk.read(&resource)?;
    let (arsc_bytes, _) = apk.read("resources.arsc")?;
    let mut arsc_slice = arsc_bytes.as_slice();
    let arsc = apk_info::ARSC::new(&mut arsc_slice)?;
    let mut xml_slice = xml.as_slice();
    let axml = apk_info::AXML::new(&mut xml_slice, Some(&arsc))?;
    println!("--- xml ---\n{}", axml.get_xml_string());
    for tag in ["foreground", "background", "monochrome"] {
        println!(
            "{tag}/drawable = {:?}",
            axml.get_attribute_value(tag, "drawable", Some(&arsc))
        );
    }
    Ok(())
}
