//! Vue de coupe : un plan du monde à travers le curseur, couches superposées (image, étiquettes, contour).
//!
//! L'image est échantillonnée sur le CPU (au plus deux échantillons par voxel), composée en RVBA puis envoyée comme
//! texture ; elle n'est recalculée que si le plan, le cadrage ou le rendu d'une couche change.

use std::sync::Arc;

use egui::{Color32, ColorImage, Pos2, Rect, Response, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2};
use guioxide_data::{Maillage, TableEtiquettes};
use guioxide_geom::{Affine, Interpolation, Orientation, Plan, PointMonde, Segment, Volume};

use crate::theme;

/// Rendu d'une couche.
#[derive(Clone, Debug, PartialEq)]
pub enum Rendu {
    /// Niveaux de gris ; fenêtre `(niveau, largeur)`, ou automatique (centiles 1 % – 99 %) si `None`.
    Image { fenetre: Option<(f32, f32)> },
    /// Étiquettes colorées, superposées avec une opacité.
    Etiquettes { table: TableEtiquettes, opacite: f32 },
    /// Contour d'un masque (valeurs > ½).
    Contour { couleur: [u8; 3] },
}

/// Une couche affichée : un volume et son rendu.
#[derive(Clone)]
pub struct Couche {
    pub volume: Arc<Volume>,
    pub rendu: Rendu,
    pub visible: bool,
}

impl Couche {
    pub fn image(volume: Arc<Volume>) -> Couche {
        Couche {
            volume,
            rendu: Rendu::Image { fenetre: None },
            visible: true,
        }
    }
}

/// Une surface dont le contour (intersection avec le plan de coupe) est tracé sur les vues.
#[derive(Clone)]
pub struct Surface {
    pub maillage: Arc<Maillage>,
    pub couleur: [u8; 3],
    pub visible: bool,
}

/// Plan d'une vue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypeVue {
    Alignee(Orientation),
    /// Plan des coupes d'une pile (axes `i`, `j` de son affine).
    Native(Affine),
}

impl TypeVue {
    fn plan(&self, origine: [f64; 3]) -> Plan {
        match self {
            TypeVue::Alignee(o) => Plan::aligne(*o, origine),
            TypeVue::Native(a) => Plan::natif(a, origine),
        }
    }

    fn nom(&self) -> &'static str {
        match self {
            TypeVue::Alignee(o) => o.nom(),
            TypeVue::Native(_) => "native",
        }
    }
}

/// Ce qui détermine l'image composée : si rien ne change, la texture est réutilisée.
#[derive(Clone, PartialEq)]
struct Cle {
    plan: Plan,
    centre: (f64, f64),
    taille: [usize; 2],
    pas: f64,
    couches: Vec<(usize, Rendu, bool)>,
}

/// Vue de coupe : cadrage (centre et zoom) et texture courante. Le curseur est partagé, hors de la vue.
pub struct VueCoupe {
    pub type_vue: TypeVue,
    /// Centre de la vue (point monde) ; `None` : cadré sur la première couche au prochain affichage.
    centre: Option<[f64; 3]>,
    /// Points d'écran par mm.
    zoom: Option<f32>,
    defilement: f32,
    texture: Option<(TextureHandle, Cle, Rect)>,
    /// Contours des surfaces pour le plan courant : (maillage, plan, segments en coordonnées du plan).
    contours: Vec<(usize, Plan, Vec<Segment>)>,
}

impl VueCoupe {
    pub fn new(type_vue: TypeVue) -> VueCoupe {
        VueCoupe {
            type_vue,
            centre: None,
            zoom: None,
            defilement: 0.0,
            texture: None,
            contours: Vec::new(),
        }
    }

    /// Recadre sur les couches au prochain affichage.
    pub fn recadrer(&mut self) {
        self.centre = None;
        self.zoom = None;
    }

