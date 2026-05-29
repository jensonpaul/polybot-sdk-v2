use eframe::egui;
use eframe::egui::Color32;

pub struct Theme;

impl Theme {
    pub const BG_PANEL: Color32 = Color32::from_rgb(14, 17, 22);
    pub const BG_ELEVATED: Color32 = Color32::from_rgb(20, 24, 31);
    pub const BORDER: Color32 = Color32::from_rgb(42, 42, 50);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 235);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 150, 160);
    pub const BUY_GREEN: Color32 = Color32::from_rgb(88, 166, 120);
    pub const BUY_GREEN_BG: Color32 = Color32::from_rgb(22, 38, 30);
    pub const SELL_RED: Color32 = Color32::from_rgb(196, 94, 94);
    pub const SELL_RED_BG: Color32 = Color32::from_rgb(42, 24, 24);
    pub const BLUE: Color32 = Color32::from_rgb(94, 129, 172);
    pub const BLUE_BG: Color32 = Color32::from_rgb(24, 30, 42);
    pub const WARNING: Color32 = Color32::from_rgb(214, 180, 90);
}

pub fn apply_dashboard_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Theme::BG_PANEL;
    v.window_fill = Theme::BG_PANEL;
    v.faint_bg_color = Theme::BG_ELEVATED;
    v.extreme_bg_color = Theme::BG_PANEL;
    v.override_text_color = Some(Theme::TEXT_PRIMARY);
    v.window_corner_radius = 8.0.into();
    v.window_stroke = egui::Stroke::new(1.0, Theme::BORDER);
    v.selection.bg_fill = Theme::BLUE;
    v.selection.stroke = egui::Stroke::new(1.0, Theme::TEXT_PRIMARY);
    ctx.set_visuals(v);
}
