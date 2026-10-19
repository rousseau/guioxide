//! Résumé d'un fichier GIfTI lu comme dans l'interface : surface (sommets, faces, boîte) ou champ par sommet.
//!
//! ```text
//! cargo run --release -p guioxide-data --example gifti -- <fichier.surf.gii | fichier.shape.gii>
//! ```

use guioxide_data::{lire_champ, lire_surface};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chemin = std::env::args().nth(1).ok_or("usage : gifti <fichier.gii>")?;
    let chemin = std::path::Path::new(&chemin);
    if chemin.to_string_lossy().ends_with(".surf.gii") {
        let m = lire_surface(chemin)?;
        let (min, max) = m.boite();
        println!(
            "{} sommets, {} faces, {:?}",
            m.sommets.len(),
            m.faces.len(),
            m.structure
        );
        println!("boîte {min:?} → {max:?}");
        println!("sommet 0 {:?}, face 0 {:?}", m.sommets[0], m.faces[0]);
    } else {
        let v = lire_champ(chemin)?;
        let somme: f64 = v.iter().map(|&x| x as f64).sum();
        println!("{} valeurs, moyenne {:.6}", v.len(), somme / v.len() as f64);
    }
    Ok(())
}
