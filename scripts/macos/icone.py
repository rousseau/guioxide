"""Icône de guioxide (1024 x 1024, PNG) tirée du logo (`crates/guioxide/assets/logo.png`) : carré centré sur le logo,
fond du logo compris, posé dans le carré arrondi de la grille d'icônes macOS (824 px dans 1024, rayon ~185).

    python3 scripts/macos/icone.py crates/guioxide/assets/logo.png crates/guioxide/assets/icone.png

Demande numpy et Pillow. L'image produite est versionnée : ce script ne sert qu'à la refaire si le logo change.
"""

import sys

import numpy as np
from PIL import Image, ImageDraw

COTE, MARGE, RAYON = 1024, 100, 185
OCCUPATION = 0.75  # part de la largeur du carré arrondi prise par le logo
BORDURE = (0xD5, 0xDD, 0xE5)
S = 4  # suréchantillonnage du masque (bords anticrénelés)


def main(logo, sortie):
    source = Image.open(logo).convert("RGB")
    a = np.asarray(source, dtype=int)
    # Boîte du logo : pixels qui s'écartent nettement du fond (médiane de l'image).
    ys, xs = np.nonzero(np.abs(a - np.median(a.reshape(-1, 3), axis=0)).max(axis=2) > 40)
    cx, cy = (xs.min() + xs.max()) / 2, (ys.min() + ys.max()) / 2
    demi = max(xs.max() - xs.min(), ys.max() - ys.min()) / OCCUPATION / 2
    interieur = COTE - 2 * MARGE
    carre = source.crop((round(cx - demi), round(cy - demi), round(cx + demi), round(cy + demi)))
    carre = carre.resize((interieur, interieur), Image.LANCZOS)

    masque = Image.new("L", (COTE * S, COTE * S), 0)
    bord = Image.new("L", (COTE * S, COTE * S), 0)
    boite = (MARGE * S, MARGE * S, (COTE - MARGE) * S, (COTE - MARGE) * S)
    ImageDraw.Draw(masque).rounded_rectangle(boite, RAYON * S, fill=255)
    ImageDraw.Draw(bord).rounded_rectangle(boite, RAYON * S, outline=255, width=4 * S)
    masque, bord = (m.resize((COTE, COTE), Image.LANCZOS) for m in (masque, bord))

    image = Image.new("RGBA", (COTE, COTE), (0, 0, 0, 0))
    fond = Image.new("RGB", (COTE, COTE))
    fond.paste(carre, (MARGE, MARGE))
    image.paste(fond, (0, 0), masque)
    image.paste(Image.new("RGB", (COTE, COTE), BORDURE), (0, 0), bord)
    image.save(sortie)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
