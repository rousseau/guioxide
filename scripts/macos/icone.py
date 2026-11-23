"""Dessine l'icône de guioxide (1024 x 1024, PNG) : une coupe de cerveau, contours piale (jaune) et blanche (bleu)
comme dans l'onglet Surfaces, réticule du curseur (orange) sur le fond du thème.

    python3 scripts/macos/icone.py crates/guioxide/assets/icone.png

Demande numpy et Pillow. L'image produite est versionnée : ce script ne sert qu'à la refaire.
"""

import sys

import numpy as np
from PIL import Image, ImageDraw

S = 4  # suréchantillonnage (anticrénelage)
N = 1024 * S
FOND_HAUT, FOND_BAS = (0x22, 0x27, 0x2E), (0x11, 0x13, 0x16)
BORDURE = (0x2A, 0x2F, 0x37)
PIALE, BLANCHE, ACCENT = (0xF2, 0xC9, 0x4C), (0x7F, 0xB3, 0xE6), (0xE0, 0x7B, 0x39)


def contour(rx, ry, sillons, n=4000):
    """Contour fermé d'une coupe axiale : ellipse creusée de sillons (gaussiennes angulaires de profondeur variable) et
    de la scissure interhémisphérique en haut et en bas."""
    t = np.linspace(0, 2 * np.pi, n, endpoint=False)
    module = np.ones_like(t)
    for angle, profondeur, largeur in sillons:
        ecart = np.angle(np.exp(1j * (t - angle)))
        module -= profondeur * np.exp(-(ecart**2) / (2 * largeur**2))
    cx, cy = N / 2, N / 2
    return np.stack([cx + rx * S * module * np.cos(t), cy - ry * S * module * np.sin(t)], axis=1)


def trait(d, points, couleur, epaisseur):
    """Trait épais et lisse : disques jointifs le long du contour (une ligne épaisse de PIL se hache aux jointures)."""
    r = epaisseur * S / 2
    for x, y in points:
        d.ellipse((x - r, y - r, x + r, y + r), fill=couleur)


def sillons():
    """Sillons symétriques (hémisphères gauche et droit), profondeurs et largeurs fixées par une graine."""
    rng = np.random.default_rng(7)
    liste = [(np.pi / 2, 0.30, 0.045), (3 * np.pi / 2, 0.22, 0.05)]  # scissure, en haut et en bas
    for angle in np.linspace(0.22, np.pi / 2 - 0.2, 4).tolist() + np.linspace(np.pi / 2 + 0.2, np.pi - 0.1, 4).tolist():
        p, l = rng.uniform(0.07, 0.15), rng.uniform(0.035, 0.05)
        liste += [(angle, p, l), (np.pi - angle, p, l)] if angle < np.pi / 2 else []
    for angle in np.linspace(np.pi + 0.25, 3 * np.pi / 2 - 0.25, 4):
        p, l = rng.uniform(0.07, 0.14), rng.uniform(0.035, 0.05)
        liste += [(angle, p, l), (3 * np.pi - angle, p, l)]
    return liste


def main(sortie):
    # Fond : carré arrondi de la grille d'icônes macOS (824 px dans 1024, rayon ~185), dégradé vertical.
    marge, rayon = 100 * S, 185 * S
    masque = Image.new("L", (N, N), 0)
    ImageDraw.Draw(masque).rounded_rectangle((marge, marge, N - marge, N - marge), rayon, fill=255)
    k = np.linspace(0, 1, N)[:, None, None]
    degrade = (np.array(FOND_HAUT) * (1 - k) + np.array(FOND_BAS) * k).repeat(N, axis=1).astype(np.uint8)
    image = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    image.paste(Image.fromarray(degrade, "RGB"), (0, 0), masque)
    d = ImageDraw.Draw(image)
    d.rounded_rectangle((marge, marge, N - marge, N - marge), rayon, outline=BORDURE, width=6 * S)

    # Réticule du curseur, comme dans les vues de coupe.
    c = N // 2
    for xy in [(c, marge + 70 * S, c, N - marge - 70 * S), (marge + 70 * S, c, N - marge - 70 * S, c)]:
        d.line(xy, fill=ACCENT + (150,), width=5 * S)

    plis = sillons()
    trait(d, contour(285, 340, [(a, p * 0.75, l) for a, p, l in plis]), PIALE, 30)
    trait(d, contour(228, 280, [(a, p * 1.25, l * 1.1) for a, p, l in plis]), BLANCHE, 26)
    d.ellipse((c - 20 * S, c - 20 * S, c + 20 * S, c + 20 * S), outline=ACCENT, width=8 * S)

    image.resize((1024, 1024), Image.LANCZOS).save(sortie)


if __name__ == "__main__":
    main(sys.argv[1])
