//! Banc de fluidité : ce que coûte à l'utilisateur l'ouverture d'un jeu, la première image d'une séance, la recomposition
//! d'une vue de coupe, la rotation de la vue 3D, la mémoire, et deux cas lourds synthétiques. Sans fenêtre ni Garage :
//! les mêmes fonctions que l'interface (`indexer`, `lire_volume`, `composer`, `Rendu3d`), un jeu BIDS donné (le jeu de test
//! fantôme de `scripts/jeu-test/`, ou un jeu rapatrié par `bench/guioxide.sh` dans le dépôt privé oxide).
//!
//! ```text
//! cargo run --release -p guioxide-view --example banc -- <racine BIDS> [--sortie banc.tsv] [--repetitions 15]
//!     [--images-3d 120] [--sans-3d] [--sans-lourds] [--session <texte>]
//! ```
//!
//! Sortie : un TSV `mesure cas valeur unite` (et le même tableau sur `stderr`). Les durées sont des **médianes** de
//! `--repetitions` essais (la première image est mesurée une seule fois, fichiers déjà en cache du système). La rotation 3D
//! attend la fin du travail du GPU après chaque image : elle donne des images par seconde de bout en bout, pas un débit de
//! soumission. Mémoire : pic de la mémoire résidente du processus (`VmHWM`, Linux ; `ps` ailleurs).
//!
//! Cas lourds (synthétiques, en mémoire) : « adulte » = un volume de 0,6 mm isotrope de 300 × 360 × 300 voxels (32,4 millions)
//! avec 1 à 4 couches ; « serie4d » = 64 volumes de 128 × 128 × 72 voxels de 1,5 mm, un par instant, parcourus un à un.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_wgpu::wgpu;
use guioxide_bids::{CheminsSession, indexer};
use guioxide_data::{TableEtiquettes, lire_surface, lire_volume};
use guioxide_geom::{Affine, Orientation, Plan, Volume};
use guioxide_view::coupe::{Couche, Rendu, composer};
use guioxide_view::vue3d::{Objet3d, Rendu3d};

const PAS_MM: f64 = 0.5;

