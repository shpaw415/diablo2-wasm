#!/usr/bin/env python3
"""bloodmoor_render.py — Rendu + collision de la Blood Moor composée.

Relit assets/map/bloodmoor.ds1 (DS1 v18 écrit par bloodmoor_build.py) avec le
même chemin de code que le Town, et produit :
  assets/map/bloodmoor-render.png           (palette PL2 + gamma 0.7, échelle 1/2)
  assets/map/bloodmoor-collision.json       {width,height,rows}, true = BLOQUÉ
  assets/map/bloodmoor-collision-heat.png   noir = bloqué, blanc = libre
  assets/map/bloodmoor-collision-overlay.png  debug teinté sur le rendu
"""
import os, sys, json
from PIL import Image

BASE = '/home/agent/workspace/diablo-2-wasm'
MAP = os.path.join(BASE, 'assets', 'map')
sys.path.insert(0, os.path.join(BASE, 'tools'))
from map_proto2 import ContainerDT1, Renderer
from bloodmoor_build import parse_ds1, UNIFIED, cell_fields

BLOCK_MASK = 0x1000 | 0x2000 | 0x0004   # cf. evidence/MAP-SPEC.md §4
GAMMA = 0.7


def load_palette():
    pal = list(open(os.path.join(MAP, 'pal_act1_pl2.dat'), 'rb').read()[:768])
    lut = [round(255 * (i / 255) ** GAMMA) for i in range(256)]
    return [lut[v] for v in pal]


def main():
    pal = load_palette()
    ds1_path = os.path.join(MAP, 'bloodmoor.ds1')
    p = parse_ds1(ds1_path)
    print(f"bloodmoor.ds1: v18 {p['W']}x{p['H']} tag={p['tag']} files={len(p['files'])}")
    for f in p['files']:
        print('   ', f)

    tilesets = []
    for fn, _tg1 in UNIFIED:
        c = ContainerDT1(os.path.join(MAP, fn))
        print(f'   {fn}: {c.nbtiles} tuiles')
        tilesets.extend(c.tiles)

    # ---------------------------------------------------------------- rendu
    class DS1Wrap:
        pass
    dsw = DS1Wrap()
    dsw.w, dsw.h = p['W'], p['H']
    dsw.layers = p['layers']
    r = Renderer(dsw, tilesets, pal, wall_head=420)
    r.draw_layer(4)   # sol
    r.draw_layer(0)   # mur1 (props/floor-walls)
    r.draw_layer(2)   # mur3
    r.draw_layer(3)   # mur4
    r.draw_layer(1)   # mur2 (arbres) en dernier
    out = os.path.join(MAP, 'bloodmoor-render.png')
    r.save(out, scale=2)
    print('stats rendu:', r.stats)

    # ---------------------------------------------------------------- collision
    W, H = p['W'], p['H']
    grid = [[True] * W for _ in range(H)]
    n_blocked = n_wall = n_nofloor = n_sub = 0
    for cy in range(H):
        for cx in range(W):
            if cy == 0 or cx == 0 or cy == H - 1 or cx == W - 1:
                continue          # bord de carte : bloqué par convention (cf. Town)
            floor_ok = False
            fb = ft = 0
            wall_hit = False
            for li in (4, 0, 2, 3, 5, 1):
                v = p['layers'][li][cy * W + cx]
                p1, p2, p3, orient = cell_fields(v)
                if p1 == 0 or (orient & 0x80):
                    continue
                idx = p1 + (p2 & 0x0F)
                if idx >= len(tilesets):
                    continue
                tile = tilesets[idx]
                if tile.kind == 'wall':
                    wall_hit = True
                else:
                    floor_ok = True
                    for blk in tile.blocks:
                        ft += 1
                        if blk[2] & BLOCK_MASK:
                            fb += 1
            blocked = wall_hit or (not floor_ok)
            if floor_ok and not wall_hit and ft:
                blocked = fb * 2 >= ft
            if wall_hit:
                n_wall += 1
            elif not floor_ok:
                n_nofloor += 1
            elif blocked:
                n_sub += 1
            grid[cy][cx] = bool(blocked)
    # anneau extérieur bloqué (hors-zone, comme le Town : "défaut = bloqué")
    for cx in range(W):
        grid[0][cx] = grid[H - 1][cx] = True
    for row in grid:
        row[0] = row[W - 1] = True
    free = sum(row.count(False) for row in grid)
    print(f'collision: {free} libres / {W*H} (murs={n_wall}, sans_sol={n_nofloor}, sous-tuiles={n_sub})')
    with open(os.path.join(MAP, 'bloodmoor-collision.json'), 'w') as f:
        json.dump({'width': W, 'height': H, 'rows': grid}, f, separators=(',', ':'))

    # heatmap
    sc = 8
    img = Image.new('L', (W * sc, H * sc), 0)
    px = img.load()
    for cy in range(H):
        for cx in range(W):
            v = 0 if grid[cy][cx] else 255
            for dy in range(sc):
                yy = cy * sc + dy
                for dx in range(sc):
                    px[cx * sc + dx, yy] = v
    img.save(os.path.join(MAP, 'bloodmoor-collision-heat.png'))

    # overlay debug
    render = Image.open(out).convert('RGB')
    ov = render.copy()
    cell_w = render.width // W
    cell_h = render.height // H
    px = ov.load()
    for cy in range(H):
        for cx in range(W):
            tint = (255, 40, 40) if grid[cy][cx] else (40, 255, 40)
            for yy in range(cy * cell_h, (cy + 1) * cell_h):
                for xx in range(cx * cell_w, (cx + 1) * cell_w):
                    r_, g_, b_ = px[xx, yy]
                    px[xx, yy] = ((r_ + 2 * tint[0]) // 3, (g_ + 2 * tint[1]) // 3, (b_ + 2 * tint[2]) // 3)
    ov.save(os.path.join(MAP, 'bloodmoor-collision-overlay.png'))
    print('écrit: bloodmoor-render.png, bloodmoor-collision.json, heat + overlay')


if __name__ == '__main__':
    main()
