//! Lecture GIfTI **provisoire** : surfaces (`.surf.gii`) et champs par sommet (`.shape.gii`).
//!
//! À remplacer par `surfoxide-io` (lecteur partagé avec surfoxide et gyroxide) dès qu'il est publié ; mêmes
//! conventions : encodages `ASCII`, `Base64Binary`, `GZipBase64Binary` (flux gzip ou zlib), ordre ligne ou colonne,
//! boutisme ; les coordonnées sont prises telles quelles, dans le repère monde (RAS, mm) du contrat — la matrice
//! `CoordinateSystemTransformMatrix` n'est pas appliquée.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use base64::Engine;
use quick_xml::events::Event;

use crate::ErreurDonnees;

/// Un maillage triangulaire de surface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Maillage {
    pub sommets: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
    /// `AnatomicalStructurePrimary` (`CortexLeft`, `CortexRight`…).
    pub structure: Option<String>,
}

impl Maillage {
    /// Boîte englobante (min, max).
    pub fn boite(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for s in &self.sommets {
            for c in 0..3 {
                min[c] = min[c].min(s[c]);
                max[c] = max[c].max(s[c]);
            }
        }
        (min, max)
    }
}

/// Un `DataArray` lu : attributs, métadonnées, valeurs décodées.
struct Tableau {
    intent: String,
    dims: Vec<usize>,
    meta: BTreeMap<String, String>,
    valeurs: Vec<f64>,
}

/// Lit une surface : le premier tableau `NIFTI_INTENT_POINTSET` (n × 3) et le premier `NIFTI_INTENT_TRIANGLE` (m × 3).
pub fn lire_surface(chemin: &Path) -> Result<Maillage, ErreurDonnees> {
    let tableaux = lire_tableaux(chemin)?;
    let trouver = |intent: &str| {
        tableaux
            .iter()
            .find(|t| t.intent == intent)
            .ok_or_else(|| ErreurDonnees::format(chemin, format!("pas de tableau {intent}")))
    };
    let (points, triangles) = (trouver("NIFTI_INTENT_POINTSET")?, trouver("NIFTI_INTENT_TRIANGLE")?);
    for t in [points, triangles] {
        if t.dims.len() != 2 || t.dims[1] != 3 {
            return Err(ErreurDonnees::format(
                chemin,
                format!("{} : dimensions {:?}, n × 3 attendu", t.intent, t.dims),
            ));
        }
    }
    let sommets: Vec<[f32; 3]> = points
        .valeurs
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let n = sommets.len() as f64;
    let mut faces = Vec::with_capacity(triangles.valeurs.len() / 3);
    for c in triangles.valeurs.as_chunks::<3>().0 {
        if c.iter().any(|&i| i < 0.0 || i >= n) {
            return Err(ErreurDonnees::format(
                chemin,
                format!("face {c:?} : indice hors des {n} sommets"),
            ));
        }
        faces.push([c[0] as u32, c[1] as u32, c[2] as u32]);
    }
    let structure = points.meta.get("AnatomicalStructurePrimary").cloned();
    Ok(Maillage {
        sommets,
        faces,
        structure,
    })
}

/// Lit un champ par sommet : le premier tableau du fichier (`.shape.gii`, `.func.gii`).
pub fn lire_champ(chemin: &Path) -> Result<Vec<f32>, ErreurDonnees> {
    let tableaux = lire_tableaux(chemin)?;
    let t = tableaux
        .first()
        .ok_or_else(|| ErreurDonnees::format(chemin, "aucun DataArray"))?;
    Ok(t.valeurs.iter().map(|&v| v as f32).collect())
}