    /// Affiche la vue dans `rect` et traite les interactions. Renvoie la réponse de la zone.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        couches: &mut [Couche],
        surfaces: &[Surface],
        curseur: &mut PointMonde,
    ) -> Response {
        let reponse = ui.allocate_rect(rect, Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, theme::FOND);
        let Some(reference) = couches.iter().find(|c| c.visible).map(|c| c.volume.clone()) else {
            return reponse;
        };
        let plan = self.type_vue.plan(curseur.0);

        // Cadrage initial : centre du volume, zoom qui fait tenir sa boîte.
        let centre = *self.centre.get_or_insert(reference.centre().0);
        let zoom = *self.zoom.get_or_insert_with(|| {
            let (min, max) = reference.boite();
            let etendue = |d: [f64; 3]| (0..3).map(|c| (max[c] - min[c]) * d[c].abs()).sum::<f64>();
            let (lu, lv) = (etendue(plan.u), etendue(plan.v));
            (rect.width() / lu as f32).min(rect.height() / lv as f32) * 0.95
        });
        let plan = ancrer(plan, centre, curseur.0);
        let centre_plan = plan.vers_plan(centre);

        self.interactions(ui, &reponse, rect, &plan, centre_plan, couches, curseur, &reference);

        // Le curseur ou le cadrage ont pu bouger : plan recalculé.
        let centre = self.centre.unwrap_or(centre);
        let plan = ancrer(self.type_vue.plan(curseur.0), centre, curseur.0);
        let centre_plan = plan.vers_plan(centre);
        let zoom = self.zoom.unwrap_or(zoom);
        let finesse = couches
            .iter()
            .filter(|c| c.visible)
            .flat_map(|c| c.volume.taille_voxel())
            .fold(f64::MAX, f64::min);
        let ppp = ui.ctx().pixels_per_point();
        let pas = (1.0 / (zoom * ppp) as f64).max(finesse / 2.0);
        let taille = [
            ((rect.width() / zoom) as f64 / pas).ceil().max(1.0) as usize,
            ((rect.height() / zoom) as f64 / pas).ceil().max(1.0) as usize,
        ];
        let cle = Cle {
            plan,
            centre: centre_plan,
            taille,
            pas,
            couches: couches
                .iter()
                .map(|c| (Arc::as_ptr(&c.volume) as usize, c.rendu.clone(), c.visible))
                .collect(),
        };
        if self.texture.as_ref().is_none_or(|(_, k, _)| *k != cle) {
            let image = composer(&plan, centre_plan, taille, pas, couches);
            let options = TextureOptions::LINEAR;
            let etendue = Vec2::new(taille[0] as f32, taille[1] as f32) * (pas as f32 * zoom);
            let zone = Rect::from_center_size(rect.center(), etendue);
            match &mut self.texture {
                Some((t, k, z)) => {
                    t.set(image, options);
                    (*k, *z) = (cle, zone);
                }
                None => self.texture = Some((ui.ctx().load_texture("coupe", image, options), cle, zone)),
            }
        }
        let peintre = ui.painter_at(rect);
        if let Some((texture, _, zone)) = &self.texture {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            peintre.image(texture.id(), *zone, uv, Color32::WHITE);
        }

        // Contours des surfaces, recalculés seulement quand le plan change.
        let vers_ecran = |(a, b): (f64, f64)| {
            rect.center() + Vec2::new((a - centre_plan.0) as f32, -(b - centre_plan.1) as f32) * zoom
        };
        for s in surfaces.iter().filter(|s| s.visible) {
            let id = Arc::as_ptr(&s.maillage) as usize;
            let segments = match self.contours.iter().position(|(i, p, _)| *i == id && *p == plan) {
                Some(k) => &self.contours[k].2,
                None => {
                    self.contours.retain(|(i, _, _)| *i != id);
                    self.contours
                        .push((id, plan, plan.intersection(&s.maillage.sommets, &s.maillage.faces)));
                    &self.contours.last().unwrap().2
                }
            };
            let [r, g, b] = s.couleur;
            let trait_ = Stroke::new(1.5, Color32::from_rgb(r, g, b));
            for [p, q] in segments {
                peintre.line_segment([vers_ecran(*p), vers_ecran(*q)], trait_);
            }
        }

        // Réticule à la position du curseur dans le plan.
        let (a, b) = plan.vers_plan(curseur.0);
        let c = rect.center() + Vec2::new((a - centre_plan.0) as f32, -(b - centre_plan.1) as f32) * zoom;
        let trait_ = Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.7));
        peintre.line_segment([Pos2::new(rect.left(), c.y), Pos2::new(rect.right(), c.y)], trait_);
        peintre.line_segment([Pos2::new(c.x, rect.top()), Pos2::new(c.x, rect.bottom())], trait_);

        // Lettres d'orientation et nom de la vue.
        let police = egui::FontId::proportional(13.0);
        let lettre = |d: [f64; 3]| lettre_orientation(d);
        let m = 6.0;
        let ecrire = |pos: Pos2, ancre: egui::Align2, texte: String| {
            peintre.text(pos, ancre, texte, police.clone(), theme::TEXTE_SECONDAIRE);
        };
        ecrire(
            Pos2::new(rect.right() - m, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            lettre(plan.u).into(),
        );
        ecrire(
            Pos2::new(rect.left() + m, rect.center().y),
            egui::Align2::LEFT_CENTER,
            lettre(plan.u.map(|x| -x)).into(),
        );
        ecrire(
            Pos2::new(rect.center().x, rect.top() + m),
            egui::Align2::CENTER_TOP,
            lettre(plan.v).into(),
        );
        ecrire(
            Pos2::new(rect.center().x, rect.bottom() - m),
            egui::Align2::CENTER_BOTTOM,
            lettre(plan.v.map(|x| -x)).into(),
        );
        ecrire(
            rect.left_top() + Vec2::splat(m),
            egui::Align2::LEFT_TOP,
            self.type_vue.nom().to_owned(),
        );
        reponse
    }

    #[allow(clippy::too_many_arguments)]
    fn interactions(
        &mut self,
        ui: &Ui,
        reponse: &Response,
        rect: Rect,
        plan: &Plan,
        centre_plan: (f64, f64),
        couches: &mut [Couche],
        curseur: &mut PointMonde,
        reference: &Volume,
    ) {
        let zoom = self.zoom.unwrap_or(1.0);
        let vers_plan = |p: Pos2| {
            let d = p - rect.center();
            (centre_plan.0 + (d.x / zoom) as f64, centre_plan.1 - (d.y / zoom) as f64)
        };
        // Clic ou glissé du bouton principal : place le curseur.
        if (reponse.clicked() || reponse.dragged_by(egui::PointerButton::Primary))
            && let Some(p) = reponse.interact_pointer_pos()
        {
            let (a, b) = vers_plan(p);
            curseur.0 = plan.vers_monde(a, b);
        }
        // Glissé du bouton du milieu : déplacement de la vue.
        if reponse.dragged_by(egui::PointerButton::Middle) {
            let d = reponse.drag_delta();
            let (a, b) = (centre_plan.0 - (d.x / zoom) as f64, centre_plan.1 + (d.y / zoom) as f64);
            self.centre = Some(plan.vers_monde(a, b));
        }
        // Glissé du bouton droit : fenêtrage de la première image (horizontal : largeur, vertical : niveau).
        if reponse.dragged_by(egui::PointerButton::Secondary)
            && let Some(couche) = couches
                .iter_mut()
                .find(|c| c.visible && matches!(c.rendu, Rendu::Image { .. }))
        {
            let d = reponse.drag_delta();
            let auto = couche.volume.fenetre_auto();
            if let Rendu::Image { fenetre } = &mut couche.rendu {
                let (niveau, largeur) = fenetre.unwrap_or(auto);
                *fenetre = Some((
                    niveau - d.y * largeur * 0.005,
                    (largeur * (d.x * 0.005).exp()).max(1e-6),
                ));
            }
        }
        if reponse.double_clicked() {
            self.recadrer();
        }
        if !reponse.hovered() {
            return;
        }
        let (molette, zoom_delta, souris) =
            ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
        if zoom_delta != 1.0 {
            // Ctrl/Cmd + molette ou pincement : zoom autour de la souris.
            let nouveau = (zoom * zoom_delta).clamp(0.05, 200.0);
            if let Some(s) = souris {
                let (a, b) = vers_plan(s);
                let d = s - rect.center();
                let centre = (a - (d.x / nouveau) as f64, b + (d.y / nouveau) as f64);
                self.centre = Some(plan.vers_monde(centre.0, centre.1));
            }
            self.zoom = Some(nouveau);
        } else if molette != 0.0 {
            // Molette : coupe suivante ou précédente, d'un voxel de la référence le long de la normale.
            self.defilement += molette;
            let n = plan.normale();
            let pas = reference.taille_voxel().into_iter().fold(f64::MAX, f64::min);
            while self.defilement.abs() >= SEUIL_MOLETTE {
                let sens = self.defilement.signum();
                self.defilement -= sens * SEUIL_MOLETTE;
                for (x, nc) in curseur.0.iter_mut().zip(n) {
                    *x += sens as f64 * pas * nc;
                }
            }
        }
    }
}

