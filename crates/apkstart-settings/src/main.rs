mod ui;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use adw::prelude::*;
use apkstart_core::{App, RuntimeState, RuntimeStatus};
use gtk4 as gtk;
use libadwaita as adw;

const APP_ID: &str = "io.github.apkstart.Settings";

type Nav = Rc<dyn Fn(&str)>;

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

enum Msg {
    Status(Result<RuntimeStatus, String>),
    Apps(Result<Vec<App>, String>),
    Resources(Result<ResourceReport, String>),
    Integration(Result<IntegrationStatus, String>),
    Doctor(String),
    ActionResult(Result<String, String>),
}

struct IntegrationStatus {
    mime: String,
    binfmt: bool,
    daemon: String,
    timer: String,
    shared_folder: bool,
}

struct ResourceProcess {
    pid: u32,
    name: String,
    cpu: f32,
    rss_kb: u64,
}

struct ResourceReport {
    processes: Vec<ResourceProcess>,
    total_cpu: f32,
    total_rss_kb: u64,
    host_cpus: usize,
    gpus: Vec<GpuReport>,
}

struct GpuReport {
    name: String,
    busy: u8,
    vram_used: u64,
    vram_total: u64,
}

struct CpuSnapshot {
    total: u64,
    pids: HashMap<u32, u64>,
}

static CPU_SAMPLE: OnceLock<Mutex<Option<CpuSnapshot>>> = OnceLock::new();

#[derive(Clone)]
struct Metrics {
    cpu_value: gtk::Label,
    cpu_sub: gtk::Label,
    ram_value: gtk::Label,
    ram_sub: gtk::Label,
    gpu_value: gtk::Label,
    gpu_sub: gtk::Label,
}

#[derive(Clone)]
struct HomePage {
    root: gtk::Widget,
    hero_title: gtk::Label,
    hero_sub: gtk::Label,
    pill: gtk::Label,
    metrics: Metrics,
    recent: gtk::FlowBox,
}

#[derive(Clone)]
struct AppsPage {
    root: gtk::Widget,
    flow: gtk::FlowBox,
    count: gtk::Label,
}

#[derive(Clone)]
struct ResourcesPage {
    root: gtk::Widget,
    metrics: Metrics,
    gpus: gtk::FlowBox,
    processes: gtk::ListBox,
    summary: gtk::Label,
}

#[derive(Clone)]
struct CheckWidgets {
    value: gtk::Label,
    pill: gtk::Label,
}

#[derive(Clone)]
struct IntegrationPage {
    root: gtk::Widget,
    mime: CheckWidgets,
    binfmt: CheckWidgets,
    daemon: CheckWidgets,
    timer: CheckWidgets,
    shared: CheckWidgets,
    overall: gtk::Label,
}

#[derive(Clone)]
struct DiagnosticsPage {
    root: gtk::Widget,
    doctor: gtk::TextView,
}

