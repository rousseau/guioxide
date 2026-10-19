//! `guioxide-geom` : repères de guioxide.
//!
//! Repère monde : **RAS, en mm** (convention NIfTI), commun aux volumes et aux maillages. Toutes les vues partagent un
//! curseur exprimé dans ce repère, jamais en indices de voxel (voir `docs/architecture.md`).
//!
//! Jalon 1 : seul le point monde existe. L'affine voxel → monde, les plans de coupe et l'échantillonnage arrivent au
//! jalon 3.

/// Point du repère monde : RAS, en millimètres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointMonde(pub [f64; 3]);