fn lire_tableaux(chemin: &Path) -> Result<Vec<Tableau>, ErreurDonnees> {
    let texte = std::fs::read_to_string(chemin).map_err(|e| ErreurDonnees::lecture(chemin, e))?;
    let erreur = |m: String| ErreurDonnees::format(chemin, m);
    let mut lecteur = quick_xml::Reader::from_str(&texte);
    let mut tableaux = Vec::new();
    // Tableau en cours de lecture : attributs, métadonnées, texte du nœud <Data>.
    let mut courant: Option<(BTreeMap<String, String>, BTreeMap<String, String>)> = None;
    let mut chemin_xml: Vec<String> = Vec::new();
    let (mut nom_md, mut valeur_md, mut donnees) = (String::new(), String::new(), String::new());
    loop {
        match lecteur.read_event().map_err(|e| erreur(format!("XML : {e}")))? {
            Event::Start(e) => {
                let nom = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if nom == "DataArray" {
                    let mut attributs = BTreeMap::new();
                    for a in e.attributes().flatten() {
                        let cle = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                        let valeur = a
                            .unescape_value()
                            .map_err(|e| erreur(format!("XML : {e}")))?
                            .into_owned();
                        attributs.insert(cle, valeur);
                    }
                    courant = Some((attributs, BTreeMap::new()));
                    donnees.clear();
                }
                if nom == "MD" {
                    (nom_md, valeur_md) = (String::new(), String::new());
                }
                chemin_xml.push(nom);
            }
            Event::End(e) => {
                let nom = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                chemin_xml.pop();
                match nom.as_str() {
                    "MD" => {
                        if let Some((_, meta)) = &mut courant {
                            meta.insert(nom_md.trim().to_owned(), valeur_md.trim().to_owned());
                        }
                    }
                    "DataArray" => {
                        let (attributs, meta) =
                            courant.take().ok_or_else(|| erreur("</DataArray> sans début".into()))?;
                        tableaux.push(decoder(&attributs, meta, &donnees).map_err(erreur)?);
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                let t = t.decode().map_err(|e| erreur(format!("XML : {e}")))?;
                ajouter_texte(&chemin_xml, &t, &mut nom_md, &mut valeur_md, &mut donnees);
            }
            Event::CData(t) => {
                let t = String::from_utf8_lossy(&t).into_owned();
                ajouter_texte(&chemin_xml, &t, &mut nom_md, &mut valeur_md, &mut donnees);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(tableaux)
}

fn ajouter_texte(chemin_xml: &[String], texte: &str, nom: &mut String, valeur: &mut String, donnees: &mut String) {
    match chemin_xml.last().map(String::as_str) {
        Some("Name") if chemin_xml.iter().any(|n| n == "MD") => nom.push_str(texte),
        Some("Value") if chemin_xml.iter().any(|n| n == "MD") => valeur.push_str(texte),
        Some("Data") => donnees.push_str(texte),
        _ => {}
    }
}

fn decoder(
    attributs: &BTreeMap<String, String>,
    meta: BTreeMap<String, String>,
    texte: &str,
) -> Result<Tableau, String> {
    let attribut = |n: &str| attributs.get(n).map(String::as_str);
    let intent = attribut("Intent").unwrap_or("NIFTI_INTENT_NONE").to_owned();
    let dimensionnalite: usize = attribut("Dimensionality")
        .unwrap_or("1")
        .parse()
        .map_err(|_| "Dimensionality invalide")?;
    let dims: Vec<usize> = (0..dimensionnalite)
        .map(|i| {
            attribut(&format!("Dim{i}"))
                .and_then(|d| d.parse().ok())
                .ok_or(format!("Dim{i} absent ou invalide"))
        })
        .collect::<Result<_, _>>()?;
    let total: usize = dims.iter().product();
    let type_ = attribut("DataType").unwrap_or("");
    let valeurs = match attribut("Encoding").unwrap_or("ASCII") {
        "ASCII" => texte
            .split_whitespace()
            .map(|v| v.parse::<f64>().map_err(|_| format!("valeur ASCII invalide : {v}")))
            .collect::<Result<Vec<_>, _>>()?,
        encodage @ ("Base64Binary" | "GZipBase64Binary") => {
            let propre: String = texte.chars().filter(|c| !c.is_whitespace()).collect();
            let mut octets = base64::engine::general_purpose::STANDARD
                .decode(propre)
                .map_err(|e| format!("base64 : {e}"))?;
            if encodage == "GZipBase64Binary" {
                octets = decompresser(&octets)?;
            }
            let gros_boutiste = attribut("Endian") == Some("BigEndian");
            convertir(&octets, type_, gros_boutiste)?
        }
        autre => return Err(format!("encodage non pris en charge : {autre}")),
    };
    if valeurs.len() != total {
        return Err(format!(
            "{intent} : {} valeurs pour les dimensions {dims:?}",
            valeurs.len()
        ));
    }
    let valeurs = if attribut("ArrayIndexingOrder") == Some("ColumnMajorOrder") && dims.len() == 2 {
        let (n, m) = (dims[0], dims[1]);
        (0..n * m).map(|k| valeurs[(k % m) * n + k / m]).collect()
    } else {
        valeurs
    };
    Ok(Tableau {
        intent,
        dims,
        meta,
        valeurs,
    })
}

/// Flux gzip (signature 1f 8b) ou zlib : les deux se rencontrent sous `GZipBase64Binary`.
fn decompresser(octets: &[u8]) -> Result<Vec<u8>, String> {
    let mut sortie = Vec::new();
    let resultat = if octets.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(octets).read_to_end(&mut sortie)
    } else {
        flate2::read::ZlibDecoder::new(octets).read_to_end(&mut sortie)
    };
    resultat.map_err(|e| format!("décompression : {e}"))?;
    Ok(sortie)
}

fn convertir(octets: &[u8], type_: &str, gros_boutiste: bool) -> Result<Vec<f64>, String> {
    macro_rules! lire {
        ($t:ty, $n:expr) => {
            octets
                .chunks_exact($n)
                .map(|c| {
                    let a: [u8; $n] = c.try_into().unwrap();
                    (if gros_boutiste {
                        <$t>::from_be_bytes(a)
                    } else {
                        <$t>::from_le_bytes(a)
                    }) as f64
                })
                .collect()
        };
    }
    Ok(match type_ {
        "NIFTI_TYPE_FLOAT32" => lire!(f32, 4),
        "NIFTI_TYPE_FLOAT64" => lire!(f64, 8),
        "NIFTI_TYPE_INT32" => lire!(i32, 4),
        "NIFTI_TYPE_UINT8" => octets.iter().map(|&b| b as f64).collect(),
        autre => return Err(format!("type de données non pris en charge : {autre}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tableau(intent: &str, type_: &str, dims: &[usize], encodage: &str, donnees: &str, ordre: &str) -> String {
        let d: String = dims
            .iter()
            .enumerate()
            .map(|(i, n)| format!(" Dim{i}=\"{n}\""))
            .collect();
        format!(
            "<DataArray Intent=\"{intent}\" DataType=\"{type_}\" ArrayIndexingOrder=\"{ordre}\" Dimensionality=\"{}\"{d} \
             Encoding=\"{encodage}\" Endian=\"LittleEndian\"><MetaData><MD><Name><![CDATA[AnatomicalStructurePrimary]]>\
             </Name><Value><![CDATA[CortexLeft]]></Value></MD></MetaData><Data>{donnees}</Data></DataArray>",
            dims.len()
        )
    }

    fn fichier(tableaux: &[String]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let chemin = dir.path().join("s.surf.gii");
        let xml = format!(
            "<?xml version=\"1.0\"?>\n<GIFTI Version=\"1.0\">{}</GIFTI>",
            tableaux.concat()
        );
        std::fs::write(&chemin, xml).unwrap();
        (dir, chemin)
    }

    fn base64(octets: &[u8], compresser: bool) -> String {
        let octets = if compresser {
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(octets).unwrap();
            z.finish().unwrap()
        } else {
            octets.to_vec()
        };
        base64::engine::general_purpose::STANDARD.encode(octets)
    }

    /// Tétraèdre : 4 sommets, 4 faces.
    const SOMMETS: [[f32; 3]; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.5]];
    const FACES: [[i32; 3]; 4] = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];

    fn attendu() -> Maillage {
        Maillage {
            sommets: SOMMETS.to_vec(),
            faces: FACES.iter().map(|f| f.map(|i| i as u32)).collect(),
            structure: Some("CortexLeft".into()),
        }
    }

    #[test]
    fn surface_ascii() {
        let p: String = SOMMETS.iter().flatten().map(|v| format!("{v} ")).collect();
        let f: String = FACES.iter().flatten().map(|v| format!("{v} ")).collect();
        let (_d, c) = fichier(&[
            tableau(
                "NIFTI_INTENT_POINTSET",
                "NIFTI_TYPE_FLOAT32",
                &[4, 3],
                "ASCII",
                &p,
                "RowMajorOrder",
            ),
            tableau(
                "NIFTI_INTENT_TRIANGLE",
                "NIFTI_TYPE_INT32",
                &[4, 3],
                "ASCII",
                &f,
                "RowMajorOrder",
            ),
        ]);
        assert_eq!(lire_surface(&c).unwrap(), attendu());
    }

    #[test]
    fn surface_binaire_compressee_et_ordre_colonne() {
        let p: Vec<u8> = SOMMETS.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        // Faces en ordre colonne : toutes les premières composantes, puis les deuxièmes, puis les troisièmes.
        let f: Vec<u8> = (0..3)
            .flat_map(|c| FACES.iter().map(move |f| f[c]))
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (_d, c) = fichier(&[
            tableau(
                "NIFTI_INTENT_POINTSET",
                "NIFTI_TYPE_FLOAT32",
                &[4, 3],
                "GZipBase64Binary",
                &base64(&p, true),
                "RowMajorOrder",
            ),
            tableau(
                "NIFTI_INTENT_TRIANGLE",
                "NIFTI_TYPE_INT32",
                &[4, 3],
                "Base64Binary",
                &base64(&f, false),
                "ColumnMajorOrder",
            ),
        ]);
        assert_eq!(lire_surface(&c).unwrap(), attendu());
    }

    #[test]
    fn champ_et_erreurs() {
        let v: Vec<u8> = [0.5f32, 1.5, 2.5, 3.5].iter().flat_map(|v| v.to_le_bytes()).collect();
        let (_d, c) = fichier(&[tableau(
            "NIFTI_INTENT_SHAPE",
            "NIFTI_TYPE_FLOAT32",
            &[4],
            "GZipBase64Binary",
            &base64(&v, true),
            "RowMajorOrder",
        )]);
        assert_eq!(lire_champ(&c).unwrap(), [0.5, 1.5, 2.5, 3.5]);
        assert!(lire_surface(&c).unwrap_err().to_string().contains("POINTSET"));

        let (_d, c) = fichier(&[
            tableau(
                "NIFTI_INTENT_POINTSET",
                "NIFTI_TYPE_FLOAT32",
                &[1, 3],
                "ASCII",
                "0 0 0",
                "RowMajorOrder",
            ),
            tableau(
                "NIFTI_INTENT_TRIANGLE",
                "NIFTI_TYPE_INT32",
                &[1, 3],
                "ASCII",
                "0 1 2",
                "RowMajorOrder",
            ),
        ]);
        let e = lire_surface(&c).unwrap_err().to_string();
        assert!(e.contains("hors des") && e.contains("s.surf.gii"), "{e}");
    }
}
