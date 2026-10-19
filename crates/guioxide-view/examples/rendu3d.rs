//! Rendu 3D hors écran de surfaces GIfTI, par le même code que la vue 3D (GPU, sans fenêtre) : contrôle visuel.
//!
//! ```text
//! cargo run --release -p guioxide-view --example rendu3d -- <sortie.ppm> <surface.surf.gii>
//!     [--champ <champ.shape.gii>] [--divergent] [--vue laterale|mediale|dorsale] [--droite]
//! ```
//!
//! Le champ, s'il est donné, colore la surface (palette séquentielle, ou divergente avec `--divergent`).

use std::sync::Arc;

use egui_wgpu::wgpu;
use guioxide_data::{lire_champ, lire_surface};
use guioxide_view::palettes::colorer;
use guioxide_view::vue3d::{Objet3d, Rendu3d, VuePredefinie};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |nom: &str| {
        args.iter()
            .position(|a| a == nom)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(sortie), Some(surface)) = (args.first(), args.get(1)) else {
        return Err("usage : rendu3d <sortie.ppm> <surface.surf.gii> [options]".into());
    };
    let maillage = Arc::new(lire_surface(surface.as_ref())?);
    let couleurs = match option("--champ") {
        Some(c) => {
            let champ = lire_champ(c.as_ref())?;
            let (couleurs, (bas, haut)) = colorer(&champ, args.iter().any(|a| a == "--divergent"));
            eprintln!("champ entre {bas:.3} et {haut:.3}");
            couleurs
        }
        None => vec![[0xC8, 0xC2, 0xB8]],
    };
    let objets = [Objet3d {
        maillage,
        couleurs: Arc::new(couleurs),
        visible: true,
    }];
    let vue = match option("--vue").as_deref() {
        None | Some("laterale") => VuePredefinie::Laterale,
        Some("mediale") => VuePredefinie::Mediale,
        Some("dorsale") => VuePredefinie::Dorsale,
        Some(v) => return Err(format!("vue inconnue : {v}").into()),
    };
    let mut camera = Rendu3d::cadrer(&objets);
    camera.regarder(vue, !args.iter().any(|a| a == "--droite"));

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adaptateur = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adaptateur.request_device(&Default::default()))?;
    eprintln!(
        "GPU : {} ({:?})",
        adaptateur.get_info().name,
        adaptateur.get_info().backend
    );

    let taille = [900u32, 700];
    let debut = std::time::Instant::now();
    let mut rendu = Rendu3d::new(&device);
    let texture = rendu.rendre(&device, &queue, taille, &camera, &objets);

    // Copie de la texture dans un tampon lisible (lignes alignées sur 256 octets).
    let ligne = (taille[0] * 4).div_ceil(256) * 256;
    let tampon = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (ligne * taille[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encodeur = device.create_command_encoder(&Default::default());
    encodeur.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &tampon,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ligne),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    queue.submit([encodeur.finish()]);
    tampon.map_async(wgpu::MapMode::Read, .., |r| r.expect("lecture du tampon"));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let octets = tampon.get_mapped_range(..)?;
    eprintln!("rendu et relecture en {:.1?}", debut.elapsed());

    let mut ppm = format!("P6\n{} {}\n255\n", taille[0], taille[1]).into_bytes();
    for y in 0..taille[1] as usize {
        for x in 0..taille[0] as usize {
            let i = y * ligne as usize + 4 * x;
            ppm.extend_from_slice(&octets[i..i + 3]);
        }
    }
    std::fs::write(sortie, ppm)?;
    Ok(())
}