fn build_ui(app: &adw::Application) {
    ui::install_css();

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("models-apk")
        .default_width(1120)
        .default_height(780)
        .build();
    window.set_size_request(460, 540);

    let (tx, rx) = mpsc::channel::<Msg>();
    let toast_overlay = adw::ToastOverlay::new();

    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("models-apk", "Ajustes");
    header.set_title_widget(Some(&title));
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Actualizar estado y apps"));
    header.pack_end(&refresh);

    let menu_button = gtk::ToggleButton::new();
    menu_button.set_icon_name("sidebar-show-symbolic");
    menu_button.set_tooltip_text(Some("Mostrar u ocultar la navegación"));
    menu_button.set_visible(false);
    header.pack_start(&menu_button);
    window.set_titlebar(Some(&header));

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    let split = adw::OverlaySplitView::new();
    split.set_min_sidebar_width(210.0);
    split.set_max_sidebar_width(260.0);
    split.set_sidebar_width_fraction(0.22);
    split.set_pin_sidebar(false);
    split.set_show_sidebar(true);
    split.set_enable_hide_gesture(true);

    // Sidebar: marca + navegación.
    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let brand = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    brand.set_margin_top(16);
    brand.set_margin_bottom(12);
    brand.set_margin_start(16);
    brand.set_margin_end(16);
    let mark = ui::brand_mark(36, "preferences-system-symbolic");
    let brand_text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let brand_name = gtk::Label::new(Some("models-apk"));
    brand_name.set_xalign(0.0);
    brand_name.add_css_class("heading");
    let brand_sub = gtk::Label::new(Some("Ajustes"));
    brand_sub.set_xalign(0.0);
    brand_sub.add_css_class("dim-label");
    brand_text.append(&brand_name);
    brand_text.append(&brand_sub);
    brand.append(&mark);
    brand.append(&brand_text);
    sidebar_box.append(&brand);

    let sidebar = gtk::ListBox::new();
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    sidebar.add_css_class("navigation-sidebar");
    sidebar.set_margin_start(8);
    sidebar.set_margin_end(8);
    sidebar.set_margin_bottom(8);

    let pages = [
        ("Inicio", "go-home-symbolic", "home"),
        ("Aplicaciones", "applications-other-symbolic", "apps"),
        ("Recursos", "utilities-system-monitor-symbolic", "resources"),
        ("Red", "network-wired-symbolic", "network"),
        ("Periféricos", "input-keyboard-symbolic", "peripherals"),
        ("Integración", "emblem-synchronized-symbolic", "integration"),
        ("Diagnóstico", "dialog-information-symbolic", "diagnostics"),
        ("Acerca de", "help-about-symbolic", "about"),
    ];
    let mut rows = Vec::new();
    for (label, icon, _) in pages {
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
        rows.push(row);
    }
    let initial_page = std::env::var("APKSTART_PAGE").unwrap_or_else(|_| "home".to_string());
    let initial_index = pages
        .iter()
        .position(|page| page.2 == initial_page)
        .unwrap_or(0);
    if let Some(row) = rows.get(initial_index) {
        sidebar.select_row(Some(row));
    }
    let sidebar_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&sidebar)
        .build();
    sidebar_box.append(&sidebar_scrolled);

    let names = pages.iter().map(|page| page.2).collect::<Vec<_>>();
    let nav: Nav = {
        let stack = stack.clone();
        let sidebar = sidebar.clone();
        let rows = rows.clone();
        let names = names.clone();
        Rc::new(move |name: &str| {
            stack.set_visible_child_name(name);
            if let Some(index) = names.iter().position(|candidate| *candidate == name)
                && let Some(row) = rows.get(index)
            {
                sidebar.select_row(Some(row));
            }
        })
    };

    let home = build_home(&tx, &nav);
    let apps_page = build_apps(&tx, &window);
    let resources_page = build_resources(&tx);
    let network = build_network();
    let peripherals = build_peripherals();
    let integration_page = build_integration(&tx);
    let diagnostics_page = build_diagnostics(&tx);
    let about = build_about();

    stack.add_named(&home.root, Some("home"));
    stack.add_named(&apps_page.root, Some("apps"));
    stack.add_named(&resources_page.root, Some("resources"));
    stack.add_named(&network, Some("network"));
    stack.add_named(&peripherals, Some("peripherals"));
    stack.add_named(&integration_page.root, Some("integration"));
    stack.add_named(&diagnostics_page.root, Some("diagnostics"));
    stack.add_named(&about, Some("about"));
    stack.set_visible_child_name(&initial_page);

    split.set_sidebar(Some(&sidebar_box));
    split.set_content(Some(&stack));

    // Sidebar colapsable: por debajo de 800 px la navegación pasa a overlay.
    let breakpoint_bin = adw::BreakpointBin::new();
    breakpoint_bin.set_size_request(360, 480);
    breakpoint_bin.set_hexpand(true);
    breakpoint_bin.set_vexpand(true);
    breakpoint_bin.set_property("child", &split);
    let narrow = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("max-width: 800px")
            .expect("condición de breakpoint válida"),
    );
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
    breakpoint_bin.add_breakpoint(narrow);

    toast_overlay.set_child(Some(&breakpoint_bin));
    window.set_child(Some(&toast_overlay));

    let split_for_toggle = split.clone();
    menu_button.connect_toggled(move |button| {
        split_for_toggle.set_show_sidebar(button.is_active());
    });
    let menu_for_state = menu_button.clone();
    split.connect_notify_local(Some("show-sidebar"), move |split, _| {
        menu_for_state.set_active(split.shows_sidebar());
    });
    let menu_for_collapsed = menu_button.clone();
    split.connect_notify_local(Some("collapsed"), move |split, _| {
        menu_for_collapsed.set_visible(split.is_collapsed());
        if !split.is_collapsed() {
            split.set_show_sidebar(true);
        }
    });

    let stack_for_sidebar = stack.clone();
    let split_for_sidebar = split.clone();
    let pages_for_sidebar = pages;
    sidebar.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let index = row.index().max(0) as usize;
            if let Some((_, _, name)) = pages_for_sidebar.get(index) {
                stack_for_sidebar.set_visible_child_name(name);
                if split_for_sidebar.is_collapsed() {
                    split_for_sidebar.set_show_sidebar(false);
                }
            }
        }
    });

    window.present();

    let tx_handler = tx.clone();
    let window_for_handler = window.clone();
    let toast_for_handler = toast_overlay.clone();
    let home_for_handler = home.clone();
    let apps_for_handler = apps_page.clone();
    let resources_for_handler = resources_page.clone();
    let integration_for_handler = integration_page.clone();
    let doctor_for_handler = diagnostics_page.doctor.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        while let Ok(message) = rx.try_recv() {
            match message {
                Msg::Status(result) => update_home_status(&home_for_handler, &result),
                Msg::Apps(result) => match result {
                    Ok(apps) => {
                        fill_apps(
                            &apps_for_handler.flow,
                            &apps_for_handler.count,
                            &apps,
                            &tx_handler,
                            &window_for_handler,
                        );
                        fill_recent(&home_for_handler.recent, &apps, &tx_handler);
                    }
                    Err(error) => {
                        clear_flow(&apps_for_handler.flow);
                        ui::set_pill(&apps_for_handler.count, "error", "hot");
                        apps_for_handler.flow.insert(&error_card(&error), -1);
                        clear_flow(&home_for_handler.recent);
                        home_for_handler.recent.insert(&error_card(&error), -1);
                    }
                },
                Msg::Resources(result) => match result {
                    Ok(report) => {
                        fill_resources(&resources_for_handler, &report);
                        update_metrics(&home_for_handler.metrics, &report);
                    }
                    Err(error) => {
                        resources_for_handler
                            .summary
                            .set_text(&format!("Recursos: error — {error}"));
                    }
                },
                Msg::Integration(result) => match result {
                    Ok(status) => fill_integration(&integration_for_handler, &status),
                    Err(error) => integration_for_handler
                        .overall
                        .set_text(&format!("Integración: error — {error}")),
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
                    spawn_resources(tx_handler.clone());
                    spawn_integration(tx_handler.clone());
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| {
        spawn_status(tx_refresh.clone());
        spawn_apps(tx_refresh.clone());
        spawn_resources(tx_refresh.clone());
        spawn_integration(tx_refresh.clone());
    });

    let tx_resources = tx.clone();
    gtk::glib::timeout_add_local(Duration::from_secs(2), move || {
        spawn_resources(tx_resources.clone());
        gtk::glib::ControlFlow::Continue
    });

    spawn_status(tx.clone());
    spawn_apps(tx.clone());
    spawn_resources(tx.clone());
    spawn_integration(tx.clone());
    spawn_doctor(tx.clone());
}

fn build_home(tx: &Sender<Msg>, nav: &Nav) -> HomePage {
    let body = ui::page_body();
    let head = ui::header(
        "RUNTIME",
        "Inicio",
        "Android 14 headless · KVM · 3 GB de guest",
    );
    body.append(&head.root);

    let hero = ui::card();
    hero.add_css_class("hero");
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let eyebrow = gtk::Label::new(Some("ESTADO DEL RUNTIME"));
    eyebrow.add_css_class("eyebrow");
    eyebrow.set_xalign(0.0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let pill = ui::pill("Consultando…", "accent");
    pill.set_valign(gtk::Align::Center);
    top.append(&eyebrow);
    top.append(&spacer);
    top.append(&pill);

    let hero_title = gtk::Label::new(Some("Consultando Android…"));
    hero_title.add_css_class("hero-title");
    hero_title.set_xalign(0.0);
    hero_title.set_wrap(true);
    let hero_sub = ui::note("Runtime, serial y estado de arranque.");

    let actions = ui::actions();
    let start = gtk::Button::with_label("Arrancar Android");
    start.add_css_class("suggested-action");
    let stop = gtk::Button::with_label("Detener");
    stop.add_css_class("destructive-action");
    let sync = gtk::Button::with_label("Sincronizar");
    actions.insert(&start, -1);
    actions.insert(&stop, -1);
    actions.insert(&sync, -1);

    hero.append(&top);
    hero.append(&hero_title);
    hero.append(&hero_sub);
    hero.append(&actions);
    body.append(&hero);

    let metrics_row = ui::flow(3, 190);
    let cpu = ui::metric("CPU");
    let ram = ui::metric("RAM");
    let gpu = ui::metric("GPU");
    ui::flow_add(&metrics_row, &cpu.root, 190);
    ui::flow_add(&metrics_row, &ram.root, 190);
    ui::flow_add(&metrics_row, &gpu.root, 190);
    body.append(&metrics_row);

    let recent_head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let recent_title = ui::section("Apps recientes");
    recent_title.set_hexpand(true);
    let see_all = gtk::Button::with_label("Ver todas");
    see_all.add_css_class("flat");
    recent_head.append(&recent_title);
    recent_head.append(&see_all);
    body.append(&recent_head);

    let recent = ui::flow(2, 280);
    body.append(&recent);
    body.append(&ui::note(
        "Cada app se abre en su propia ventana. Cerrar la ventana destruye la sesión.",
    ));

    let tx_start = tx.clone();
    start.connect_clicked(move |_| spawn_action(tx_start.clone(), &["runtime", "start"]));
    let tx_stop = tx.clone();
    stop.connect_clicked(move |_| spawn_action(tx_stop.clone(), &["runtime", "stop"]));
    let tx_sync = tx.clone();
    sync.connect_clicked(move |_| spawn_action(tx_sync.clone(), &["sync"]));
    let nav_see_all = nav.clone();
    see_all.connect_clicked(move |_| nav_see_all("apps"));

    HomePage {
        root: ui::page(&body),
        hero_title,
        hero_sub,
        pill,
        metrics: Metrics {
            cpu_value: cpu.value,
            cpu_sub: cpu.sub,
            ram_value: ram.value,
            ram_sub: ram.sub,
            gpu_value: gpu.value,
            gpu_sub: gpu.sub,
        },
        recent,
    }
}

fn update_home_status(home: &HomePage, result: &Result<RuntimeStatus, String>) {
    match result {
        Ok(status) => {
            let (title, pill_text, class) = match status.state {
                RuntimeState::Stopped => ("Android está detenido", "Detenido", ""),
                RuntimeState::Starting => ("Arrancando Android", "Arrancando…", "accent"),
                RuntimeState::Running => ("Todo listo para abrir apps", "En ejecución", "ok"),
                RuntimeState::Degraded => ("Android en estado degradado", "Degradado", "warn"),
                RuntimeState::Failed => ("Android no pudo arrancar", "Error", "hot"),
            };
            home.hero_title.set_text(title);
            home.hero_sub.set_text(&format!(
                "Android 14 headless · {} · boot {}",
                status.serial.as_deref().unwrap_or("sin serial"),
                if status.boot_completed {
                    "completado"
                } else {
                    "pendiente"
                }
            ));
            ui::set_pill(&home.pill, pill_text, class);
        }
        Err(error) => {
            home.hero_title.set_text("No se pudo consultar el runtime");
            home.hero_sub.set_text(error);
            ui::set_pill(&home.pill, "Error", "hot");
        }
    }
}

fn fill_recent(flow: &gtk::FlowBox, apps: &[App], tx: &Sender<Msg>) {
    clear_flow(flow);
    if apps.is_empty() {
        flow.insert(
            &note_card("Aún no hay apps registradas. Instala un APK o pulsa Sincronizar."),
            -1,
        );
        return;
    }
    for app in apps.iter().take(4) {
        let card = ui::card();
        card.add_css_class("compact");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let icon = app_icon(app, 30);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        text.set_hexpand(true);
        let name = gtk::Label::new(Some(&app.label));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.add_css_class("card-title");
        let package = gtk::Label::new(Some(&app.package));
        package.set_xalign(0.0);
        package.add_css_class("dim-label");
        package.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&name);
        text.append(&package);
        let open = gtk::Button::with_label("Abrir");
        open.add_css_class("flat");
        let package_id = app.package.clone();
        let tx_open = tx.clone();
        open.connect_clicked(move |_| spawn_action(tx_open.clone(), &["launch", &package_id]));
        row.append(&icon);
        row.append(&text);
        row.append(&open);
        card.append(&row);
        ui::flow_add(flow, &card, 280);
    }
}

fn build_apps(tx: &Sender<Msg>, window: &gtk::ApplicationWindow) -> AppsPage {
    let body = ui::page_body();
    let head = ui::header("PACKAGES", "Aplicaciones", "Apps instaladas y lanzables");
    let count = head.pill.clone();
    let refresh = gtk::Button::with_label("Actualizar");
    let install = gtk::Button::with_label("Instalar APK");
    install.add_css_class("suggested-action");
    head.action(&refresh);
    head.action(&install);
    body.append(&head.root);

    let flow = ui::flow(2, 340);
    body.append(&flow);
    body.append(&ui::note(
        "Abrir lanza una ventana scrcpy. Detalles muestra package, SDK, ABIs y la opción de desinstalar.",
    ));

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| spawn_apps(tx_refresh.clone()));

    let tx_install = tx.clone();
    let window = window.clone();
    install.connect_clicked(move |_| {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("APK de Android"));
        filter.add_pattern("*.apk");
        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Seleccionar APK")
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let tx_choose = tx_install.clone();
        dialog.open(
            Some(&window),
            None::<&gtk::gio::Cancellable>,
            move |result| {
                if let Ok(file) = result
                    && let Some(path) = file.path()
                {
                    let path = path.to_string_lossy().into_owned();
                    spawn_action(tx_choose.clone(), &["install", "--launch", &path]);
                }
            },
        );
    });

    AppsPage {
        root: ui::page(&body),
        flow,
        count,
    }
}

