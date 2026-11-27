use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use apkstart_core::{App, RuntimeStatus};
use gtk4 as gtk;
use libadwaita as adw;

const APP_ID: &str = "io.github.apkstart.Settings";

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

enum Msg {
    Status(Result<RuntimeStatus, String>),
    Apps(Result<Vec<App>, String>),
    Doctor(String),
    ActionResult(Result<String, String>),
}

fn build_ui(app: &adw::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("models-apk")
        .default_width(980)
        .default_height(680)
        .build();

    let header = adw::HeaderBar::new();
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Actualizar estado y apps"));
    header.pack_start(&refresh);
    window.set_titlebar(Some(&header));

    let (tx, rx) = mpsc::channel::<Msg>();
    let toast_overlay = adw::ToastOverlay::new();

    let status_label = gtk::Label::new(Some("Runtime: consultando…"));
    status_label.set_xalign(0.0);
    status_label.set_wrap(true);

    let home = build_home(&tx, &status_label);

    let apps_list = gtk::ListBox::new();
    apps_list.set_selection_mode(gtk::SelectionMode::None);
    apps_list.set_hexpand(true);
    let apps = build_apps(&tx, &window, &apps_list);

    let doctor_view = gtk::TextView::new();
    doctor_view.set_editable(false);
    doctor_view.set_monospace(true);
    doctor_view.set_wrap_mode(gtk::WrapMode::WordChar);
    let diagnostics = build_diagnostics(&tx, &doctor_view);

    let about = build_about();

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_named(&home, Some("home"));
    stack.add_named(&apps, Some("apps"));
    stack.add_named(&diagnostics, Some("diagnostics"));
    stack.add_named(&about, Some("about"));
    stack.set_visible_child_name("home");

    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.set_size_request(190, -1);
    sidebar.add_css_class("navigation-sidebar");
    let pages = [
        ("Inicio", "go-home-symbolic", "home"),
        ("Aplicaciones", "applications-other-symbolic", "apps"),
        ("Diagnóstico", "dialog-information-symbolic", "diagnostics"),
        ("Acerca de", "help-about-symbolic", "about"),
    ];
    for (index, (label, icon, _)) in pages.iter().enumerate() {
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(10);
        content.set_margin_end(10);
        let image = gtk::Image::from_icon_name(icon);
        let text = gtk::Label::new(Some(label));
        text.set_xalign(0.0);
        content.append(&image);
        content.append(&text);
        row.set_child(Some(&content));
        row.set_activatable(true);
        sidebar.append(&row);
        if index == 0 {
            sidebar.select_row(Some(&row));
        }
    }
    let stack_for_sidebar = stack.clone();
    sidebar.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let index = row.index().max(0) as usize;
            if let Some((_, _, name)) = pages.get(index) {
                stack_for_sidebar.set_visible_child_name(name);
            }
        }
    });

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.append(&sidebar);
    root.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    root.append(&stack);
    toast_overlay.set_child(Some(&root));
    window.set_child(Some(&toast_overlay));
    window.present();

    let tx_handler = tx.clone();
    let status_for_handler = status_label.clone();
    let apps_for_handler = apps_list.clone();
    let doctor_for_handler = doctor_view.clone();
    let toast_for_handler = toast_overlay.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(message) = rx.try_recv() {
            match message {
                Msg::Status(result) => match result {
                    Ok(status) => {
                        status_for_handler.set_text(&status_text(&status));
                        status_for_handler.remove_css_class("error");
                    }
                    Err(error) => {
                        status_for_handler.set_text(&format!("Runtime: error\n{error}"));
                        status_for_handler.add_css_class("error");
                    }
                },
                Msg::Apps(result) => match result {
                    Ok(apps) => fill_apps(&apps_for_handler, &apps, &tx_handler),
                    Err(error) => {
                        clear_list(&apps_for_handler);
                        apps_for_handler.append(&error_row(&error));
                    }
                },
                Msg::Doctor(output) => {
                    doctor_for_handler.buffer().set_text(&output);
                }
                Msg::ActionResult(result) => {
                    match result {
                        Ok(output) if !output.trim().is_empty() => {
                            toast(&toast_for_handler, output.trim());
                        }
                        Ok(_) => toast(&toast_for_handler, "Listo"),
                        Err(error) => toast(&toast_for_handler, &format!("Error: {error}")),
                    }
                    spawn_status(tx_handler.clone());
                    spawn_apps(tx_handler.clone());
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| {
        spawn_status(tx_refresh.clone());
        spawn_apps(tx_refresh.clone());
    });

    spawn_status(tx.clone());
    spawn_apps(tx.clone());
    spawn_doctor(tx.clone());
}

fn build_home(tx: &Sender<Msg>, status_label: &gtk::Label) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 16);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(28);
    page.set_margin_end(28);

    let title = gtk::Label::new(Some("Inicio"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    page.append(&title);

    status_label.add_css_class("dim-label");
    page.append(status_label);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let start = gtk::Button::with_label("Arrancar Android");
    start.add_css_class("suggested-action");
    let stop = gtk::Button::with_label("Detener Android");
    stop.add_css_class("destructive-action");
    let sync = gtk::Button::with_label("Sincronizar apps");
    actions.append(&start);
    actions.append(&stop);
    actions.append(&sync);
    page.append(&actions);

    let hint = gtk::Label::new(Some(
        "Android se ejecuta headless. Cada app se abre en su propia ventana.",
    ));
    hint.set_xalign(0.0);
    hint.add_css_class("dim-label");
    hint.set_wrap(true);
    page.append(&hint);

    let tx_start = tx.clone();
    start.connect_clicked(move |_| {
        spawn_action(tx_start.clone(), &["runtime", "start"]);
    });
    let tx_stop = tx.clone();
    stop.connect_clicked(move |_| {
        spawn_action(tx_stop.clone(), &["runtime", "stop"]);
    });
    let tx_sync = tx.clone();
    sync.connect_clicked(move |_| {
        spawn_action(tx_sync.clone(), &["sync"]);
    });

    page.upcast()
}

fn build_apps(
    tx: &Sender<Msg>,
    window: &gtk::ApplicationWindow,
    list: &gtk::ListBox,
) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(18);
    page.set_margin_bottom(18);
    page.set_margin_start(18);
    page.set_margin_end(18);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Aplicaciones"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    title.set_hexpand(true);
    let install = gtk::Button::with_label("Instalar APK");
    install.add_css_class("suggested-action");
    let refresh = gtk::Button::with_label("Actualizar");
    toolbar.append(&title);
    toolbar.append(&refresh);
    toolbar.append(&install);
    page.append(&toolbar);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(list));
    page.append(&scrolled);

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| spawn_apps(tx_refresh.clone()));

    let tx_install = tx.clone();
    let window = window.clone();
    install.connect_clicked(move |_| {
        let chooser = gtk::FileChooserNative::new(
            Some("Seleccionar APK"),
            Some(&window),
            gtk::FileChooserAction::Open,
            Some("Instalar"),
            Some("Cancelar"),
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("APK de Android"));
        filter.add_pattern("*.apk");
        chooser.add_filter(&filter);
        let tx_choose = tx_install.clone();
        chooser.connect_response(move |chooser, response| {
            if response == gtk::ResponseType::Accept
                && let Some(path) = chooser.file().and_then(|file| file.path())
            {
                let path = path.to_string_lossy().into_owned();
                spawn_action(tx_choose.clone(), &["install", "--launch", &path]);
            }
            chooser.destroy();
        });
        chooser.show();
    });

    page.upcast()
}

