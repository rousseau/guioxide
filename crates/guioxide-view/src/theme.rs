//! Thème sombre pour la lecture d'images (couleurs de `docs/interface.md`).

use egui::{Color32, Context, Stroke, Theme};

pub const FOND: Color32 = Color32::from_rgb(0x11, 0x13, 0x16);
pub const PANNEAU: Color32 = Color32::from_rgb(0x18, 0x1B, 0x20);
pub const TEXTE: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEB);
pub const TEXTE_SECONDAIRE: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAB);
/// Accent rouille : sélection, onglet actif, étape en cours.
pub const ACCENT: Color32 = Color32::from_rgb(0xE0, 0x7B, 0x39);
/// État « terminé ».
pub const TERMINE: Color32 = Color32::from_rgb(0x7F, 0xB3, 0xE6);
pub const ERREUR: Color32 = Color32::from_rgb(0xE5, 0x5C, 0x5C);
/// Traits de séparation et bordures.
pub const BORDURE: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x37);

/// Applique le thème sombre à toute l'application.
pub fn appliquer(ctx: &Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXTE);
    v.panel_fill = PANNEAU;
    v.window_fill = PANNEAU;
    v.extreme_bg_color = FOND;
    v.faint_bg_color = PANNEAU;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDURE);
    ctx.set_visuals_of(Theme::Dark, v);
    ctx.set_theme(Theme::Dark);
}
