use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Margin, RichText};

pub const APP_TITLE: &str = "Maxion Protector GUI";
pub const ICON_FONT_FAMILY: &str = "material_symbols";

pub const ICON_APP: &str = "\u{e8b8}";
pub const ICON_FILE: &str = "\u{e24d}";
pub const ICON_FOLDER: &str = "\u{e2c7}";
pub const ICON_OUTPUT: &str = "\u{e2c6}";
pub const ICON_LOG: &str = "\u{e8b0}";
pub const ICON_DONE: &str = "\u{e876}";
pub const ICON_ERROR: &str = "\u{e000}";

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sans".to_owned(),
        FontData::from_static(include_bytes!("../assets/NotoSansThai.ttf")).into(),
    );
    fonts.font_data.insert(
        ICON_FONT_FAMILY.to_owned(),
        FontData::from_static(include_bytes!("../assets/MaterialSymbolsOutlined.ttf")).into(),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_sans".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "noto_sans".to_owned());
    fonts
        .families
        .entry(FontFamily::Name(ICON_FONT_FAMILY.into()))
        .or_default()
        .insert(0, ICON_FONT_FAMILY.to_owned());

    ctx.set_fonts(fonts);
}

pub fn configure_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 8.0);
    style.spacing.window_margin = Margin::same(10);
    style.visuals.override_text_color = Some(Color32::from_rgb(53, 46, 38));
    style.visuals.window_fill = Color32::from_rgb(245, 240, 231);
    style.visuals.panel_fill = Color32::from_rgb(245, 240, 231);
    style.visuals.extreme_bg_color = Color32::from_rgb(233, 226, 214);
    style.visuals.faint_bg_color = Color32::from_rgb(238, 232, 222);
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(245, 240, 231);
    style.visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(82, 74, 64);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(229, 221, 207);
    style.visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(53, 46, 38);
    style.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(186, 102, 48);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(201, 121, 64);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(221, 161, 92);
    style.visuals.selection.bg_fill = Color32::from_rgb(95, 124, 93);
    style.visuals.hyperlink_color = Color32::from_rgb(62, 102, 156);
    ctx.set_style(style);
}

pub fn icon(codepoint: &str, color: Color32, size: f32) -> RichText {
    RichText::new(codepoint)
        .family(FontFamily::Name(ICON_FONT_FAMILY.into()))
        .size(size)
        .color(color)
}
