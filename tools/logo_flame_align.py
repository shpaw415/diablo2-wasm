#!/usr/bin/env python3
"""logo_flame_align.py — ré-aligne les frames du logo en flammes sur leurs
LETTRES. L'extraction d'origine a recadré chaque frame indépendamment : le
centroïde des lettres (pixels sombres opaques) dérive de ±2-3 px entre les
frames — le titre « danse » quand les flammes s'animent. On mesure le
centroïde sombre de chaque frame et on la repose dans un canvas fixe décalé
de (réf - frame) : les lettres redeviennent immobiles au pixel près, seules
les flammes bougent.
"""
from PIL import Image
import glob, os

MENU = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'demo', 'assets', 'menu')


def dark_centroid(im):
    w, h = im.size
    px = im.load()
    sx = sy = n = 0
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a > 128 and r + g + b < 330:
                sx += x
                sy += y
                n += 1
    return (sx / n, sy / n) if n else (w / 2, h / 2)


def main():
    for side in ('l', 'r'):
        paths = sorted(glob.glob(os.path.join(MENU, f'logo_flame_{side}_*.png')))
        frames = [Image.open(p).convert('RGBA') for p in paths]
        W, H = frames[0].size
        ref = dark_centroid(frames[0])
        for p, im in zip(paths, frames):
            c = dark_centroid(im)
            dx = round(ref[0] - c[0])
            dy = round(ref[1] - c[1])
            if dx or dy:
                fixed = Image.new('RGBA', (W, H), (0, 0, 0, 0))
                fixed.paste(im, (dx, dy), im)
                fixed.save(p)
                print(f'{os.path.basename(p)}: décalée de ({dx},{dy})')
            else:
                print(f'{os.path.basename(p)}: déjà alignée')
        print(f'{side}: {len(paths)} frames ré-alignées sur le centroïde de la frame 00')


if __name__ == '__main__':
    main()
