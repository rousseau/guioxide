//! Vue 3D des maillages : rendu wgpu dans une texture propre (couleur et profondeur, anticrénelage 4×), affichée
//! comme une image egui. Caméra orbitale autour d'une cible ; clic sur la surface → curseur monde.
//!
//! Le rendu n'est refait que si la caméra, la taille ou les objets changent.

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use egui_wgpu::{RenderState, wgpu};
use guioxide_data::Maillage;
use guioxide_geom::PointMonde;
use wgpu::util::DeviceExt;

use crate::theme;

/// Champ de vision vertical.
const CHAMP: f32 = 30.0 * std::f32::consts::PI / 180.0;
const ECHANTILLONS: u32 = 4;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const PROFONDEUR: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Un maillage affiché, avec une couleur par sommet, ou une seule pour tout le maillage.
#[derive(Clone)]
pub struct Objet3d {
    pub maillage: Arc<Maillage>,
    pub couleurs: Arc<Vec<[u8; 3]>>,
    pub visible: bool,
}

/// Points de vue prédéfinis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VuePredefinie {
    Laterale,
    Mediale,
    Dorsale,
}

/// Caméra orbitale : la cible, et l'œil sur une sphère autour d'elle (z vers le haut, S).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub cible: [f32; 3],
    /// Angle dans le plan axial, depuis +x (radians).
    pub azimut: f32,
    /// Hauteur au-dessus du plan axial (radians).
    pub elevation: f32,
    pub distance: f32,
}

impl Camera {
    fn oeil(&self) -> [f32; 3] {
        let (ce, se) = (self.elevation.cos(), self.elevation.sin());
        let d = [ce * self.azimut.cos(), ce * self.azimut.sin(), se];
        std::array::from_fn(|c| self.cible[c] + self.distance * d[c])
    }

    /// Repère de la caméra : avant, droite, haut.
    fn axes(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let o = self.oeil();
        let avant = normaliser(std::array::from_fn(|c| self.cible[c] - o[c]));
        let droite = normaliser(vectoriel(avant, [0.0, 0.0, 1.0]));
        (avant, droite, vectoriel(droite, avant))
    }

    /// Place l'œil pour un point de vue prédéfini ; `gauche` : hémisphère gauche (sa face latérale regarde −x).
    pub fn regarder(&mut self, vue: VuePredefinie, gauche: bool) {
        let pi = std::f32::consts::PI;
        (self.azimut, self.elevation) = match (vue, gauche) {
            (VuePredefinie::Laterale, true) | (VuePredefinie::Mediale, false) => (pi, 0.0),
            (VuePredefinie::Laterale, false) | (VuePredefinie::Mediale, true) => (0.0, 0.0),
            (VuePredefinie::Dorsale, _) => (-pi / 2.0, ELEVATION_MAX),
        };
    }

    /// Projection et vue, colonne par colonne (convention WGSL), profondeur dans [0, 1].
    fn matrice(&self, aspect: f32) -> [[f32; 4]; 4] {
        let o = self.oeil();
        let (f, s, u) = self.axes();
        let point = |a: [f32; 3]| a[0] * o[0] + a[1] * o[1] + a[2] * o[2];
        let vue = [
            [s[0], u[0], -f[0], 0.0],
            [s[1], u[1], -f[1], 0.0],
            [s[2], u[2], -f[2], 0.0],
            [-point(s), -point(u), point(f), 1.0],
        ];
        let (proche, loin) = (self.distance * 0.01, self.distance * 10.0);
        let k = 1.0 / (CHAMP / 2.0).tan();
        let proj = [
            [k / aspect, 0.0, 0.0, 0.0],
            [0.0, k, 0.0, 0.0],
            [0.0, 0.0, loin / (proche - loin), -1.0],
            [0.0, 0.0, proche * loin / (proche - loin), 0.0],
        ];
        std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| proj[k][r] * vue[c][k]).sum()))
    }

    /// Rayon (origine, direction) passant par le point `(x, y)` de l'image, normalisé dans [−1, 1] (y vers le haut).
    fn rayon(&self, x: f32, y: f32, aspect: f32) -> ([f32; 3], [f32; 3]) {
        let (f, s, u) = self.axes();
        let t = (CHAMP / 2.0).tan();
        let d = normaliser(std::array::from_fn(|c| f[c] + x * t * aspect * s[c] + y * t * u[c]));
        (self.oeil(), d)
    }

    /// Projection d'un point du monde dans l'image, normalisée dans [−1, 1] ; `None` derrière la caméra.
    fn projeter(&self, p: [f32; 3], aspect: f32) -> Option<(f32, f32)> {
        let m = self.matrice(aspect);
        let c: [f32; 4] = std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r]);
        (c[3] > 0.0).then(|| (c[0] / c[3], c[1] / c[3]))
    }
}

