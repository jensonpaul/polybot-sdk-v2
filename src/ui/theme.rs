use eframe::egui::Color32;

pub struct Theme;

impl Theme {
    // =========================================================
    // BASE SURFACES
    // =========================================================

    pub const BG_PANEL: Color32 =
        Color32::from_rgb(
            14,
            17,
            22,
        );

    pub const BG_ELEVATED: Color32 =
        Color32::from_rgb(
            20,
            24,
            31,
        );

    pub const BORDER: Color32 =
        Color32::from_rgb(
            42,
            42,
            50,
        );

    // =========================================================
    // TEXT
    // =========================================================

    pub const TEXT_PRIMARY: Color32 =
        Color32::from_rgb(230, 230, 235);

    pub const TEXT_MUTED: Color32 =
        Color32::from_rgb(150, 150, 160);

        // =========================================================
    // BUY / POSITIVE
    // =========================================================

    pub const BUY_GREEN: Color32 =
        Color32::from_rgb(
            88,
            166,
            120,
        );

    pub const BUY_GREEN_BG: Color32 =
        Color32::from_rgb(
            22,
            38,
            30,
        );

    // =========================================================
    // SELL / NEGATIVE
    // =========================================================

    pub const SELL_RED: Color32 =
        Color32::from_rgb(
            196,
            94,
            94,
        );

    pub const SELL_RED_BG: Color32 =
        Color32::from_rgb(
            42,
            24,
            24,
        );

    // =========================================================
    // INFO
    // =========================================================

    pub const BLUE: Color32 =
        Color32::from_rgb(
            94,
            129,
            172,
        );

    pub const BLUE_BG: Color32 =
        Color32::from_rgb(
            24,
            30,
            42,
        );

    // =========================================================
    // WARNING
    // =========================================================

    pub const WARNING: Color32 =
        Color32::from_rgb(
            214,
            180,
            90,
        );
}