use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
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
    Resources(Result<ResourceReport, String>),
    Doctor(String),
    ActionResult(Result<String, String>),
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

    let resources_list = gtk::ListBox::new();
    resources_list.set_selection_mode(gtk::SelectionMode::None);
    resources_list.set_hexpand(true);
    let resources_summary = gtk::Label::new(Some("Recursos: consultando…"));
    resources_summary.set_xalign(0.0);
    resources_summary.set_wrap(true);
    let gpu_summary = gtk::Label::new(Some("GPU: consultando…"));
    gpu_summary.set_xalign(0.0);
    gpu_summary.set_wrap(true);
    let resources = build_resources(&tx, &resources_list, &resources_summary, &gpu_summary);

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
    stack.add_named(&resources, Some("resources"));
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
        ("Recursos", "utilities-system-monitor-symbolic", "resources"),
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
    let window_for_handler = window.clone();
    let status_for_handler = status_label.clone();
    let apps_for_handler = apps_list.clone();
    let resources_list_for_handler = resources_list.clone();
    let resources_summary_for_handler = resources_summary.clone();
    let gpu_summary_for_handler = gpu_summary.clone();
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
                    Ok(apps) => {
                        fill_apps(&apps_for_handler, &apps, &tx_handler, &window_for_handler)
                    }
                    Err(error) => {
                        clear_list(&apps_for_handler);
                        apps_for_handler.append(&error_row(&error));
                    }
                },
                Msg::Resources(result) => match result {
                    Ok(report) => {
                        fill_resources(
                            &resources_list_for_handler,
                            &resources_summary_for_handler,
                            &gpu_summary_for_handler,
                            &report,
                        );
                    }
                    Err(error) => {
                        resources_summary_for_handler
                            .set_text(&format!("Recursos: error\n{error}"));
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
        spawn_resources(tx_refresh.clone());
    });

    let tx_resources = tx.clone();
    gtk::glib::timeout_add_local(Duration::from_secs(2), move || {
        spawn_resources(tx_resources.clone());
        gtk::glib::ControlFlow::Continue
    });

    spawn_status(tx.clone());
    spawn_apps(tx.clone());
    spawn_resources(tx.clone());
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

fn build_resources(
    tx: &Sender<Msg>,
    list: &gtk::ListBox,
    summary: &gtk::Label,
    gpu_summary: &gtk::Label,
) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(18);
    page.set_margin_bottom(18);
    page.set_margin_start(18);
    page.set_margin_end(18);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Recursos"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    title.set_hexpand(true);
    let refresh = gtk::Button::with_label("Actualizar");
    toolbar.append(&title);
    toolbar.append(&refresh);
    page.append(&toolbar);

    summary.add_css_class("dim-label");
    page.append(summary);

    gpu_summary.add_css_class("dim-label");
    page.append(gpu_summary);

    let note = gtk::Label::new(Some(
        "CPU >100% es normal: suma hilos y equivale a varios núcleos. La GPU se muestra global; el kernel no la atribuye por proceso. btop muestra apkstartd, qemu y scrcpy por separado; esta vista los agrupa.",
    ));
    note.set_xalign(0.0);
    note.set_wrap(true);
    note.add_css_class("dim-label");
    page.append(&note);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(list));
    page.append(&scrolled);

    let tx_refresh = tx.clone();
    refresh.connect_clicked(move |_| spawn_resources(tx_refresh.clone()));
    page.upcast()
}