const ELEVATION_MAX: f32 = 1.55;

/// Ressources GPU d'un maillage.
struct MaillageGpu {
    cle: (usize, usize),
    sommets: wgpu::Buffer,
    indices: wgpu::Buffer,
    nombre: u32,
}

/// Cible de rendu : couleur multi-échantillonnée, résolue dans une texture enregistrée auprès d'egui.
struct Cible {
    taille: [u32; 2],
    couleur: wgpu::TextureView,
    resolue: wgpu::Texture,
    vue_resolue: wgpu::TextureView,
    profondeur: wgpu::TextureView,
}

/// Rendu hors écran des maillages : n'a besoin que d'un `Device` et d'une `Queue` (utilisable sans fenêtre).
pub struct Rendu3d {
    gpu: Gpu,
}

impl Rendu3d {
    pub fn new(device: &wgpu::Device) -> Rendu3d {
        Rendu3d { gpu: Gpu::new(device) }
    }

    /// Rend les objets visibles dans une texture `taille` (pixels, format RVBA sRGB) et la renvoie.
    pub fn rendre(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        taille: [u32; 2],
        camera: &Camera,
        objets: &[Objet3d],
    ) -> &wgpu::Texture {
        let visibles: Vec<&Objet3d> = objets.iter().filter(|o| o.visible).collect();
        self.gpu.preparer_cible(device, taille);
        self.gpu.preparer_maillages(device, &visibles);
        self.gpu
            .rendre(device, queue, camera, &visibles, taille[0] as f32 / taille[1] as f32);
        &self.gpu.cible.as_ref().unwrap().resolue
    }

    /// Caméra qui fait tenir les objets visibles, vue latérale gauche.
    /// `aspect` : largeur / hauteur de l'image.
    pub fn cadrer(objets: &[Objet3d], aspect: f32) -> Camera {
        cadrer(&objets.iter().filter(|o| o.visible).collect::<Vec<_>>(), aspect)
    }
}

struct Gpu {
    pipeline: wgpu::RenderPipeline,
    uniformes: wgpu::Buffer,
    groupe: wgpu::BindGroup,
    maillages: Vec<MaillageGpu>,
    cible: Option<Cible>,
}

#[derive(Clone, PartialEq)]
struct CleRendu {
    camera: Camera,
    taille: [u32; 2],
    objets: Vec<(usize, usize, bool)>,
}

#[derive(Default)]
pub struct Vue3d {
    pub camera: Option<Camera>,
    gpu: Option<Gpu>,
    cle: Option<CleRendu>,
    texture: Option<egui::TextureId>,
}

