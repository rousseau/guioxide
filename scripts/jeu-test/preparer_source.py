#!/usr/bin/env python3
"""Prépare la source et les gabarits du jeu de test fantôme (docs/donnees.md, « Jeu de test »).

Entrée : l'atlas CINeMA dHCP fœtal (Zenodo 17023473, `dhcp_fetal.tar.gz`, CC BY 4.0), dézippé : `T2w_ga=21-37.nii.gz` et
`SegEM9_ga=21-37.nii.gz` (17 volumes, 0,5 mm, une semaine chacun).
Sortie, dans `--sortie` : `source_T2w.nii.gz` (volume de l'âge demandé, recadré, intensités sur 0-1000), `source_mask.nii.gz`
(étiquettes > 0), `gabarit-{axial,coronal,sagittal}.nii.gz` (trois piles vides, en-têtes synthétiques de trous de 0,8125 mm
dans le plan et de coupes de 3,5 mm : seuls leurs en-têtes et leurs dimensions servent à `medx svr simulate`).
Aucune donnée de patiente : l'atlas est une moyenne publique, les gabarits sont fabriqués ici.

Usage : python3 -I preparer_source.py --atlas dossier_atlas --ga 32 --sortie dossier
"""

import argparse
import os

import nibabel as nib
import numpy as np

PLAN = 0.8125
COUPE = 3.5
# Matrices de permutation des axes (colonne = axe de la pile : x, y, z) vers l'espace RAS : axial, coronal, sagittal.
ORIENTATIONS = {
    "axial": [(0, 0), (1, 1), (2, 2)],
    "coronal": [(0, 0), (2, 1), (1, 2)],
    "sagittal": [(1, 0), (2, 1), (0, 2)],
}


def volume_de_l_age(chemin, ga, premiere=21):
    """Le volume de la semaine `ga` d'un fichier 4D qui commence à `premiere` ; `(donnees, image)`."""
    image = nib.load(chemin)
    k = int(round(ga)) - premiere
    if not 0 <= k < image.shape[3]:
        raise SystemExit(f"{chemin} : l'âge {ga} SA est hors de {premiere}-{premiere + image.shape[3] - 1}")
    return np.asarray(image.dataobj[..., k], dtype=np.float32), image


def recadrer(masque, marge):
    """Boîte englobante du masque, élargie de `marge` voxels (bornée au volume) : trois tranches."""
    pts = np.argwhere(masque)
    bas = np.maximum(pts.min(axis=0) - marge, 0)
    haut = np.minimum(pts.max(axis=0) + marge + 1, masque.shape)
    return tuple(slice(int(b), int(h)) for b, h in zip(bas, haut))


def affine_gabarit(orientation, dims, centre_mm):
    """Affine d'une pile synthétique : axes permutés selon `orientation`, voxels PLAN × PLAN × COUPE, centrée sur `centre_mm`."""
    a = np.zeros((4, 4))
    a[3, 3] = 1.0
    pas = (PLAN, PLAN, COUPE)
    for axe_pile, (axe_monde, _) in enumerate(ORIENTATIONS[orientation]):
        a[axe_monde, axe_pile] = pas[axe_pile]
    centre_voxel = (np.array(dims) - 1) / 2.0
    a[:3, 3] = np.asarray(centre_mm) - a[:3, :3] @ centre_voxel
    return a


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--atlas", required=True)
    p.add_argument("--ga", type=float, default=32)
    p.add_argument("--sortie", required=True)
    p.add_argument("--marge", type=int, default=12)
    a = p.parse_args()
    os.makedirs(a.sortie, exist_ok=True)
    t2, im = volume_de_l_age(os.path.join(a.atlas, "T2w_ga=21-37.nii.gz"), a.ga)
    seg, _ = volume_de_l_age(os.path.join(a.atlas, "SegEM9_ga=21-37.nii.gz"), a.ga)
    masque = seg > 0
    boite = recadrer(masque, a.marge)
    t2 = t2[boite]
    masque = masque[boite]
    t2 = 1000.0 * t2 / max(float(t2.max()), 1e-6)
    affine = im.affine.copy()
    affine[:3, 3] = affine[:3, :3] @ np.array([b.start for b in boite], dtype=float) + affine[:3, 3]
    nib.save(nib.Nifti1Image(t2.astype(np.float32), affine), os.path.join(a.sortie, "source_T2w.nii.gz"))
    nib.save(nib.Nifti1Image(masque.astype(np.uint8), affine), os.path.join(a.sortie, "source_mask.nii.gz"))
    centre = affine @ np.append((np.array(masque.shape) - 1) / 2.0, 1.0)
    for nom, dims in {"axial": (160, 160, 36), "coronal": (160, 160, 36), "sagittal": (160, 160, 36)}.items():
        g = nib.Nifti1Image(np.zeros(dims, dtype=np.float32), affine_gabarit(nom, dims, centre[:3]))
        g.header.set_zooms((PLAN, PLAN, COUPE))
        nib.save(g, os.path.join(a.sortie, f"gabarit-{nom}.nii.gz"))
    print(f"source {t2.shape} voxels de 0,5 mm, masque {int(masque.sum())} voxels ; 3 gabarits -> {a.sortie}")


if __name__ == "__main__":
    main()