/// Plan de la vue : celui du curseur, mais d'origine le centre de la vue ramené à la profondeur du curseur. Déplacer
/// le curseur dans le plan ne change alors pas le plan, et l'image n'est pas recomposée.
fn ancrer(plan: Plan, centre: [f64; 3], curseur: [f64; 3]) -> Plan {
    let n = plan.normale();
    let profondeur: f64 = (0..3).map(|c| (curseur[c] - centre[c]) * n[c]).sum();
    Plan {
        origine: std::array::from_fn(|c| centre[c] + profondeur * n[c]),
        ..plan
    }
}

/// Défilement (points) pour passer d'une coupe à la suivante.
const SEUIL_MOLETTE: f32 = 20.0;

/// Lettre de l'axe monde dominant d'une direction (RAS).
fn lettre_orientation(d: [f64; 3]) -> &'static str {
    let c = (0..3).max_by(|&a, &b| d[a].abs().total_cmp(&d[b].abs())).unwrap_or(0);
    match (c, d[c] >= 0.0) {
        (0, true) => "R",
        (0, false) => "L",
        (1, true) => "A",
        (1, false) => "P",
        (2, true) => "S",
        _ => "I",
    }
}

/// Compose les couches visibles en une image RVBA : `taille` pixels de `pas` mm, centrés en `centre` (coordonnées
/// dans le plan). C'est l'image affichée par [`VueCoupe`] ; publique pour le rendu hors écran.
pub fn composer(plan: &Plan, centre: (f64, f64), taille: [usize; 2], pas: f64, couches: &[Couche]) -> ColorImage {
    let [l, h] = taille;
    let fond = theme::FOND;
    let mut pixels = vec![fond; l * h];
    for couche in couches.iter().filter(|c| c.visible) {
        let interpolation = match couche.rendu {
            Rendu::Image { .. } => Interpolation::Trilineaire,
            _ => Interpolation::PlusProche,
        };
        let coupe = plan.echantillonner(&couche.volume, centre, l, h, pas, interpolation);
        match &couche.rendu {
            Rendu::Image { fenetre } => {
                let (niveau, largeur) = fenetre.unwrap_or_else(|| couche.volume.fenetre_auto());
                let bas = niveau - largeur / 2.0;
                for (p, v) in pixels.iter_mut().zip(&coupe.valeurs) {
                    if v.is_finite() {
                        let g = (((v - bas) / largeur).clamp(0.0, 1.0) * 255.0) as u8;
                        *p = Color32::from_gray(g);
                    }
                }
            }
            Rendu::Etiquettes { table, opacite } => {
                for (p, v) in pixels.iter_mut().zip(&coupe.valeurs) {
                    let index = v.round();
                    if v.is_finite() && index >= 1.0 {
                        let [r, g, b] = table.couleur(index as u32);
                        *p = melanger(*p, Color32::from_rgb(r, g, b), *opacite);
                    }
                }
            }
            Rendu::Contour { couleur } => {
                let dedans = |i: usize| coupe.valeurs[i] > 0.5;
                let [r, g, b] = *couleur;
                for y in 0..h {
                    for x in 0..l {
                        let i = y * l + x;
                        let bord = dedans(i)
                            && (x == 0
                                || y == 0
                                || x + 1 == l
                                || y + 1 == h
                                || !dedans(i - 1)
                                || !dedans(i + 1)
                                || !dedans(i - l)
                                || !dedans(i + l));
                        if bord {
                            pixels[i] = Color32::from_rgb(r, g, b);
                        }
                    }
                }
            }
        }
    }
    ColorImage::new([l, h], pixels)
}