fn fill_resources(
    list: &gtk::ListBox,
    summary: &gtk::Label,
    gpu_summary: &gtk::Label,
    report: &ResourceReport,
) {
    clear_list(list);
    summary.set_text(&format!(
        "Total models-apk: {:.1}% CPU (~{:.1} de {} hilos) · {:.1} MiB RAM · {} procesos",
        report.total_cpu,
        report.total_cpu / 100.0,
        report.host_cpus,
        report.total_rss_kb as f64 / 1024.0,
        report.processes.len()
    ));
    if report.gpus.is_empty() {
        gpu_summary.set_text("GPU: sin métricas sysfs disponibles");
    } else {
        let lines = report
            .gpus
            .iter()
            .map(|gpu| {
                format!(
                    "{}: {}% · VRAM {:.2}/{:.2} GiB",
                    gpu.name,
                    gpu.busy,
                    gpu.vram_used as f64 / 1_073_741_824.0,
                    gpu.vram_total as f64 / 1_073_741_824.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        gpu_summary.set_text(&format!("GPU global (no por proceso):\n{lines}"));
    }

    let header = gtk::Label::new(Some(&format!(
        "{:<24} {:>7} {:>8} {:>10}",
        "PROCESO", "PID", "CPU", "RAM"
    )));
    header.set_xalign(0.0);
    header.add_css_class("monospace");
    header.add_css_class("dim-label");
    let header_row = gtk::ListBoxRow::new();
    header_row.set_child(Some(&header));
    header_row.set_activatable(false);
    list.append(&header_row);

    for process in &report.processes {
        let row = gtk::ListBoxRow::new();
        let label = gtk::Label::new(Some(&format!(
            "{:<24} {:>7} {:>7.1}% {:>7.1} MB",
            process.name,
            process.pid,
            process.cpu,
            process.rss_kb as f64 / 1024.0
        )));
        label.set_xalign(0.0);
        label.add_css_class("monospace");
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(10);
        label.set_margin_end(10);
        row.set_child(Some(&label));
        list.append(&row);
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

    let doctor_page = gtk::ScrolledWindow::new();
    doctor_page.set_vexpand(true);
    doctor_page.set_child(Some(view));

    let logs_view = gtk::TextView::new();
    logs_view.set_editable(false);
    logs_view.set_monospace(true);
    logs_view.set_wrap_mode(gtk::WrapMode::WordChar);
    logs_view.buffer().set_text(&read_logs());
    let logs_page = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let logs_note = gtk::Label::new(Some(
        "En vivo cada 2 s: daemon, emulador y última sesión. Para una foto exacta usa Doctor.",
    ));
    logs_note.set_xalign(0.0);
    logs_note.add_css_class("dim-label");
    logs_page.append(&logs_note);
    let logs_scrolled = gtk::ScrolledWindow::new();
    logs_scrolled.set_vexpand(true);
    logs_scrolled.set_child(Some(&logs_view));
    logs_page.append(&logs_scrolled);

    let notebook = gtk::Notebook::new();
    notebook.set_vexpand(true);
    notebook.append_page(&doctor_page, Some(&gtk::Label::new(Some("Doctor"))));
    notebook.append_page(&logs_page, Some(&gtk::Label::new(Some("Logs en vivo"))));
    page.append(&notebook);

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
    run.connect_clicked(move |_| spawn_doctor(tx_run.clone()));
    let tx_export = tx.clone();
    export.connect_clicked(move |_| spawn_action(tx_export.clone(), &["export-log"]));
    page.upcast()
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

fn fill_apps(list: &gtk::ListBox, apps: &[App], tx: &Sender<Msg>, window: &gtk::ApplicationWindow) {
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

        let details = gtk::Button::with_label("Detalles");
        let open = gtk::Button::with_label("Abrir");
        let stop = gtk::Button::with_label("Cerrar");
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
            let dialog = gtk::MessageDialog::new(
                Some(&window_details),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Info,
                gtk::ButtonsType::None,
                &text,
            );
            dialog.set_title(Some(&app_details.label));
            dialog.add_button("Cerrar", gtk::ResponseType::Close);
            dialog.add_button("Desinstalar", gtk::ResponseType::Accept);
            let package = app_details.package.clone();
            let tx_uninstall = tx_details.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    spawn_action(tx_uninstall.clone(), &["uninstall", &package]);
                }
                dialog.close();
            });
            dialog.present();
        });

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
        content.append(&details);
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
