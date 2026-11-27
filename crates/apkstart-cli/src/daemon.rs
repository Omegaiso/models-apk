use zbus::Connection;
use zbus::proxy;

#[proxy(
    interface = "org.apkstart.Manager1",
    default_service = "org.apkstart.Manager1",
    default_path = "/org/apkstart/Manager1"
)]
trait Manager1 {
    fn get_runtime_status(&self) -> zbus::Result<String>;
    fn start_runtime(&self) -> zbus::Result<String>;
    fn stop_runtime(&self) -> zbus::Result<String>;
    fn install(&self, path: &str, launch: bool) -> zbus::Result<String>;
    fn sync(&self, refresh: bool) -> zbus::Result<(u32, u32)>;
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

pub async fn runtime_status() -> Call<String> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.get_runtime_status().await {
        Ok(status) => Call::Ok(status),
        Err(error) => classify(error),
    }
}

pub async fn start_runtime() -> Call<String> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.start_runtime().await {
        Ok(status) => Call::Ok(status),
        Err(error) => classify(error),
    }
}

pub async fn stop_runtime() -> Call<String> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.stop_runtime().await {
        Ok(status) => Call::Ok(status),
        Err(error) => classify(error),
    }
}

pub async fn install(path: &std::path::Path, launch_after: bool) -> Call<String> {
    let path = path.to_string_lossy().into_owned();
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.install(&path, launch_after).await {
        Ok(package) => Call::Ok(package),
        Err(error) => classify(error),
    }
}

pub async fn sync(refresh: bool) -> Call<(u32, u32)> {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => return classify(error),
    };
    let proxy = match Manager1Proxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => return classify(error),
    };
    match proxy.sync(refresh).await {
        Ok(report) => Call::Ok(report),
        Err(error) => classify(error),
    }
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
    if text.contains("ServiceUnknown")
        || text.contains("NameHasNoOwner")
        || text.contains("UnknownMethod")
    {
        Call::Unavailable
    } else {
        Call::Failed(text)
    }
}
