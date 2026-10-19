//! Rendu hors écran d'une coupe, composée comme dans l'interface : contrôle visuel sans fenêtre.
//!
//! ```text
//! cargo run --release -p guioxide-view --example coupe -- <image.nii.gz> <sortie.ppm>
//!     [--etiquettes <dseg.nii.gz>] [--contour <masque.nii.gz>]
//!     [--orientation axiale|coronale|sagittale] [--curseur x,y,z] [--pas mm]
//! ```
//!
//! Sans `--curseur`, la coupe passe par le centre de l'image. Écrit une image PPM (P6).

use guioxide_data::{TableEtiquettes, lire_volume};
use guioxide_geom::{Orientation, Plan};
use guioxide_view::coupe::{Couche, Rendu, composer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |nom: &str| {
        args.iter()
            .position(|a| a == nom)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let positionnels: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--")))
        .map(|(_, a)| a)
        .collect();
    let [image, sortie] = positionnels[..] else {
        return Err("usage : coupe <image.nii.gz> <sortie.ppm> [options]".into());
    };

    let image = lire_volume(image.as_ref())?.volume;
    let mut couches = vec![Couche::image(image.clone())];
    if let Some(chemin) = option("--etiquettes") {
        let rendu = Rendu::Etiquettes {
            table: TableEtiquettes::default(),
            opacite: 0.45,
        };
        couches.push(Couche {
            volume: lire_volume(chemin.as_ref())?.volume,
            rendu,
            visible: true,
        });
    }
    if let Some(chemin) = option("--contour") {
        let rendu = Rendu::Contour {
            couleur: [0xE0, 0x7B, 0x39],
        };
        couches.push(Couche {
            volume: lire_volume(chemin.as_ref())?.volume,
            rendu,
            visible: true,
        });
    }
    let orientation = match option("--orientation").as_deref() {
        None | Some("axiale") => Orientation::Axiale,
        Some("coronale") => Orientation::Coronale,
        Some("sagittale") => Orientation::Sagittale,
        Some(o) => return Err(format!("orientation inconnue : {o}").into()),
    };
    let curseur = match option("--curseur") {
        Some(t) => {
            let v: Vec<f64> = t.split(',').map(str::parse).collect::<Result<_, _>>()?;
            [v[0], v[1], v[2]]
        }
        None => image.centre().0,
    };
    let pas: f64 = option("--pas").map(|p| p.parse()).transpose()?.unwrap_or(0.5);

    // Cadrage : la boîte de l'image, projetée sur le plan.
    let plan = Plan::aligne(orientation, curseur);
    let (min, max) = image.boite();
    let etendue = |d: [f64; 3]| (0..3).map(|c| (max[c] - min[c]) * d[c].abs()).sum::<f64>();
    let taille = [
        (etendue(plan.u) / pas).ceil() as usize,
        (etendue(plan.v) / pas).ceil() as usize,
    ];
    let centre = plan.vers_plan(image.centre().0);
    let debut = std::time::Instant::now();
    let rendu = composer(&plan, centre, taille, pas, &couches);
    let duree = debut.elapsed();

    let mut ppm = format!("P6\n{} {}\n255\n", taille[0], taille[1]).into_bytes();
    for p in &rendu.pixels {
        ppm.extend_from_slice(&[p.r(), p.g(), p.b()]);
    }
    std::fs::write(sortie, ppm)?;
    eprintln!(
        "{sortie} : {} × {} pixels de {pas} mm, coupe {} en {curseur:?}, composée en {duree:.1?}",
        taille[0],
        taille[1],
        orientation.nom()
    );
    Ok(())
}
