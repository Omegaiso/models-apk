use zbus::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.apkstart.Manager1",
    default_service = "org.apkstart.Manager1",
    default_path = "/org/apkstart/Manager1"
)]
trait Manager1 {
    fn launch(&self, package: &str) -> zbus::Result<u32>;
    fn stop_app(&self, package: &str) -> zbus::Result<bool>;
}

/// Resultado de intentar hablar con `apkstartd`.
pub enum Call<T> {
    Ok(T),
    /// No hay daemon ni activación D-Bus disponible; usar el modo directo.
    Unavailable,
    /// El daemon respondió con un error real.
    Failed(String),
}

pub async fn launch(package: &str) -> Call<u32> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.launch(package).await {
        Ok(pid) => Call::Ok(pid),
        Err(error) => classify(error),
    }
}

pub async fn stop_app(package: &str) -> Call<bool> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.stop_app(package).await {
        Ok(stopped) => Call::Ok(stopped),
        Err(error) => classify(error),
    }
}

fn classify<T>(error: zbus::Error) -> Call<T> {
    let text = error.to_string();
    if text.contains("ServiceUnknown") || text.contains("NameHasNoOwner") {
        Call::Unavailable
    } else {
        Call::Failed(text)
    }
}