fn fill_apps(
    flow: &gtk::FlowBox,
    count: &gtk::Label,
    apps: &[App],
    tx: &Sender<Msg>,
    window: &gtk::ApplicationWindow,
) {
    clear_flow(flow);
    let count_text = if apps.len() == 1 {
        "1 app".to_string()
    } else {
        format!("{} apps", apps.len())
    };
    ui::set_pill(count, &count_text, "accent");
    if apps.is_empty() {
        flow.insert(
            &note_card("No hay apps registradas. Instala un APK o sincroniza."),
            -1,
        );
        return;
    }

    for app in apps {
        let card = ui::card();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let icon = app_icon(app, 44);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        let name = gtk::Label::new(Some(&app.label));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.add_css_class("card-title");
        let meta = gtk::Label::new(Some(&format!(
            "{} · {}",
            app.package,
            app.version_name.as_deref().unwrap_or("sin versión")
        )));
        meta.set_xalign(0.0);
        meta.add_css_class("dim-label");
        meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
        meta.set_tooltip_text(Some(&app.package));
        text.append(&name);
        text.append(&meta);

        let category = app
            .categories
            .iter()
            .find(|category| !category.is_empty())
            .cloned()
            .unwrap_or_else(|| "Sin categoría".to_string());
        let badge = ui::pill(&category, "accent");
        badge.set_valign(gtk::Align::Start);

        row.append(&icon);
        row.append(&text);
        row.append(&badge);
        card.append(&row);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_halign(gtk::Align::End);
        actions.set_margin_top(2);
        let details = gtk::Button::with_label("Detalles");
        details.add_css_class("flat");
        let stop = gtk::Button::with_label("Cerrar");
        stop.add_css_class("flat");
        let open = gtk::Button::with_label("Abrir");
        open.add_css_class("suggested-action");
        actions.append(&details);
        actions.append(&stop);
        actions.append(&open);
        card.append(&actions);

        let app_details = app.clone();
        let window_details = window.clone();
        let tx_details = tx.clone();
        details.connect_clicked(move |_| {
            let text = format!(
                "{}\n\nPackage: {}\nVersión: {} ({})\nActivity: {}\nMin SDK: {}\nTarget SDK: {}\nABIs: {}\nCategorías: {}\nAPK: {}",
                app_details.label,
                app_details.package,
                app_details.version_name.as_deref().unwrap_or("-"),
                app_details.version_code,
                app_details.activity.as_deref().unwrap_or("-"),
                app_details.min_sdk.unwrap_or(0),
                app_details.target_sdk.unwrap_or(0),
                if app_details.abis.is_empty() {
                    "ninguna".to_string()
                } else {
                    app_details.abis.join(", ")
                },
                app_details.categories.join(", "),
                app_details
                    .apk_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            let dialog = adw::MessageDialog::new(
                Some(&window_details),
                Some(&app_details.label),
                Some(&text),
            );
            dialog.add_response("close", "Cerrar");
            dialog.add_response("uninstall", "Desinstalar");
            dialog.set_response_appearance(
                "uninstall",
                adw::ResponseAppearance::Destructive,
            );
            dialog.set_close_response("close");
            dialog.set_default_response(Some("close"));
            let package = app_details.package.clone();
            let tx_uninstall = tx_details.clone();
            dialog.connect_response(None, move |dialog: &adw::MessageDialog, response: &str| {
                if response == "uninstall" {
                    spawn_action(tx_uninstall.clone(), &["uninstall", &package]);
                }
                dialog.close();
            });
            dialog.present();
        });

        let package = app.package.clone();
        let tx_open = tx.clone();
        open.connect_clicked(move |_| spawn_action(tx_open.clone(), &["launch", &package]));
        let package = app.package.clone();
        let tx_stop = tx.clone();
        stop.connect_clicked(move |_| spawn_action(tx_stop.clone(), &["stop", &package]));

        ui::flow_add(flow, &card, 340);
    }
}

fn build_resources(tx: &Sender<Msg>) -> ResourcesPage {
    let body = ui::page_body();
    let head = ui::header(
        "MONITOR",
        "Recursos",
        "CPU instantánea · RAM · GPU global del host",
    );
    let refresh = gtk::Button::with_label("Actualizar");
    head.action(&refresh);
    body.append(&head.root);

    let metrics_row = ui::flow(3, 190);
    let cpu = ui::metric("CPU");
    let ram = ui::metric("RAM");
    let gpu = ui::metric("GPU");
    ui::flow_add(&metrics_row, &cpu.root, 190);
    ui::flow_add(&metrics_row, &ram.root, 190);
    ui::flow_add(&metrics_row, &gpu.root, 190);
    body.append(&metrics_row);

    let summary = ui::note("Total models-apk: consultando…");
    body.append(&summary);

    body.append(&ui::section("GPU"));
    let gpus = ui::flow(3, 220);
    body.append(&gpus);

    body.append(&ui::section("Procesos del proyecto"));
    let processes = gtk::ListBox::new();
    processes.set_selection_mode(gtk::SelectionMode::None);
    processes.add_css_class("processes");
    let table = ui::card();
    table.append(&processes);
    body.append(&table);
    body.append(&ui::note(
        "CPU >100% es normal: suma hilos y equivale a varios núcleos. La GPU se muestra global; el kernel no la atribuye por proceso. btop lista apkstartd, qemu y scrcpy por separado.",
    ));

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| spawn_resources(tx_refresh.clone()));

    ResourcesPage {
        root: ui::page(&body),
        metrics: Metrics {
            cpu_value: cpu.value,
            cpu_sub: cpu.sub,
            ram_value: ram.value,
            ram_sub: ram.sub,
            gpu_value: gpu.value,
            gpu_sub: gpu.sub,
        },
        gpus,
        processes,
        summary,
    }
}

