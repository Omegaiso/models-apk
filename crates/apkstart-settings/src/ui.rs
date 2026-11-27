//! Componentes visuales compartidos de la UI de ajustes (variante B).

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

const CSS: &str = r#"
.card {
  background-color: @card_bg_color;
  border: 1px solid alpha(@borders, 0.85);
  border-radius: 14px;
  padding: 16px;
}
.card.compact { padding: 12px; }

.hero {
  background-image: linear-gradient(135deg, alpha(@accent_bg_color, 0.24), alpha(@accent_bg_color, 0.03));
  border: 1px solid alpha(@accent_bg_color, 0.24);
  border-radius: 18px;
  padding: 20px;
}

.eyebrow {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 1.2px;
  color: alpha(@window_fg_color, 0.58);
}
.page-title { font-size: 26px; font-weight: 800; }
.hero-title { font-size: 22px; font-weight: 800; }
.metric-value { font-size: 26px; font-weight: 800; letter-spacing: -0.4px; }
.metric-label { font-size: 10px; font-weight: 700; letter-spacing: 1px; color: alpha(@window_fg_color, 0.55); }
.card-title { font-size: 15px; font-weight: 700; }
.section-title { font-size: 15px; font-weight: 700; }
.mono { font-family: monospace; font-size: 12px; }

.pill {
  border-radius: 999px;
  padding: 2px 9px;
  font-size: 11px;
  font-weight: 600;
  background-color: alpha(@borders, 0.55);
  color: alpha(@window_fg_color, 0.78);
}
.pill.ok { background-color: alpha(@success_color, 0.16); color: @success_color; }
.pill.warn { background-color: alpha(@warning_color, 0.18); color: @warning_color; }
.pill.hot { background-color: alpha(@error_color, 0.16); color: @error_color; }
.pill.accent { background-color: alpha(@accent_bg_color, 0.16); color: @accent_color; }

flowboxchild { padding: 0; background-color: transparent; }

.brand-mark { background-color: @accent_bg_color; color: @accent_fg_color; border-radius: 12px; }
.brand-mark.large { border-radius: 16px; }

.processes { background: transparent; }
.process-row { padding: 3px 0; }

.gpu-bar { min-height: 6px; border-radius: 999px; }
.gpu-bar > trough { min-height: 6px; border-radius: 999px; background-color: alpha(@borders, 0.5); }
.gpu-bar > trough > progress { min-height: 6px; border-radius: 999px; background-color: @accent_bg_color; }
"#;

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Página con scroll vertical y ancho máximo, para que el contenido respire en
/// pantallas grandes sin estirarse en exceso.
pub fn page(content: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let clamp = adw::Clamp::builder()
        .maximum_size(1120)
        .tightening_threshold(920)
        .child(content)
        .build();
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build()
        .upcast()
}

pub fn page_body() -> gtk::Box {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    body.set_margin_top(24);
    body.set_margin_bottom(28);
    body.set_margin_start(26);
    body.set_margin_end(26);
    body
}

pub struct PageHeader {
    pub root: gtk::Box,
    pub actions: gtk::FlowBox,
    pub pill: gtk::Label,
}

pub fn header(eyebrow: &str, title: &str, subtitle: &str) -> PageHeader {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let eyebrow_label = gtk::Label::new(Some(eyebrow));
    eyebrow_label.add_css_class("eyebrow");
    eyebrow_label.set_xalign(0.0);
    let pill = gtk::Label::new(None);
    pill.add_css_class("pill");
    pill.set_visible(false);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&eyebrow_label);
    top.append(&spacer);
    top.append(&pill);

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("page-title");
    title_label.set_xalign(0.0);
    title_label.set_wrap(true);

    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);

    let actions = actions();

    root.append(&top);
    root.append(&title_label);
    root.append(&subtitle_label);
    root.append(&actions);
    PageHeader {
        root,
        actions,
        pill,
    }
}

