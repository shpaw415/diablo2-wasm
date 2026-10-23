#!/usr/bin/env python3
"""map_render2.py — Rendu carte avec la palette authentique du PL2 acte I.

Découverte (voir evidence/MAP-SPEC.md §palette):
  - data\\global\\palette\\act1\\pal.dat  = palette "sombre" (pas celle des niveaux)
  - data\\global\\palette\\act1\\pal.pl2 @0 = 256 entrées RGBA 6 bits (alpha=0)
    = la vraie palette d'affichage de l'acte I (verts/bruns act1).
  Conversion moderne: v (0..252, pas de 4) -> v*255/252.

Produit:
  assets/map/pal_act1_pl2.dat   palette extraite (768 o RGB 8 bits, prête d2game)
  assets/map/level-render-v2.png
  assets/map/level-compare.png  v1 (pal.dat+gamma) | v2 (PL2) côte à côte
"""
import struct, os, sys
from PIL import Image

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAP = os.path.join(BASE, 'assets', 'map')
sys.path.insert(0, os.path.join(BASE, 'tools'))
from map_proto2 import DS1, ContainerDT1, tg1_to_container, Renderer  # noqa: E402

PL2_OFF = 0            # palette de base dans pal.pl2


def load_pl2_palette():
    p = open(os.path.join(MAP, 'pal_act1.pl2'), 'rb').read()
    pal = []
    for i in range(256):
        r, g, b = p[PL2_OFF + 4 * i], p[PL2_OFF + 4 * i + 1], p[PL2_OFF + 4 * i + 2]
        pal += [min(255, r * 255 // 252), min(255, g * 255 // 252), min(255, b * 255 // 252)]
    return pal


GAMMA = 0.7   # lift type "brightness D2" (le jeu applique une rampe vidéo)


def main():
    pal = load_pl2_palette()
    open(os.path.join(MAP, 'pal_act1_pl2.dat'), 'wb').write(bytes(pal))
    lut = [round(255 * (i / 255) ** GAMMA) for i in range(256)]
    pal = [lut[v] for v in pal]

    ds1 = DS1(os.path.join(MAP, 'TownN1.ds1'))
    tilesets = []
    for f in ds1.files:
        tilesets.extend(ContainerDT1(os.path.join(MAP, tg1_to_container(f))).tiles)

    r = Renderer(ds1, tilesets, pal)
    r.draw_layer(4); r.draw_layer(0); r.draw_layer(2); r.draw_layer(5); r.draw_layer(1)
    out = os.path.join(MAP, 'level-render-v2.png')
    r.save(out, scale=2)

    # comparaison côte à côte
    a = Image.open(os.path.join(MAP, 'level-render.png')).convert('RGB')
    b = Image.open(out).convert('RGB')
    h = 900
    aw = int(a.width * h / a.height); bw = int(b.width * h / b.height)
    a = a.resize((aw, h)); b = b.resize((bw, h))
    cmp_img = Image.new('RGB', (aw + bw + 12, h), (20, 20, 20))
    cmp_img.paste(a, (0, 0)); cmp_img.paste(b, (aw + 12, 0))
    cmp_img.save(os.path.join(MAP, 'level-compare.png'))
    print('comparaison:', cmp_img.size)


if __name__ == '__main__':
    main()