struct Mesures(Vec<(String, String, f64, &'static str)>);

impl Mesures {
    fn ajouter(&mut self, mesure: &str, cas: &str, valeur: f64, unite: &'static str) {
        eprintln!("{mesure:<22} {cas:<28} {valeur:>12.2} {unite}");
        self.0.push((mesure.into(), cas.into(), valeur, unite));
    }
}

fn mediane_ms(repetitions: usize, mut f: impl FnMut()) -> f64 {
    let mut durees: Vec<Duration> = (0..repetitions)
        .map(|_| {
            let debut = Instant::now();
            f();
            debut.elapsed()
        })
        .collect();
    durees.sort();
    durees[durees.len() / 2].as_secs_f64() * 1000.0
}

/// Pic de la mémoire résidente du processus, en Mo.
fn memoire_max_mo() -> Option<f64> {
    if let Ok(statut) = std::fs::read_to_string("/proc/self/status") {
        let ligne = statut.lines().find(|l| l.starts_with("VmHWM:"))?;
        let ko: f64 = ligne.split_whitespace().nth(1)?.parse().ok()?;
        return Some(ko / 1024.0);
    }
    let sortie = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let ko: f64 = String::from_utf8_lossy(&sortie.stdout).trim().parse().ok()?;
    Some(ko / 1024.0)
}

/// Taille en pixels et centre d'une vue de l'orientation donnée, qui cadre la boîte du volume au pas `pas`.
fn cadrage(volume: &Volume, orientation: Orientation, pas: f64) -> (Plan, (f64, f64), [usize; 2]) {
    let centre_monde = volume.centre().0;
    let plan = Plan::aligne(orientation, centre_monde);
    let (min, max) = volume.boite();
    let etendue = |d: [f64; 3]| (0..3).map(|c| (max[c] - min[c]) * d[c].abs()).sum::<f64>();
    let taille = [
        (etendue(plan.u) / pas).ceil() as usize,
        (etendue(plan.v) / pas).ceil() as usize,
    ];
    (plan, plan.vers_plan(centre_monde), taille)
}

fn volume_synthetique(dims: [usize; 3], mm: f64, graine: f32) -> Arc<Volume> {
    let affine = Affine([
        [mm, 0.0, 0.0, -(dims[0] as f64) * mm / 2.0],
        [0.0, mm, 0.0, -(dims[1] as f64) * mm / 2.0],
        [0.0, 0.0, mm, -(dims[2] as f64) * mm / 2.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    let mut voxels = Vec::with_capacity(dims[0] * dims[1] * dims[2]);
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let (x, y, z) = (
                    i as f32 / dims[0] as f32,
                    j as f32 / dims[1] as f32,
                    k as f32 / dims[2] as f32,
                );
                voxels.push(500.0 + 400.0 * ((12.0 * x + graine).sin() * (9.0 * y).cos() + (7.0 * z).sin()));
            }
        }
    }
    Arc::new(Volume::new(dims, affine, voxels).expect("volume synthétique"))
}

/// Couches n°1 à n : image, étiquettes (le même volume seuillé en entiers, comme une segmentation), contour, seconde image.
fn couches_de(image: &Arc<Volume>, etiquettes: &Arc<Volume>, n: usize) -> Vec<Couche> {
    let toutes = [
        Couche::image(image.clone()),
        Couche {
            volume: etiquettes.clone(),
            rendu: Rendu::Etiquettes {
                table: TableEtiquettes::default(),
                opacite: 0.45,
            },
            visible: true,
        },
        Couche {
            volume: etiquettes.clone(),
            rendu: Rendu::Contour {
                couleur: [0xE0, 0x7B, 0x39],
            },
            visible: true,
        },
        Couche {
            volume: image.clone(),
            rendu: Rendu::Image {
                fenetre: Some((500.0, 600.0)),
            },
            visible: true,
        },
    ];
    toutes[..n].to_vec()
}

fn etiquettes_depuis(volume: &Volume) -> Arc<Volume> {
    let voxels = volume
        .voxels()
        .iter()
        .map(|v| (v / 150.0).floor().clamp(0.0, 18.0))
        .collect();
    Arc::new(Volume::new(volume.dims(), *volume.affine(), voxels).expect("étiquettes"))
}

fn recomposition(m: &mut Mesures, cas: &str, image: &Arc<Volume>, etiquettes: &Arc<Volume>, repetitions: usize) {
    for n in 1..=4 {
        let couches = couches_de(image, etiquettes, n);
        for orientation in Orientation::TOUTES {
            let (plan, centre, taille) = cadrage(image, orientation, PAS_MM);
            let ms = mediane_ms(repetitions, || {
                std::hint::black_box(composer(&plan, centre, taille, PAS_MM, &couches));
            });
            m.ajouter(
                "recomposition",
                &format!("{cas} {} {n} couche(s)", orientation.nom()),
                ms,
                "ms",
            );
        }
    }
}

fn rotation_3d(m: &mut Mesures, objets: &[Objet3d], images: usize) -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adaptateur = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let info = adaptateur.get_info();
    eprintln!("GPU : {} ({:?})", info.name, info.backend);
    let (device, queue) = pollster::block_on(adaptateur.request_device(&Default::default()))?;
    let taille = [1200u32, 800];
    let mut camera = Rendu3d::cadrer(objets, taille[0] as f32 / taille[1] as f32);
    let mut rendu = Rendu3d::new(&device);
    let debut = Instant::now();
    rendu.rendre(&device, &queue, taille, &camera, objets);
    device.poll(wgpu::PollType::wait_indefinitely())?;
    m.ajouter(
        "rendu3d_premiere_image",
        &info.name,
        debut.elapsed().as_secs_f64() * 1000.0,
        "ms",
    );
    let debut = Instant::now();
    for k in 0..images {
        camera.azimut += std::f32::consts::TAU / images as f32;
        rendu.rendre(&device, &queue, taille, &camera, objets);
        device.poll(wgpu::PollType::wait_indefinitely())?;
        std::hint::black_box(k);
    }
    m.ajouter(
        "rotation_3d",
        &format!("{} {}x{}", info.name, taille[0], taille[1]),
        images as f64 / debut.elapsed().as_secs_f64(),
        "images/s",
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |nom: &str| {
        args.iter()
            .position(|a| a == nom)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let drapeau = |nom: &str| args.iter().any(|a| a == nom);
    let racine = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage : banc <racine BIDS> [options]")?;
    let repetitions: usize = option("--repetitions").map(|v| v.parse()).transpose()?.unwrap_or(15);
    let images_3d: usize = option("--images-3d").map(|v| v.parse()).transpose()?.unwrap_or(120);
    let mut m = Mesures(Vec::new());
    let racine = std::path::Path::new(racine);

    // 1. Ouverture du jeu (indexation).
    let jeu = indexer(racine)?;
    let ms = mediane_ms(repetitions, || {
        std::hint::black_box(indexer(racine).expect("indexation"));
    });
    m.ajouter(
        "ouverture_jeu",
        &format!(
            "{} session(s)",
            jeu.sujets.iter().map(|s| s.sessions.len()).sum::<usize>()
        ),
        ms,
        "ms",
    );

    // 2. Première image d'une séance : lecture du volume reconstruit et de sa segmentation, première coupe composée.
    let texte = option("--session");
    let (sujet, session) = jeu
        .sujets
        .iter()
        .flat_map(|s| s.sessions.iter().map(move |se| (s, se)))
        .find(|(s, se)| {
            let libelle = format!("{} / {}", s.id, se.id.clone().unwrap_or_default());
            texte.as_ref().is_none_or(|t| libelle.contains(t))
        })
        .ok_or("aucune session")?;
    let chemins = CheminsSession::new(&jeu.racine_etude.join("derivatives"), sujet, session);
    let debut = Instant::now();
    // Séance sans dérivées (jeu brut) : la première pile T2w tient lieu d'image et la segmentation est synthétisée
    // (seuils de l'image) ; le tableau le dit dans le nom du cas.
    let (image, etiquettes, origine) = if chemins.svr().is_file() && chemins.segmentation().is_file() {
        (
            lire_volume(&chemins.svr())?.volume,
            lire_volume(&chemins.segmentation())?.volume,
            "svr",
        )
    } else {
        let pile = session
            .piles
            .iter()
            .find(|p| p.contraste == "T2w")
            .ok_or("séance sans pile T2w")?;
        let image = lire_volume(&pile.chemin)?.volume;
        let etiquettes = etiquettes_depuis(&image);
        (image, etiquettes, "pile")
    };
    let (plan, centre, taille) = cadrage(&image, Orientation::Axiale, PAS_MM);
    let couches = couches_de(&image, &etiquettes, 2);
    std::hint::black_box(composer(&plan, centre, taille, PAS_MM, &couches));
    m.ajouter(
        "premiere_image",
        &format!("{origine} {} voxels", image.dims().iter().product::<usize>()),
        debut.elapsed().as_secs_f64() * 1000.0,
        "ms",
    );

    // 3. Recomposition d'une vue à 0,5 mm, 1 à 4 couches.
    recomposition(&mut m, &format!("seance-{origine}"), &image, &etiquettes, repetitions);

    // 4. Rotation de la vue 3D (surfaces piales des deux hémisphères de la séance).
    if !drapeau("--sans-3d") {
        let objets: Vec<Objet3d> = ["L", "R"]
            .iter()
            .filter_map(|h| lire_surface(&chemins.surface("pial", Some(h))).ok())
            .map(|maillage| Objet3d {
                maillage: Arc::new(maillage),
                couleurs: Arc::new(vec![[0xC8, 0xC2, 0xB8]]),
                visible: true,
            })
            .collect();
        if objets.is_empty() {
            eprintln!("rotation 3D ignorée : pas de surface piale dans la séance");
        } else if let Err(e) = rotation_3d(&mut m, &objets, images_3d) {
            eprintln!("rotation 3D impossible : {e}");
        }
    }

    // 5. Cas lourds synthétiques.
    if !drapeau("--sans-lourds") {
        let adulte = volume_synthetique([300, 360, 300], 0.6, 0.0);
        let seg_adulte = etiquettes_depuis(&adulte);
        recomposition(&mut m, "adulte_0.6mm", &adulte, &seg_adulte, repetitions.min(7));
        drop((adulte, seg_adulte));
        let serie: Vec<Arc<Volume>> = (0..64)
            .map(|t| volume_synthetique([128, 128, 72], 1.5, t as f32 * 0.1))
            .collect();
        let (plan, centre, taille) = cadrage(&serie[0], Orientation::Axiale, PAS_MM);
        let mut t = 0;
        let ms = mediane_ms(64, || {
            let couches = vec![Couche::image(serie[t % 64].clone())];
            std::hint::black_box(composer(&plan, centre, taille, PAS_MM, &couches));
            t += 1;
        });
        m.ajouter("serie4d", "64 volumes, instant suivant, 1 couche", ms, "ms");
    }

    // 6. Mémoire.
    if let Some(mo) = memoire_max_mo() {
        m.ajouter("memoire_max", "pic du processus", mo, "Mo");
    }

    if let Some(sortie) = option("--sortie") {
        let mut tsv = String::from("mesure\tcas\tvaleur\tunite\n");
        for (mesure, cas, valeur, unite) in &m.0 {
            tsv.push_str(&format!("{mesure}\t{cas}\t{valeur:.3}\t{unite}\n"));
        }
        std::fs::write(&sortie, tsv)?;
        eprintln!("écrit : {sortie}");
    }
    Ok(())
}