impl PageHeader {
    pub fn action(&self, button: &impl IsA<gtk::Widget>) {
        self.actions.insert(button, -1);
    }
}

pub fn actions() -> gtk::FlowBox {
    let actions = gtk::FlowBox::new();
    actions.set_selection_mode(gtk::SelectionMode::None);
    actions.set_homogeneous(false);
    actions.set_max_children_per_line(6);
    actions.set_min_children_per_line(1);
    actions.set_column_spacing(8);
    actions.set_row_spacing(8);
    actions.set_halign(gtk::Align::Start);
    actions.set_valign(gtk::Align::Center);
    actions
}

pub fn set_pill(label: &gtk::Label, text: &str, class: &str) {
    label.set_text(text);
    for name in ["ok", "warn", "hot", "accent"] {
        label.remove_css_class(name);
    }
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label.set_visible(!text.is_empty());
}

pub fn pill(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("pill");
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

pub fn card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("card");
    card.set_hexpand(true);
    card.set_valign(gtk::Align::Fill);
    card
}

pub fn card_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("card-title");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

pub fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("section-title");
    label.set_xalign(0.0);
    label
}

pub fn note(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

pub struct Metric {
    pub root: gtk::Box,
    pub value: gtk::Label,
    pub sub: gtk::Label,
}

pub fn metric(name: &str) -> Metric {
    let root = card();
    let label = gtk::Label::new(Some(name));
    label.add_css_class("metric-label");
    label.set_xalign(0.0);
    let value = gtk::Label::new(Some("—"));
    value.add_css_class("metric-value");
    value.set_xalign(0.0);
    let sub = gtk::Label::new(Some("sin datos"));
    sub.add_css_class("dim-label");
    sub.set_xalign(0.0);
    sub.set_wrap(true);
    root.append(&label);
    root.append(&value);
    root.append(&sub);
    Metric { root, value, sub }
}

/// Contenedor que reordena sus tarjetas según el ancho disponible.
pub fn flow(max_children_per_line: u32, _min_width: i32) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_activate_on_single_click(false);
    flow.set_homogeneous(true);
    flow.set_max_children_per_line(max_children_per_line);
    flow.set_min_children_per_line(1);
    flow.set_column_spacing(12);
    flow.set_row_spacing(12);
    flow.set_halign(gtk::Align::Fill);
    flow.set_valign(gtk::Align::Start);
    flow.set_hexpand(true);
    flow
}

pub fn flow_add(flow: &gtk::FlowBox, child: &impl IsA<gtk::Widget>, min_width: i32) {
    child.set_size_request(min_width, -1);
    flow.insert(child, -1);
}

pub fn brand_mark(size: i32, icon: &str) -> gtk::CenterBox {
    let mark = gtk::CenterBox::new();
    mark.set_size_request(size, size);
    mark.add_css_class("brand-mark");
    if size >= 44 {
        mark.add_css_class("large");
    }
    mark.set_halign(gtk::Align::Center);
    mark.set_valign(gtk::Align::Center);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size((size as f64 * 0.5).round() as i32);
    mark.set_center_widget(Some(&image));
    mark
}

/// Tarjeta informativa: etiqueta, valor grande y descripción con pill opcional.
pub fn info_card(eyebrow: &str, value: &str, sub: &str, badge: Option<(&str, &str)>) -> gtk::Box {
    let card = card();
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(eyebrow));
    label.add_css_class("eyebrow");
    label.set_xalign(0.0);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&label);
    top.append(&spacer);
    if let Some((text, class)) = badge {
        top.append(&pill(text, class));
    }
    let value_label = gtk::Label::new(Some(value));
    value_label.add_css_class("card-title");
    value_label.set_xalign(0.0);
    value_label.set_wrap(true);
    value_label.set_max_width_chars(34);
    let sub_label = note(sub);
    sub_label.set_max_width_chars(44);
    card.append(&top);
    card.append(&value_label);
    card.append(&sub_label);
    card
}
