//! Polices IBM Plex Sans et IBM Plex Mono (licence OFL, `assets/fonts/OFL.txt`), embarquées dans le binaire.

use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily};

/// Famille des titres et libellés appuyés.
pub const SEMI_GRAS: &str = "plex-sans-semibold";

/// Installe les polices Plex en tête des familles d'egui ; les polices par défaut restent en secours (symboles).
pub fn installer(ctx: &Context) {
    let mut polices = FontDefinitions::default();
    let mut ajouter = |nom: &str, octets: &'static [u8]| {
        polices
            .font_data
            .insert(nom.to_owned(), Arc::new(FontData::from_static(octets)));
    };
    ajouter("plex-sans", include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"));
    ajouter(SEMI_GRAS, include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"));
    ajouter("plex-mono", include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"));

    let secours_sans = polices.families[&FontFamily::Proportional].clone();
    polices
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "plex-sans".to_owned());
    polices
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "plex-mono".to_owned());
    polices.families.insert(
        FontFamily::Name(SEMI_GRAS.into()),
        [vec![SEMI_GRAS.to_owned()], secours_sans].concat(),
    );
    ctx.set_fonts(polices);
}