fn update_metrics(metrics: &Metrics, report: &ResourceReport) {
    metrics
        .cpu_value
        .set_text(&format!("{:.0}%", report.total_cpu));
    metrics.cpu_sub.set_text(&format!(
        "≈ {:.1} de {} hilos",
        report.total_cpu / 100.0,
        report.host_cpus
    ));

    let ram_mib = report.total_rss_kb as f64 / 1024.0;
    if ram_mib >= 1024.0 {
        metrics
            .ram_value
            .set_text(&format!("{:.1} GiB", ram_mib / 1024.0));
    } else {
        metrics.ram_value.set_text(&format!("{ram_mib:.0} MiB"));
    }
    metrics
        .ram_sub
        .set_text(&format!("{} procesos del proyecto", report.processes.len()));

    if let Some(gpu) = report.gpus.iter().max_by_key(|gpu| gpu.busy) {
        metrics.gpu_value.set_text(&format!("{}%", gpu.busy));
        metrics.gpu_sub.set_text(&format!(
            "{} · VRAM {:.2}/{:.2} GiB",
            gpu.name,
            gpu.vram_used as f64 / 1_073_741_824.0,
            gpu.vram_total as f64 / 1_073_741_824.0
        ));
    } else {
        metrics.gpu_value.set_text("—");
        metrics.gpu_sub.set_text("sin métricas sysfs");
    }
}