fn build_diagnostics(tx: &Sender<Msg>, view: &gtk::TextView) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(18);
    page.set_margin_bottom(18);
    page.set_margin_start(18);
    page.set_margin_end(18);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Diagnóstico"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    title.set_hexpand(true);
    let run = gtk::Button::with_label("Ejecutar doctor");
    let export = gtk::Button::with_label("Exportar logs");
    toolbar.append(&title);
    toolbar.append(&run);
    toolbar.append(&export);
    page.append(&toolbar);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(view));
    page.append(&scrolled);

    let tx_run = tx.clone();
    run.connect_clicked(move |_| spawn_doctor(tx_run.clone()));
    let tx_export = tx.clone();
    export.connect_clicked(move |_| spawn_action(tx_export.clone(), &["export-log"]));
    page.upcast()
}

fn build_about() -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(28);
    page.set_margin_end(28);
    let title = gtk::Label::new(Some("models-apk"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    page.append(&title);
    for text in [
        format!("Versión {}", env!("CARGO_PKG_VERSION")),
        "Gestor de aplicaciones Android para Linux.".to_string(),
        "Android Emulator + scrcpy + Wayland.".to_string(),
        "Licencia MIT.".to_string(),
    ] {
        let label = gtk::Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_wrap(true);
        page.append(&label);
    }
    page.upcast()
}

fn fill_apps(list: &gtk::ListBox, apps: &[App], tx: &Sender<Msg>) {
    clear_list(list);
    if apps.is_empty() {
        list.append(&error_row(
            "No hay apps registradas. Instala un APK o sincroniza.",
        ));
        return;
    }
    let tx = tx.clone();
    for app in apps {
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.set_margin_top(10);
        content.set_margin_bottom(10);
        content.set_margin_start(10);
        content.set_margin_end(10);

        let icon = app
            .icon_path
            .as_ref()
            .filter(|path| path.is_file())
            .map(gtk::Image::from_file)
            .unwrap_or_else(|| gtk::Image::from_icon_name("application-x-executable"));
        icon.set_pixel_size(36);

        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        let name = gtk::Label::new(Some(&app.label));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        let meta = gtk::Label::new(Some(&format!(
            "{} · {}",
            app.package,
            app.version_name.as_deref().unwrap_or("sin versión")
        )));
        meta.set_xalign(0.0);
        meta.add_css_class("dim-label");
        meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&name);
        text.append(&meta);

        let open = gtk::Button::with_label("Abrir");
        let stop = gtk::Button::with_label("Cerrar");
        let package = app.package.clone();
        let tx_open = tx.clone();
        open.connect_clicked(move |_| {
            spawn_action(tx_open.clone(), &["launch", &package]);
        });
        let package = app.package.clone();
        let tx_stop = tx.clone();
        stop.connect_clicked(move |_| {
            spawn_action(tx_stop.clone(), &["stop", &package]);
        });

        content.append(&icon);
        content.append(&text);
        content.append(&open);
        content.append(&stop);
        row.set_child(Some(&content));
        list.append(&row);
    }
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn error_row(message: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_margin_top(12);
    label.set_margin_bottom(12);
    label.set_margin_start(12);
    label.set_margin_end(12);
    row.set_child(Some(&label));
    row
}

fn status_text(status: &RuntimeStatus) -> String {
    format!(
        "Runtime: {}\nSerial: {}\nBoot completado: {}",
        status.state.as_str(),
        status.serial.as_deref().unwrap_or("-"),
        if status.boot_completed { "sí" } else { "no" }
    )
}

fn toast(overlay: &adw::ToastOverlay, message: &str) {
    overlay.add_toast(adw::Toast::new(message));
}

fn cli_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .map(|path| path.with_file_name("apkstart"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("apkstart"))
}

fn run_cli(args: &[&str]) -> Result<String, String> {
    let output = Command::new(cli_path())
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn run_cli_raw(args: &[&str]) -> String {
    match Command::new(cli_path()).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let status = output.status;
            format!("{stdout}{stderr}\n[estado: {status}]")
        }
        Err(error) => format!("No se pudo ejecutar el CLI: {error}"),
    }
}

fn spawn_status(tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = run_cli(&["runtime", "status", "--json"])
            .and_then(|json| serde_json::from_str(&json).map_err(|error| error.to_string()));
        let _ = tx.send(Msg::Status(result));
    });
}

fn spawn_apps(tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = run_cli(&["list", "--json"])
            .and_then(|json| serde_json::from_str(&json).map_err(|error| error.to_string()));
        let _ = tx.send(Msg::Apps(result));
    });
}

fn spawn_doctor(tx: Sender<Msg>) {
    thread::spawn(move || {
        let output = run_cli_raw(&["doctor"]);
        let _ = tx.send(Msg::Doctor(output));
    });
}

fn spawn_action(tx: Sender<Msg>, args: &[&str]) {
    let args = args
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    thread::spawn(move || {
        let args = args.iter().map(String::as_str).collect::<Vec<_>>();
        let result = run_cli(&args);
        let _ = tx.send(Msg::ActionResult(result));
    });
}