impl Vue3d {
    /// Recadre sur les objets visibles au prochain affichage.
    pub fn recadrer(&mut self) {
        self.camera = None;
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        rs: &RenderState,
        objets: &[Objet3d],
        curseur: &mut PointMonde,
    ) -> Response {
        let reponse = ui.allocate_rect(rect, Sense::click_and_drag());
        let visibles: Vec<&Objet3d> = objets
            .iter()
            .filter(|o| o.visible && !o.maillage.faces.is_empty())
            .collect();
        if visibles.is_empty() {
            ui.painter().rect_filled(rect, 0.0, theme::FOND);
            return reponse;
        }
        let aspect = rect.width() / rect.height().max(1.0);
        let camera = self.camera.get_or_insert_with(|| cadrer(&visibles, aspect));
        interactions(ui, &reponse, rect, camera, &visibles, curseur);
        let camera = *camera;

        let ppp = ui.ctx().pixels_per_point();
        let taille = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        let gpu = self.gpu.get_or_insert_with(|| Gpu::new(&rs.device));
        let ancienne = gpu.cible.as_ref().map(|c| c.taille);
        let cle = CleRendu {
            camera,
            taille,
            objets: objets
                .iter()
                .map(|o| {
                    (
                        Arc::as_ptr(&o.maillage) as usize,
                        Arc::as_ptr(&o.couleurs) as usize,
                        o.visible,
                    )
                })
                .collect(),
        };
        if self.cle.as_ref() != Some(&cle) || gpu.cible.as_ref().is_none_or(|c| c.taille != taille) {
            gpu.preparer_cible(&rs.device, taille);
            gpu.preparer_maillages(&rs.device, &visibles);
            gpu.rendre(&rs.device, &rs.queue, &camera, &visibles, aspect);
            self.cle = Some(cle);
        }
        // La texture résolue est enregistrée auprès d'egui ; remplacée quand la taille change.
        if let Some(cible) = &gpu.cible {
            let mut rendu = rs.renderer.write();
            let filtre = wgpu::FilterMode::Linear;
            match self.texture {
                Some(_) if ancienne == Some(cible.taille) => {}
                Some(id) => rendu.update_egui_texture_from_wgpu_texture(&rs.device, &cible.vue_resolue, filtre, id),
                None => self.texture = Some(rendu.register_native_texture(&rs.device, &cible.vue_resolue, filtre)),
            }
        }
        if let Some(id) = self.texture {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter().image(id, rect, uv, Color32::WHITE);
        }
        // Marqueur du curseur monde, projeté (sans tenir compte de ce qui le cache).
        let p = curseur.0.map(|v| v as f32);
        if let Some((x, y)) = camera.projeter(p, aspect) {
            let ecran = rect.center() + Vec2::new(x, -y) * rect.size() / 2.0;
            if rect.contains(ecran) {
                ui.painter().circle_stroke(ecran, 5.0, Stroke::new(2.0, theme::ACCENT));
            }
        }
        reponse
    }
}

/// Plus petit demi-angle de champ (vertical ou horizontal) : la sphère englobante doit tenir dans les deux sens.
fn demi_champ_minimal(aspect: f32) -> f32 {
    let vertical = CHAMP / 2.0;
    let horizontal = ((CHAMP / 2.0).tan() * aspect).atan();
    vertical.min(horizontal)
}

/// Caméra qui fait tenir la boîte des objets, vue latérale gauche.
fn cadrer(objets: &[&Objet3d], aspect: f32) -> Camera {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for o in objets {
        let (a, b) = o.maillage.boite();
        for c in 0..3 {
            min[c] = min[c].min(a[c]);
            max[c] = max[c].max(b[c]);
        }
    }
    let cible = std::array::from_fn(|c| (min[c] + max[c]) / 2.0);
    let rayon = norme(std::array::from_fn(|c| max[c] - min[c])) / 2.0;
    let mut camera = Camera {
        cible,
        azimut: 0.0,
        elevation: 0.0,
        distance: rayon.max(1.0) / demi_champ_minimal(aspect).sin(),
    };
    camera.regarder(VuePredefinie::Laterale, true);
    camera
}