fn fill_resources(page: &ResourcesPage, report: &ResourceReport) {
    update_metrics(&page.metrics, report);
    page.summary.set_text(&format!(
        "Total models-apk: {:.1}% CPU · {:.1} GiB RAM · {} procesos",
        report.total_cpu,
        report.total_rss_kb as f64 / 1_073_741.824,
        report.processes.len()
    ));

    clear_flow(&page.gpus);
    if report.gpus.is_empty() {
        page.gpus
            .insert(&note_card("Sin métricas sysfs de GPU en este host."), -1);
    } else {
        for gpu in &report.gpus {
            let card = ui::card();
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let name = gtk::Label::new(Some(&gpu.name));
            name.set_xalign(0.0);
            name.add_css_class("mono");
            name.set_hexpand(true);
            let class = if gpu.busy >= 85 {
                "hot"
            } else if gpu.busy >= 60 {
                "warn"
            } else {
                "ok"
            };
            let badge = ui::pill(&format!("{}%", gpu.busy), class);
            top.append(&name);
            top.append(&badge);
            let bar = gtk::ProgressBar::new();
            bar.set_fraction(gpu.busy as f64 / 100.0);
            bar.add_css_class("gpu-bar");
            let vram = ui::note(&format!(
                "VRAM {:.2} / {:.2} GiB",
                gpu.vram_used as f64 / 1_073_741_824.0,
                gpu.vram_total as f64 / 1_073_741_824.0
            ));
            card.append(&top);
            card.append(&bar);
            card.append(&vram);
            ui::flow_add(&page.gpus, &card, 220);
        }
    }

    clear_list(&page.processes);
    let header = gtk::Label::new(Some(&format!(
        "{:<22} {:>7} {:>7} {:>9}",
        "PROCESO", "PID", "CPU", "RAM"
    )));
    header.set_xalign(0.0);
    header.add_css_class("mono");
    header.add_css_class("dim-label");
    let header_row = gtk::ListBoxRow::new();
    header_row.set_child(Some(&header));
    header_row.set_activatable(false);
    page.processes.append(&header_row);

    for process in &report.processes {
        let row = gtk::ListBoxRow::new();
        let label = gtk::Label::new(Some(&format!(
            "{:<22} {:>7} {:>6.1}% {:>7.0} MB",
            pad_name(&process.name, 22),
            process.pid,
            process.cpu,
            process.rss_kb as f64 / 1024.0
        )));
        label.set_xalign(0.0);
        label.add_css_class("mono");
        label.add_css_class("process-row");
        row.set_child(Some(&label));
        row.set_activatable(false);
        page.processes.append(&row);
    }
}

fn build_network() -> gtk::Widget {
    let body = ui::page_body();
    let head = ui::header("NETWORK", "Red", "NAT del host · slirp · sin root");
    ui::set_pill(&head.pill, "NAT", "ok");
    body.append(&head.root);

    let proxy = std::env::var("http_proxy")
        .or_else(|_| std::env::var("HTTP_PROXY"))
        .unwrap_or_else(|_| "no definido".to_string());

    let grid = ui::flow(2, 300);
    ui::flow_add(
        &grid,
        &ui::info_card(
            "MODO",
            "Red del sistema",
            "slirp · 10.0.2.15 · el tráfico sale por el host",
            Some(("recomendado", "ok")),
        ),
        300,
    );
    ui::flow_add(
        &grid,
        &ui::info_card(
            "PROXY DEL HOST",
            &proxy,
            "Se hereda a través del NAT si está definido.",
            None,
        ),
        300,
    );
    ui::flow_add(
        &grid,
        &ui::info_card(
            "DNS",
            "systemd-resolved",
            "El guest resuelve a través del host.",
            None,
        ),
        300,
    );
    ui::flow_add(
        &grid,
        &ui::info_card(
            "LIMITACIONES",
            "LAN y mDNS",
            "Chromecast y descubrimiento local no son transparentes. Proxy/DNS/puertos avanzados: Fase 4.",
            Some(("fase 4", "warn")),
        ),
        300,
    );
    body.append(&grid);
    body.append(&ui::note(
        "La VPN/proxy del host se aplica porque el tráfico sale por el host. No hay modo bridge en el MVP.",
    ));
    ui::page(&body)
}

fn build_peripherals() -> gtk::Widget {
    let body = ui::page_body();
    let head = ui::header("DEVICES", "Periféricos", "Audio, entrada y sensores");
    ui::set_pill(&head.pill, "MVP", "accent");
    body.append(&head.root);

    let items = [
        (
            "AUDIO",
            "Salida del emulador",
            "Mezcla global de Android y audio de scrcpy.",
            Some(("activo", "ok")),
        ),
        (
            "PORTAPAPELES",
            "Sincronización",
            "Texto y URL entre el host y Android mediante scrcpy.",
            Some(("activo", "ok")),
        ),
        (
            "CÁMARA",
            "Emulada",
            "Webcam del host pendiente de validación.",
            Some(("parcial", "warn")),
        ),
        (
            "MICRÓFONO",
            "Portal / PipeWire",
            "Pendiente de integración con el portal del escritorio.",
            Some(("pendiente", "")),
        ),
        (
            "GAMEPAD",
            "UHID de scrcpy",
            "Falta prueba y mapeo de mandos.",
            Some(("pendiente", "")),
        ),
        (
            "USB",
            "USB host",
            "No soportado en el MVP.",
            Some(("no soportado", "hot")),
        ),
    ];
    let grid = ui::flow(2, 300);
    for (eyebrow, value, sub, badge) in items {
        ui::flow_add(&grid, &ui::info_card(eyebrow, value, sub, badge), 300);
    }
    body.append(&grid);
    body.append(&ui::note(
        "El audio se mezcla globalmente en Android; scrcpy sincroniza el portapapeles. Cámara, micrófono y gamepad son Fase 4.",
    ));
    ui::page(&body)
}

