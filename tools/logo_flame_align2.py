#!/usr/bin/env python3
"""logo_flame_align2.py — alignement par corrélation de gabarit.

v1 (centroïde sombre) laissait ~0,6px de dérive : le centroïde est pollué
par les flammes qui modifient l'ensemble mesuré à chaque frame. v2 mesure
le déplacement qui MINIMISE la différence réelle contre la frame 0, restreint
au NOYAU STABLE = pixels sombres opaques dans TOUTES les frames (le cœur des
lettres, jamais touché par les flammes). Recherche exhaustive ±8 px.
"""
from PIL import Image
import glob, os
import numpy as np

MENU = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'demo', 'assets', 'menu')
RANGE = 8


def main():
    for side in ('l', 'r'):
        paths = sorted(glob.glob(os.path.join(MENU, f'logo_flame_{side}_*.png')))
        frames = [Image.open(p).convert('RGBA') for p in paths]
        W, H = frames[0].size
        arrs = []
        for im in frames:
            a = np.asarray(im, dtype=np.int16)
            arrs.append(a)
        # noyau stable : sombre opaque dans TOUTES les frames
        dark = [(a[..., 3] > 128) & (a[..., 0] + a[..., 1] + a[..., 2] < 330) for a in arrs]
        core = dark[0].copy()
        for d in dark[1:]:
            core &= d
        ys, xs = np.nonzero(core)
        print(f'{side}: noyau stable {len(ys)} px')
        base = arrs[0]
        shifts = []
        for i, a in enumerate(arrs):
            if i == 0:
                shifts.append((0, 0))
                continue
            best, bdx, bdy = None, 0, 0
            for dy in range(-RANGE, RANGE + 1):
                for dx in range(-RANGE, RANGE + 1):
                    ys2, xs2 = ys + dy, xs + dx
                    if ys2.min() < 0 or xs2.min() < 0 or ys2.max() >= H or xs2.max() >= W:
                        continue
                    diff = np.abs(a[ys2, xs2] - base[ys, xs]).sum()
                    if best is None or diff < best:
                        best, bdx, bdy = diff, dx, dy
            shifts.append((bdx, bdy))
        for p, im, (dx, dy) in zip(paths, frames, shifts):
            if dx or dy:
                fixed = Image.new('RGBA', (W, H), (0, 0, 0, 0))
                fixed.paste(im, (dx, dy), im)
                fixed.save(p)
        print(f'{side}: shifts appliqués {sorted(set(shifts))} (frame 0 = référence)')


if __name__ == '__main__':
    main()