/// Glisser : rotation ; Maj + glisser ou bouton du milieu : translation ; molette : distance ; clic : curseur.
fn interactions(
    ui: &Ui,
    reponse: &Response,
    rect: Rect,
    camera: &mut Camera,
    objets: &[&Objet3d],
    curseur: &mut PointMonde,
) {
    let d = reponse.drag_delta();
    let translation = ui.input(|i| i.modifiers.shift) || reponse.dragged_by(egui::PointerButton::Middle);
    if reponse.dragged() && translation {
        let (_, s, u) = camera.axes();
        let k = camera.distance * (CHAMP / 2.0).tan() * 2.0 / rect.height();
        for c in 0..3 {
            camera.cible[c] += (-d.x * s[c] + d.y * u[c]) * k;
        }
    } else if reponse.dragged_by(egui::PointerButton::Primary) {
        camera.azimut -= d.x * 0.01;
        camera.elevation = (camera.elevation + d.y * 0.01).clamp(-ELEVATION_MAX, ELEVATION_MAX);
    }
    if reponse.hovered() {
        let (molette, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        camera.distance = (camera.distance * (-molette * 0.002).exp() / zoom).max(1.0);
    }
    if reponse.clicked()
        && let Some(p) = reponse.interact_pointer_pos()
    {
        let x = (p.x - rect.center().x) / (rect.width() / 2.0);
        let y = -(p.y - rect.center().y) / (rect.height() / 2.0);
        let (origine, direction) = camera.rayon(x, y, rect.width() / rect.height());
        if let Some(sommet) = premier_sommet_touche(objets, origine, direction) {
            curseur.0 = sommet.map(|v| v as f64);
        }
    }
}

/// Sommet le plus proche du premier point d'impact du rayon sur les maillages (Möller–Trumbore).
fn premier_sommet_touche(objets: &[&Objet3d], o: [f32; 3], d: [f32; 3]) -> Option<[f32; 3]> {
    let mut meilleur: Option<(f32, [f32; 3])> = None;
    for objet in objets {
        let m = &objet.maillage;
        for f in &m.faces {
            let [a, b, c] = f.map(|i| m.sommets[i as usize]);
            let e1 = difference(b, a);
            let e2 = difference(c, a);
            let p = vectoriel(d, e2);
            let det = scalaire(e1, p);
            if det.abs() < 1e-12 {
                continue;
            }
            let t0 = difference(o, a);
            let u = scalaire(t0, p) / det;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = vectoriel(t0, e1);
            let v = scalaire(d, q) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let t = scalaire(e2, q) / det;
            if t > 0.0 && meilleur.is_none_or(|(tm, _)| t < tm) {
                let impact: [f32; 3] = std::array::from_fn(|k| o[k] + t * d[k]);
                let proche = [a, b, c]
                    .into_iter()
                    .min_by(|x, y| norme(difference(*x, impact)).total_cmp(&norme(difference(*y, impact))))
                    .unwrap();
                meilleur = Some((t, proche));
            }
        }
    }
    meilleur.map(|(_, s)| s)
}

/// Normales aux sommets : somme des normales des faces voisines, pondérées par leur aire.
pub fn normales(m: &Maillage) -> Vec<[f32; 3]> {
    let mut n = vec![[0.0f32; 3]; m.sommets.len()];
    for f in &m.faces {
        let [a, b, c] = f.map(|i| m.sommets[i as usize]);
        let nf = vectoriel(difference(b, a), difference(c, a));
        for i in f {
            for k in 0..3 {
                n[*i as usize][k] += nf[k];
            }
        }
    }
    n.into_iter().map(normaliser).collect()
}

impl Gpu {
    fn new(device: &wgpu::Device) -> Gpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vue3d"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let disposition = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vue3d"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniformes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vue3d"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vue3d"),
            layout: &disposition,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniformes.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vue3d"),
            bind_group_layouts: &[Some(&disposition)],
            immediate_size: 0,
        });
        let attributs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vue3d"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: TAILLE_SOMMET as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributs,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: PROFONDEUR,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: ECHANTILLONS,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        Gpu {
            pipeline,
            uniformes,
            groupe,
            maillages: Vec::new(),
            cible: None,
        }
    }

    fn preparer_cible(&mut self, device: &wgpu::Device, taille: [u32; 2]) {
        if self.cible.as_ref().is_some_and(|c| c.taille == taille) {
            return;
        }
        let texture = |format, echantillons, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vue3d"),
                size: wgpu::Extent3d {
                    width: taille[0],
                    height: taille[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: echantillons,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let attache = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let usages = attache | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC;
        let couleur = texture(FORMAT, ECHANTILLONS, attache).create_view(&Default::default());
        let resolue = texture(FORMAT, 1, usages);
        let vue_resolue = resolue.create_view(&Default::default());
        let profondeur = texture(PROFONDEUR, ECHANTILLONS, attache).create_view(&Default::default());
        self.cible = Some(Cible {
            taille,
            couleur,
            resolue,
            vue_resolue,
            profondeur,
        });
    }

    fn preparer_maillages(&mut self, device: &wgpu::Device, objets: &[&Objet3d]) {
        let cles: Vec<(usize, usize)> = objets
            .iter()
            .map(|o| (Arc::as_ptr(&o.maillage) as usize, Arc::as_ptr(&o.couleurs) as usize))
            .collect();
        self.maillages.retain(|m| cles.contains(&m.cle));
        for (o, cle) in objets.iter().zip(cles) {
            if self.maillages.iter().any(|m| m.cle == cle) {
                continue;
            }
            let m = &o.maillage;
            let n = normales(m);
            let mut octets = Vec::with_capacity(m.sommets.len() * TAILLE_SOMMET);
            for (i, (p, n)) in m.sommets.iter().zip(&n).enumerate() {
                let c = o
                    .couleurs
                    .get(i)
                    .or(o.couleurs.first())
                    .copied()
                    .unwrap_or([200, 200, 200]);
                p.iter()
                    .chain(n)
                    .for_each(|v| octets.extend_from_slice(&v.to_le_bytes()));
                octets.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
            let indices: Vec<u8> = m.faces.iter().flatten().flat_map(|i| i.to_le_bytes()).collect();
            let creer = |contenu: &[u8], usage| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vue3d"),
                    contents: contenu,
                    usage,
                })
            };
            self.maillages.push(MaillageGpu {
                cle,
                sommets: creer(&octets, wgpu::BufferUsages::VERTEX),
                indices: creer(&indices, wgpu::BufferUsages::INDEX),
                nombre: (m.faces.len() * 3) as u32,
            });
        }
    }

    fn rendre(&self, device: &wgpu::Device, queue: &wgpu::Queue, camera: &Camera, objets: &[&Objet3d], aspect: f32) {
        let Some(cible) = &self.cible else { return };
        let matrice = camera.matrice(aspect);
        let (avant, _, _) = camera.axes();
        let mut uniformes: Vec<u8> = matrice.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        for v in [-avant[0], -avant[1], -avant[2], 0.0] {
            uniformes.extend_from_slice(&v.to_le_bytes());
        }
        queue.write_buffer(&self.uniformes, 0, &uniformes);
        let lineaire = |c: u8| (c as f64 / 255.0).powf(2.2);
        let fond = theme::FOND;
        let mut encodeur = device.create_command_encoder(&Default::default());
        {
            let mut passe = encodeur.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vue3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cible.couleur,
                    depth_slice: None,
                    resolve_target: Some(&cible.vue_resolue),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: lineaire(fond.r()),
                            g: lineaire(fond.g()),
                            b: lineaire(fond.b()),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &cible.profondeur,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            passe.set_pipeline(&self.pipeline);
            passe.set_bind_group(0, &self.groupe, &[]);
            for o in objets {
                let cle = (Arc::as_ptr(&o.maillage) as usize, Arc::as_ptr(&o.couleurs) as usize);
                if let Some(m) = self.maillages.iter().find(|m| m.cle == cle) {
                    passe.set_vertex_buffer(0, m.sommets.slice(..));
                    passe.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                    passe.draw_indexed(0..m.nombre, 0, 0..1);
                }
            }
        }
        queue.submit([encodeur.finish()]);
    }
}