fn build_integration(tx: &Sender<Msg>) -> IntegrationPage {
    let body = ui::page_body();
    let head = ui::header(
        "INTEGRATION",
        "Integración",
        "APK por doble clic, ./app.apk, daemon y timer",
    );
    let overall = head.pill.clone();
    let setup = gtk::Button::with_label("Reinstalar integración");
    setup.add_css_class("suggested-action");
    let sync = gtk::Button::with_label("Sincronizar apps");
    head.action(&sync);
    head.action(&setup);
    body.append(&head.root);

    let grid = ui::flow(2, 300);
    let (mime_card, mime) = check_card("MIME APK");
    let (binfmt_card, binfmt) = check_card("./APP.APK");
    let (daemon_card, daemon) = check_card("APKSTARTD");
    let (timer_card, timer) = check_card("TIMER LEGADO");
    let (shared_card, shared) = check_card("CARPETA COMPARTIDA");
    ui::flow_add(&grid, &mime_card, 300);
    ui::flow_add(&grid, &binfmt_card, 300);
    ui::flow_add(&grid, &daemon_card, 300);
    ui::flow_add(&grid, &timer_card, 300);
    ui::flow_add(&grid, &shared_card, 300);
    body.append(&grid);
    body.append(&ui::note(
        "La carpeta compartida, notificaciones y proxy son Fase 4. Reinstalar ejecuta `apkstart setup`.",
    ));

    let tx_setup = tx.clone();
    setup.connect_clicked(move |_| spawn_action(tx_setup.clone(), &["setup"]));
    let tx_sync = tx.clone();
    sync.connect_clicked(move |_| spawn_action(tx_sync.clone(), &["sync"]));

    IntegrationPage {
        root: ui::page(&body),
        mime,
        binfmt,
        daemon,
        timer,
        shared,
        overall,
    }
}

fn check_card(eyebrow: &str) -> (gtk::Box, CheckWidgets) {
    let card = ui::card();
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(eyebrow));
    label.add_css_class("eyebrow");
    label.set_xalign(0.0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let pill = ui::pill("…", "");
    top.append(&label);
    top.append(&spacer);
    top.append(&pill);
    let value = gtk::Label::new(Some("consultando…"));
    value.set_xalign(0.0);
    value.set_wrap(true);
    value.set_max_width_chars(44);
    value.add_css_class("mono");
    card.append(&top);
    card.append(&value);
    (card, CheckWidgets { value, pill })
}

fn fill_integration(page: &IntegrationPage, status: &IntegrationStatus) {
    let mime_ok = !status.mime.contains("no registrado");
    page.mime.value.set_text(&status.mime);
    ui::set_pill(
        &page.mime.pill,
        if mime_ok { "OK" } else { "pendiente" },
        if mime_ok { "ok" } else { "warn" },
    );

    page.binfmt.value.set_text(if status.binfmt {
        "registrado en binfmt_misc"
    } else {
        "no registrado"
    });
    ui::set_pill(
        &page.binfmt.pill,
        if status.binfmt { "OK" } else { "pendiente" },
        if status.binfmt { "ok" } else { "warn" },
    );

    let daemon_ok = status.daemon == "active";
    page.daemon
        .value
        .set_text(&format!("apkstartd · {}", status.daemon));
    ui::set_pill(
        &page.daemon.pill,
        if daemon_ok { "activo" } else { "revisar" },
        if daemon_ok { "ok" } else { "warn" },
    );

    let timer_ok = status.timer == "inactive" || status.timer == "unknown";
    page.timer
        .value
        .set_text(&format!("apkstart-sync.timer · {}", status.timer));
    ui::set_pill(
        &page.timer.pill,
        if timer_ok { "legado" } else { "activo" },
        if timer_ok { "" } else { "warn" },
    );

    page.shared.value.set_text(if status.shared_folder {
        "~/ApkStartShared existe"
    } else {
        "~/ApkStartShared pendiente (Fase 4)"
    });
    ui::set_pill(
        &page.shared.pill,
        if status.shared_folder { "OK" } else { "fase 4" },
        if status.shared_folder { "ok" } else { "warn" },
    );

    let ok_count = [
        mime_ok,
        status.binfmt,
        daemon_ok,
        timer_ok,
        status.shared_folder,
    ]
    .into_iter()
    .filter(|ok| *ok)
    .count();
    ui::set_pill(
        &page.overall,
        &format!("{ok_count}/5"),
        if ok_count == 5 { "ok" } else { "warn" },
    );
}

fn build_diagnostics(tx: &Sender<Msg>) -> DiagnosticsPage {
    let body = ui::page_body();
    let head = ui::header("DIAGNOSTICS", "Diagnóstico", "Doctor y logs en vivo");
    let doctor_button = gtk::Button::with_label("Ejecutar doctor");
    doctor_button.add_css_class("suggested-action");
    let export = gtk::Button::with_label("Exportar logs");
    head.action(&doctor_button);
    head.action(&export);
    body.append(&head.root);

    let grid = ui::flow(2, 380);

    let doctor_view = gtk::TextView::new();
    doctor_view.set_editable(false);
    doctor_view.set_monospace(true);
    doctor_view.set_wrap_mode(gtk::WrapMode::WordChar);
    doctor_view.set_top_margin(8);
    doctor_view.set_bottom_margin(8);
    doctor_view.set_left_margin(8);
    doctor_view.set_right_margin(8);
    let doctor_card = ui::card();
    doctor_card.append(&ui::card_title("Doctor"));
    doctor_card.append(&ui::note("Comprobaciones del sistema bajo demanda."));
    let doctor_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(360)
        .vexpand(true)
        .child(&doctor_view)
        .build();
    doctor_card.append(&doctor_scrolled);
    ui::flow_add(&grid, &doctor_card, 380);

    let logs_view = gtk::TextView::new();
    logs_view.set_editable(false);
    logs_view.set_monospace(true);
    logs_view.set_wrap_mode(gtk::WrapMode::WordChar);
    logs_view.set_top_margin(8);
    logs_view.set_bottom_margin(8);
    logs_view.set_left_margin(8);
    logs_view.set_right_margin(8);
    logs_view.buffer().set_text(&read_logs());
    let logs_card = ui::card();
    logs_card.append(&ui::card_title("Logs en vivo"));
    logs_card.append(&ui::note(
        "Daemon, emulador y última sesión · refresco cada 2 s.",
    ));
    let logs_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(360)
        .vexpand(true)
        .child(&logs_view)
        .build();
    logs_card.append(&logs_scrolled);
    ui::flow_add(&grid, &logs_card, 380);
    body.append(&grid);

    let logs_buffer = logs_view.buffer();
    gtk::glib::timeout_add_local(Duration::from_secs(2), move || {
        let text = read_logs();
        let current = logs_buffer.text(&logs_buffer.start_iter(), &logs_buffer.end_iter(), true);
        if current.as_str() != text {
            logs_buffer.set_text(&text);
        }
        gtk::glib::ControlFlow::Continue
    });

    let tx_run = tx.clone();
    doctor_button.connect_clicked(move |_| spawn_doctor(tx_run.clone()));
    let tx_export = tx.clone();
    export.connect_clicked(move |_| spawn_action(tx_export.clone(), &["export-log"]));

    DiagnosticsPage {
        root: ui::page(&body),
        doctor: doctor_view,
    }
}

