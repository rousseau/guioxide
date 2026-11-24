//! Thème sombre pour la lecture d'images, aligné sur les autres applications de l'utilisateur (rusty_music,
//! « finitions » du 24 novembre 2026) : fond « espresso », surfaces plates séparées par des traits d'un pixel, accent
//! rouille utilisé en teinte douce, coins arrondis (7 px pour les contrôles, 10 pour les cadres, 14 pour les fenêtres).

use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Vec2};

/// Fond des vues d'image et des champs.
pub const FOND: Color32 = Color32::from_rgb(0x14, 0x12, 0x0E);
/// Panneaux et fenêtres.
pub const PANNEAU: Color32 = Color32::from_rgb(0x1B, 0x18, 0x13);
/// Surfaces en relief : boutons, cadres, cartes.
pub const SURFACE: Color32 = Color32::from_rgb(0x22, 0x1E, 0x17);
pub const TEXTE: Color32 = Color32::from_rgb(0xED, 0xE8, 0xDC);
pub const TEXTE_SECONDAIRE: Color32 = Color32::from_rgb(0x9A, 0x92, 0x84);
/// Accent rouille : sélection, onglet actif, étape en cours.
pub const ACCENT: Color32 = Color32::from_rgb(0xC0, 0x7C, 0x4A);
/// Accent en teinte douce (15 %) : élément actif, ligne choisie, bouton principal.
pub const ACCENT_DOUX: Color32 = Color32::from_rgba_premultiplied(0x1D, 0x13, 0x0B, 0x26);
/// État « terminé ».
pub const TERMINE: Color32 = Color32::from_rgb(0x4E, 0xC4, 0x97);
pub const ERREUR: Color32 = Color32::from_rgb(0xDE, 0x6A, 0x50);
/// Traits de séparation et bordures.
pub const BORDURE: Color32 = Color32::from_rgb(0x2E, 0x29, 0x21);

/// Applique le thème sombre à toute l'application.
pub fn appliquer(ctx: &Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXTE);
    v.panel_fill = PANNEAU;
    v.window_fill = PANNEAU;
    v.extreme_bg_color = FOND;
    v.faint_bg_color = Color32::from_rgb(0x1F, 0x1C, 0x16);
    v.code_bg_color = SURFACE;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.30);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, BORDURE);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow {
        offset: [0, 18],
        blur: 48,
        spread: 0,
        color: Color32::from_black_alpha(82),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };
    v.striped = true;
    let controle = CornerRadius::same(7);
    let w = &mut v.widgets;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDURE);
    w.noninteractive.bg_fill = PANNEAU;
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXTE);
    w.noninteractive.corner_radius = controle;
    for etat in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        etat.corner_radius = controle;
        etat.bg_fill = SURFACE;
        etat.weak_bg_fill = SURFACE;
        etat.bg_stroke = Stroke::new(1.0, BORDURE);
        etat.fg_stroke = Stroke::new(1.0, TEXTE);
        etat.expansion = 0.0;
    }
    w.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    w.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
    w.active.weak_bg_fill = ACCENT.linear_multiply(0.25);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.open.weak_bg_fill = ACCENT.linear_multiply(0.15);
    ctx.set_visuals_of(Theme::Dark, v);
    ctx.set_theme(Theme::Dark);

    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(11.0, 4.0);
        s.spacing.window_margin = Margin::same(16);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.interact_size.y = 24.0;
        s.spacing.scroll = egui::style::ScrollStyle::thin();
        s.text_styles = [
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (
                TextStyle::Heading,
                FontId::new(19.0, FontFamily::Name(crate::polices::SEMI_GRAS.into())),
            ),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Titre de section : capitales, police à chasse fixe, petite taille, couleur secondaire.
pub fn titre_section(ui: &mut egui::Ui, texte: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(texte.to_uppercase())
            .monospace()
            .size(11.5)
            .color(TEXTE_SECONDAIRE),
    );
    ui.add_space(2.0);
}

/// Cadre de section (carte) : surface en relief, coins de 10 px.
pub fn carte() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE.linear_multiply(0.6))
        .stroke(Stroke::new(1.0, BORDURE))
        .corner_radius(10)
        .inner_margin(Margin::same(12))
}