/// Vignette de la coupe `k` d'une pile, dans son plan natif : `cote` pixels sur le plus grand côté, fenêtrage
/// automatique de la pile.
pub fn vignette(volume: &Arc<Volume>, k: usize, cote: usize) -> ColorImage {
    let [nx, ny, _] = volume.dims();
    let centre = volume.vers_monde([(nx as f64 - 1.0) / 2.0, (ny as f64 - 1.0) / 2.0, k as f64]);
    let plan = Plan::natif(volume.affine(), centre);
    let [ti, tj, _] = volume.taille_voxel();
    let (lu, lv) = (nx as f64 * ti, ny as f64 * tj);
    let pas = lu.max(lv) / cote as f64;
    let taille = [
        ((lu / pas).round() as usize).max(1),
        ((lv / pas).round() as usize).max(1),
    ];
    composer(&plan, (0.0, 0.0), taille, pas, &[Couche::image(volume.clone())])
}

fn melanger(fond: Color32, dessus: Color32, alpha: f32) -> Color32 {
    let m = |a: u8, b: u8| (a as f32 * (1.0 - alpha) + b as f32 * alpha).round() as u8;
    Color32::from_rgb(
        m(fond.r(), dessus.r()),
        m(fond.g(), dessus.g()),
        m(fond.b(), dessus.b()),
    )
}