fn build_about() -> gtk::Widget {
    let body = ui::page_body();
    let head = ui::header(
        "ABOUT",
        "Acerca de",
        "Gestor de aplicaciones Android para Linux",
    );
    ui::set_pill(
        &head.pill,
        &format!("v{} · MIT", env!("CARGO_PKG_VERSION")),
        "accent",
    );
    body.append(&head.root);

    let hero = ui::card();
    hero.add_css_class("hero");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_halign(gtk::Align::Center);
    let mark = ui::brand_mark(56, "preferences-system-symbolic");
    let name = gtk::Label::new(Some("models-apk"));
    name.add_css_class("page-title");
    let sub = ui::note("Android Emulator + scrcpy + Wayland, sin Electron ni Node.");
    sub.set_xalign(0.5);
    sub.set_justify(gtk::Justification::Center);
    content.append(&mark);
    content.append(&name);
    content.append(&sub);
    hero.append(&content);
    body.append(&hero);

    let grid = ui::flow(2, 300);
    for (eyebrow, value, sub) in [
        ("PROYECTO", "models-apk", "Gestor Android para Linux"),
        ("LICENCIA", "MIT", "Código abierto"),
        ("MOTOR", "Android Emulator", "QEMU · KVM · API 34"),
        ("VENTANAS", "Wayland", "scrcpy + SDL3"),
    ] {
        ui::flow_add(&grid, &ui::info_card(eyebrow, value, sub, None), 300);
    }
    body.append(&grid);

    body.append(&ui::section("Pendiente y parcial"));
    body.append(&ui::note(
        "Parcial: funciona con límites conocidos. Pendiente: aún no implementado. Limitación: fuera del alcance del MVP.",
    ));
    let status_grid = ui::flow(2, 300);
    for (eyebrow, value, sub, badge, class) in [
        (
            "DISTRIBUCIÓN",
            "Actualizaciones automáticas",
            "Comprobación de versión y descarga desde GitHub Releases. Fase 2.",
            "pendiente",
            "",
        ),
        (
            "DISTRIBUCIÓN",
            "Paquetes .deb · .rpm · Arch",
            "Instaladores por distro y script curl. Fase 2.",
            "pendiente",
            "",
        ),
        (
            "SESIONES",
            "Persistencia y adopción",
            "scrcpy no se re-adopta tras reiniciar el daemon; tabla sessions v3 pendiente.",
            "pendiente",
            "",
        ),
        (
            "MULTIMONITOR",
            "Foco y posición",
            "KWin decide la posición; falta traer la ventana al monitor activo.",
            "parcial",
            "warn",
        ),
        (
            "ARCHIVOS",
            "Carpeta compartida",
            "~/ApkStartShared y arrastrar archivos. Fase 4.",
            "pendiente",
            "",
        ),
        (
            "NOTIFICACIONES",
            "Android → escritorio",
            "Reenvío de notificaciones Android al escritorio. Fase 4.",
            "pendiente",
            "",
        ),
        (
            "RED",
            "Proxy, DNS y puertos",
            "LAN, mDNS y Chromecast limitados por NAT. Fase 4.",
            "parcial",
            "warn",
        ),
        (
            "PERIFÉRICOS",
            "Cámara, micro y gamepad",
            "Audio y portapapeles funcionan; el resto está pendiente.",
            "parcial",
            "warn",
        ),
        (
            "ICONOS",
            "Adaptive icons",
            "Bitmap o fallback con inicial; falta el icono adaptativo real.",
            "parcial",
            "warn",
        ),
        (
            "PLAY STORE",
            "Imagen no redistribuible",
            "El usuario instala y acepta el SDK de Google.",
            "limitación",
            "hot",
        ),
        (
            "COMPATIBILIDAD",
            "ARM-only · DRM · anticheat",
            "El motor x86_64 no ejecuta binarios ARM; DRM/anticheat puede fallar.",
            "limitación",
            "hot",
        ),
        (
            "VISOR",
            "Visor propio",
            "Hoy se usa scrcpy; el visor nativo es Fase 3.",
            "pendiente",
            "",
        ),
    ] {
        ui::flow_add(
            &status_grid,
            &ui::info_card(eyebrow, value, sub, Some((badge, class))),
            300,
        );
    }
    body.append(&status_grid);
    body.append(&ui::note(
        "github.com/Omegaiso/models-apk · El SDK y la imagen de Android los instala y acepta el usuario.",
    ));
    ui::page(&body)
}

fn app_icon(app: &App, size: i32) -> gtk::Image {
    let image = app
        .icon_path
        .as_ref()
        .filter(|path| path.is_file())
        .map(gtk::Image::from_file)
        .unwrap_or_else(|| gtk::Image::from_icon_name("application-x-executable"));
    image.set_pixel_size(size);
    image
}

fn note_card(text: &str) -> gtk::Box {
    let card = ui::card();
    card.append(&ui::note(text));
    card
}

fn error_card(message: &str) -> gtk::Box {
    let card = ui::card();
    let label = ui::note(message);
    label.add_css_class("error");
    card.append(&label);
    card
}

fn clear_flow(flow: &gtk::FlowBox) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
}

fn pad_name(name: &str, width: usize) -> String {
    if name.chars().count() > width {
        let mut shortened = name
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>();
        shortened.push('…');
        shortened
    } else {
        format!("{name:<width$}")
    }
}

fn spawn_resources(tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = resource_report();
        let _ = tx.send(Msg::Resources(result));
    });
}