/// Position (3 × f32), normale (3 × f32), couleur (4 × u8).
const TAILLE_SOMMET: usize = 28;

const SHADER: &str = r#"
struct Uniformes { vp: mat4x4<f32>, lumiere: vec4<f32> };
@group(0) @binding(0) var<uniform> u: Uniformes;

struct Entree { @location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec4<f32> };
struct Sortie { @builtin(position) pos: vec4<f32>, @location(0) n: vec3<f32>, @location(1) c: vec3<f32> };

@vertex fn vs(e: Entree) -> Sortie {
    var s: Sortie;
    s.pos = u.vp * vec4<f32>(e.p, 1.0);
    s.n = e.n;
    s.c = pow(e.c.rgb, vec3<f32>(2.2));
    return s;
}

// Éclairage frontal (lampe sur la caméra), deux faces, terme ambiant.
@fragment fn fs(s: Sortie) -> @location(0) vec4<f32> {
    let d = abs(dot(normalize(s.n), u.lumiere.xyz));
    return vec4<f32>(s.c * (0.2 + 0.8 * d), 1.0);
}
"#;

fn difference(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scalaire(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vectoriel(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norme(a: [f32; 3]) -> f32 {
    scalaire(a, a).sqrt()
}

fn normaliser(a: [f32; 3]) -> [f32; 3] {
    let n = norme(a);
    if n > 0.0 { a.map(|x| x / n) } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetraedre() -> Maillage {
        Maillage {
            sommets: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            faces: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            ..Default::default()
        }
    }

    #[test]
    fn camera_projette_la_cible_au_centre_et_le_rayon_y_revient() {
        let mut c = Camera {
            cible: [1.0, 2.0, 3.0],
            azimut: 0.3,
            elevation: 0.2,
            distance: 50.0,
        };
        let (x, y) = c.projeter(c.cible, 1.5).unwrap();
        assert!(x.abs() < 1e-5 && y.abs() < 1e-5);
        // Un point à droite de la cible (axe « droite » de la caméra) est projeté à droite du centre.
        let (_, s, u) = c.axes();
        let droite: [f32; 3] = std::array::from_fn(|k| c.cible[k] + 5.0 * s[k] + 2.0 * u[k]);
        let (x, y) = c.projeter(droite, 1.5).unwrap();
        assert!(x > 0.0 && y > 0.0);
        // Le rayon passant par ce point de l'image retrouve sa direction.
        let (o, d) = c.rayon(x, y, 1.5);
        let vers: [f32; 3] = normaliser(difference(droite, o));
        assert!(scalaire(d, vers) > 0.99999, "{d:?} vs {vers:?}");
        // Vues prédéfinies : la face latérale gauche est vue depuis −x.
        c.regarder(VuePredefinie::Laterale, true);
        assert!(c.oeil()[0] < c.cible[0]);
        c.regarder(VuePredefinie::Dorsale, true);
        assert!(c.oeil()[2] > c.cible[2] + 0.9 * c.distance);
    }

    #[test]
    fn cadrage_selon_le_champ_le_plus_etroit() {
        assert_eq!(
            demi_champ_minimal(2.0),
            CHAMP / 2.0,
            "image large : le champ vertical limite"
        );
        let etroit = demi_champ_minimal(0.5);
        assert!(etroit < CHAMP / 2.0 && (etroit.tan() - (CHAMP / 2.0).tan() * 0.5).abs() < 1e-6);
    }

    #[test]
    fn rayon_touche_le_sommet_le_plus_proche_de_l_impact() {
        let objet = Objet3d {
            maillage: Arc::new(tetraedre()),
            couleurs: Arc::new(vec![[255; 3]]),
            visible: true,
        };
        // Rayon le long de −z, juste à côté du sommet (0, 0, 1) : il touche la face oblique près de ce sommet.
        let s = premier_sommet_touche(&[&objet], [0.05, 0.05, 5.0], [0.0, 0.0, -1.0]);
        assert_eq!(s, Some([0.0, 0.0, 1.0]));
        assert_eq!(
            premier_sommet_touche(&[&objet], [2.0, 2.0, 5.0], [0.0, 0.0, -1.0]),
            None
        );
    }

    #[test]
    fn normales_sortantes_unitaires() {
        let n = normales(&tetraedre());
        for v in &n {
            assert!((norme(*v) - 1.0).abs() < 1e-6);
        }
        // Sommet (1, 0, 0) : normale vers +x en moyenne (faces orientées vers l'extérieur).
        assert!(n[1][0] > 0.0);
        assert!(
            n[0].iter().all(|&c| c < 0.0),
            "le sommet à l'origine regarde vers −x −y −z : {:?}",
            n[0]
        );
    }
}