/// Trois vues alignées sur les axes, curseur partagé : 1 × 3, ou 2 × 2 avec une quatrième case libre.
pub struct Triplanaire {
    pub vues: [VueCoupe; 3],
}

impl Default for Triplanaire {
    fn default() -> Self {
        Triplanaire {
            vues: Orientation::TOUTES.map(|o| VueCoupe::new(TypeVue::Alignee(o))),
        }
    }
}

impl Triplanaire {
    pub fn recadrer(&mut self) {
        self.vues.iter_mut().for_each(VueCoupe::recadrer);
    }

    /// Affiche les trois vues dans l'espace disponible ; `quatrieme` dessine la case libre de la grille 2 × 2.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        couches: &mut [Couche],
        surfaces: &[Surface],
        curseur: &mut PointMonde,
        quatrieme: impl FnOnce(&mut Ui, Rect),
    ) {
        let zone = ui.available_rect_before_wrap();
        let ecart = 2.0;
        let (l, h) = ((zone.width() - ecart) / 2.0, (zone.height() - ecart) / 2.0);
        let cases: [Rect; 4] = std::array::from_fn(|n| {
            let (x, y) = ((n % 2) as f32, (n / 2) as f32);
            Rect::from_min_size(zone.min + Vec2::new(x * (l + ecart), y * (h + ecart)), Vec2::new(l, h))
        });
        for (vue, case) in self.vues.iter_mut().zip(cases) {
            vue.ui(ui, case, couches, surfaces, curseur);
        }
        quatrieme(ui, cases[3]);
        ui.advance_cursor_after_rect(zone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_ancre_insensible_aux_deplacements_dans_le_plan() {
        let centre = [10.0, 20.0, 30.0];
        let p1 = ancrer(
            Plan::aligne(Orientation::Axiale, [0.0, 0.0, 5.0]),
            centre,
            [0.0, 0.0, 5.0],
        );
        let p2 = ancrer(
            Plan::aligne(Orientation::Axiale, [7.0, -3.0, 5.0]),
            centre,
            [7.0, -3.0, 5.0],
        );
        assert_eq!(p1, p2, "même profondeur : même plan");
        assert_eq!(p1.origine, [10.0, 20.0, 5.0]);
        let p3 = ancrer(
            Plan::aligne(Orientation::Axiale, [0.0, 0.0, 6.0]),
            centre,
            [0.0, 0.0, 6.0],
        );
        assert_ne!(p1, p3);
    }

    #[test]
    fn lettres_d_orientation() {
        assert_eq!(lettre_orientation([-1.0, 0.0, 0.0]), "L");
        assert_eq!(lettre_orientation([0.2, 0.9, -0.1]), "A");
        assert_eq!(lettre_orientation([0.0, 0.3, -0.95]), "I");
    }

    #[test]
    fn composition_etiquettes_et_contour() {
        // Volume 5 × 5 × 1, étiquette 1 au centre (3 × 3), coupe axiale au pas du voxel.
        let mut voxels = vec![0.0; 25];
        for j in 1..4 {
            for i in 1..4 {
                voxels[i + 5 * j] = 1.0;
            }
        }
        let vol = Arc::new(Volume::new([5, 5, 1], Affine::IDENTITE, voxels).unwrap());
        let plan = Plan::aligne(Orientation::Axiale, [2.0, 2.0, 0.0]);
        let table = TableEtiquettes::default();
        let couches = [
            Couche {
                volume: vol.clone(),
                rendu: Rendu::Etiquettes {
                    table: table.clone(),
                    opacite: 1.0,
                },
                visible: true,
            },
            Couche {
                volume: vol.clone(),
                rendu: Rendu::Contour { couleur: [255, 0, 0] },
                visible: true,
            },
        ];
        let image = composer(&plan, (0.0, 0.0), [5, 5], 1.0, &couches);
        let rouge = Color32::from_rgb(255, 0, 0);
        let [r, g, b] = table.couleur(1);
        assert_eq!(
            image.pixels[12],
            Color32::from_rgb(r, g, b),
            "centre : étiquette, pas de contour"
        );
        assert_eq!(image.pixels[6], rouge, "bord du carré : contour");
        assert_eq!(image.pixels[0], theme::FOND, "fond");
    }
}