fn resource_report() -> Result<ResourceReport, String> {
    let output = Command::new("ps")
        .args(["-eo", "pid=,comm=,rss=,pcpu=,args=", "--no-headers"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut processes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let Some(pid) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(_comm) = fields.next() else {
            continue;
        };
        let Some(rss_kb) = fields.next().and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        let Some(average_cpu) = fields.next().and_then(|value| value.parse::<f32>().ok()) else {
            continue;
        };
        let args = fields.collect::<Vec<_>>().join(" ");
        if !is_project_process(&args) {
            continue;
        }
        let name = if args.contains("qemu-system-x86_64-headless") {
            "Emulador Android"
        } else if args.contains("scrcpy") {
            "scrcpy (ventana)"
        } else if args.contains("apkstartd") {
            "apkstartd (daemon)"
        } else if args.contains("apkstart-settings") {
            "UI ajustes"
        } else {
            "ADB del proyecto"
        };
        processes.push(ResourceProcess {
            pid,
            name: name.to_string(),
            cpu: average_cpu,
            rss_kb,
        });
    }

    let host_cpus = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let total_ticks = read_total_cpu_ticks();
    let pids_now = processes
        .iter()
        .filter_map(|process| read_pid_cpu_ticks(process.pid).map(|ticks| (process.pid, ticks)))
        .collect::<HashMap<_, _>>();
    if let Ok(mut sample) = CPU_SAMPLE.get_or_init(|| Mutex::new(None)).lock() {
        if let Some(previous) = sample.as_ref() {
            let delta_total = total_ticks.saturating_sub(previous.total) as f32;
            if delta_total > 0.0 {
                for process in &mut processes {
                    if let (Some(now), Some(before)) =
                        (pids_now.get(&process.pid), previous.pids.get(&process.pid))
                    {
                        process.cpu = now.saturating_sub(*before) as f32 / delta_total
                            * host_cpus as f32
                            * 100.0;
                    }
                }
            }
        }
        *sample = Some(CpuSnapshot {
            total: total_ticks,
            pids: pids_now,
        });
    }

    processes.sort_by(|left, right| left.name.cmp(&right.name));
    let total_cpu = processes.iter().map(|process| process.cpu).sum();
    let total_rss_kb = processes.iter().map(|process| process.rss_kb).sum();
    Ok(ResourceReport {
        processes,
        total_cpu,
        total_rss_kb,
        host_cpus,
        gpus: gpu_report(),
    })
}

fn read_total_cpu_ticks() -> u64 {
    let Ok(stat) = fs::read_to_string("/proc/stat") else {
        return 0;
    };
    stat.lines()
        .next()
        .and_then(|line| line.strip_prefix("cpu "))
        .map(|values| {
            values
                .split_whitespace()
                .filter_map(|value| value.parse::<u64>().ok())
                .sum()
        })
        .unwrap_or(0)
}

fn read_pid_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let end = stat.rfind(')')?;
    let fields = stat[end + 2..].split_whitespace().collect::<Vec<_>>();
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    Some(utime + stime)
}

fn gpu_report() -> Vec<GpuReport> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut gpus = Vec::new();
    for entry in entries.flatten() {
        let card = entry.file_name().to_string_lossy().into_owned();
        if !card.starts_with("card") || card.contains('-') {
            continue;
        }
        let device = entry.path().join("device");
        let Some(busy) = fs::read_to_string(device.join("gpu_busy_percent"))
            .ok()
            .and_then(|value| value.trim().parse::<u8>().ok())
        else {
            continue;
        };
        let vram_used = fs::read_to_string(device.join("mem_info_vram_used"))
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let vram_total = fs::read_to_string(device.join("mem_info_vram_total"))
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let name = fs::read_to_string(device.join("uevent"))
            .ok()
            .and_then(|uevent| {
                uevent
                    .lines()
                    .find_map(|line| line.strip_prefix("PCI_ID=").map(str::to_string))
            })
            .map(|pci| format!("GPU {pci}"))
            .unwrap_or(card);
        gpus.push(GpuReport {
            name,
            busy,
            vram_used,
            vram_total,
        });
    }
    gpus.sort_by(|left, right| left.name.cmp(&right.name));
    gpus
}

fn is_project_process(args: &str) -> bool {
    args.contains("apkstartd")
        || (args.contains("qemu-system-x86_64-headless") && args.contains("-avd apkstart"))
        || (args.contains("scrcpy") && args.contains("emulator-5556"))
        || args.contains("apkstart-settings")
        || (args.contains("adb") && args.contains("5038"))
}

fn read_logs() -> String {
    let mut output = String::from("=== apkstartd ===\n");
    if let Ok(logs) = Command::new("journalctl")
        .args(["--user", "-u", "apkstartd", "-n", "80", "--no-pager"])
        .output()
    {
        output.push_str(&String::from_utf8_lossy(&logs.stdout));
    }

    output.push_str("\n=== emulador ===\n");
    output.push_str(&tail_file(&state_dir().join("emulator.log"), 80));

    if let Ok(entries) = fs::read_dir(state_dir().join("sessions")) {
        let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            if latest
                .as_ref()
                .map(|(time, _)| modified > *time)
                .unwrap_or(true)
            {
                latest = Some((modified, path));
            }
        }
        if let Some((_, path)) = latest {
            output.push_str(&format!("\n=== última sesión: {} ===\n", path.display()));
            output.push_str(&tail_file(&path, 80));
        }
    }
    output
}

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/state")
        })
        .join("apkstart")
}

fn tail_file(path: &Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return format!("(no existe {})\n", path.display());
    };
    let all = text.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

fn spawn_integration(tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = integration_status();
        let _ = tx.send(Msg::Integration(result));
    });
}

fn integration_status() -> Result<IntegrationStatus, String> {
    let mime = Command::new("xdg-mime")
        .args([
            "query",
            "default",
            "application/vnd.android.package-archive",
        ])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    let binfmt = Path::new("/proc/sys/fs/binfmt_misc/models-apk").exists();
    let daemon = Command::new("systemctl")
        .args(["--user", "is-active", "apkstartd.service"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "desconocido".to_string());
    let timer = Command::new("systemctl")
        .args(["--user", "is-active", "apkstart-sync.timer"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "desconocido".to_string());
    let shared_folder = dirs::home_dir()
        .map(|home| home.join("ApkStartShared").is_dir())
        .unwrap_or(false);
    Ok(IntegrationStatus {
        mime: if mime.is_empty() {
            "no registrado".to_string()
        } else {
            mime
        },
        binfmt,
        daemon,
        timer,
        shared_folder,
    })
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
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
