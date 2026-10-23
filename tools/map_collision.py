#!/usr/bin/env python3
"""map_collision.py — Grille de collision booléenne du Town acte I (TownN1.ds1).

Extrait les flags de sous-tuiles des DT1 (mot @8 de l'entête 20 o de chaque
sous-bloc — voir evidence/MAP-SPEC.md) et produit:
  - assets/map/collision.json      {width, height, rows[[bool]]}, true = BLOQUÉ
  - assets/map/collision-heat.png  heatmap (noir = bloqué, blanc = libre)
  - assets/map/collision-overlay.png  (debug) heatmap teintée sur level-render.png

Encodage des flags constaté sur ces fichiers (release digitale v1.12):
  0x0001  sol/mur décoratif marchable (herbe, routes, petits props)
  0x1001  bloquant (murs, clôtures, troncs d'arbres, warp)   -> bit 12
  0x2005  bloquant (rochers, buissons denses, berges)        -> bits 13+2
  => tuile sol bloquée si flags & (0x1000|0x2000|0x0004) sur la majorité
     de ses sous-blocs; mur (kind='wall') => cellule bloquée si présent.
"""
import struct, os, json, collections
from PIL import Image

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAP = os.path.join(BASE, 'assets', 'map')

BLOCK_MASK = 0x1000 | 0x2000 | 0x0004   # bits observés bloquants
FLOOR_NSUB = 25                          # sol = 25 sous-blocs 32x8 (mur = autre)


# ------------------------------------------------------------------ formats
class Tile:
    __slots__ = ('kind', 'blocks')

    def __init__(self):
        self.kind = 'floor'
        self.blocks = []          # (a, b, flags, payload)


class ContainerDT1:
    """Conteneur tuiles release digitale: entête 268 o, table @252 stride 96."""
    def __init__(self, path):
        d = self.d = open(path, 'rb').read()
        v1, v2 = struct.unpack_from('<ii', d, 0)
        assert v1 == 7 and v2 == 6, (path, v1, v2)
        self.nbtiles = struct.unpack_from('<i', d, 268)[0]
        self.tiles = []
        for k in range(self.nbtiles):
            dataoff, datalen, nsub = struct.unpack_from('<3i', d, 252 + 96 * k)
            t = Tile()
            t.kind = 'floor' if nsub == FLOOR_NSUB else 'wall'
            if datalen:
                for j in range(nsub):
                    h = d[dataoff + 20 * j: dataoff + 20 * j + 20]
                    g = struct.unpack_from('<H', h, 8)[0]     # flags @8
                    t.blocks.append((g,))                      # positions inutiles ici
            self.tiles.append(t)


def tg1_to_container(tg1_path):
    base = tg1_path.replace('/', '\\').split('\\')[-1][:-4]
    folder = tg1_path.replace('/', '\\').split('\\')[-2]
    if base == 'objects':
        return 'town_objects.dt1' if folder == 'town' else 'out_objects.dt1'
    alias = {'floor': 'floor.dt1', 'fence': 'fence.dt1', 'stonewall': 'stonewall.dt1',
             'river': 'river.dt1', 'treegroups': 'treegroups.dt1', 'warp': 'warp.dt1'}
    return alias[base]


class DS1:
    """Layout v18: (w+1)x(h+1) cellules, 6 couches int32 après l'entête+chemins."""
    def __init__(self, path):
        d = open(path, 'rb').read()
        assert struct.unpack_from('<i', d, 0)[0] == 18
        w, h = struct.unpack_from('<ii', d, 4)
        self.w, self.h = w + 1, h + 1
        filecount = struct.unpack_from('<i', d, 20)[0]
        p = 24
        self.files = []
        for _ in range(filecount):
            e = d.index(b'\x00', p)
            self.files.append(d[p:e].decode()); p = e + 1
        p += 8                                    # meta (2 ints)
        n = self.w * self.h
        self.layers = [struct.unpack_from('<%di' % n, d, p + 4 * n * i)
                       for i in range(6)]


def cell_fields(v):
    return (v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF)


# ------------------------------------------------------------------ extraction
def main():
    ds1 = DS1(os.path.join(MAP, 'TownN1.ds1'))
    tilesets = []
    for f in ds1.files:
        c = ContainerDT1(os.path.join(MAP, tg1_to_container(f)))
        tilesets.extend(c.tiles)

    W, H = ds1.w, ds1.h
    grid = [[True] * W for _ in range(H)]        # True = bloqué par défaut
    stats = collections.Counter()

    for cy in range(H):
        for cx in range(W):
            floor_ok = False
            floor_blocked = 0
            floor_total = 0
            wall_hit = False
            for li in (4, 0, 2, 5, 1):           # sol, rues, rares, objets
                v = ds1.layers[li][cy * W + cx]
                p1, p2, p3, orient = cell_fields(v)
                if p1 == 0 or (orient & 0x80):
                    continue
                idx = p1 + (p2 & 0x0F)
                if idx >= len(tilesets):
                    stats['oob'] += 1
                    continue
                tile = tilesets[idx]
                if tile.kind == 'wall':
                    wall_hit = True
                else:
                    floor_ok = True
                    for (g,) in tile.blocks:
                        floor_total += 1
                        if g & BLOCK_MASK:
                            floor_blocked += 1
            blocked = wall_hit or (not floor_ok)
            if floor_ok and not wall_hit and floor_total:
                blocked = floor_blocked * 2 >= floor_total   # >= 50% sous-blocs bloquants
            grid[cy][cx] = bool(blocked)
            stats['bloqué' if blocked else 'libre'] += 1
            if wall_hit:
                stats['mur'] += 1
            if not floor_ok:
                stats['sans_sol'] += 1

    # ---- collision.json (contrat: {width, height, rows}, true = BLOQUÉ)
    out = {'width': W, 'height': H, 'rows': grid}
    with open(os.path.join(MAP, 'collision.json'), 'w') as f:
        json.dump(out, f, separators=(',', ':'))

    # ---- heatmap noir=bloqué / blanc=libre
    sc = 8
    img = Image.new('L', (W * sc, H * sc))
    for cy in range(H):
        for cx in range(W):
            v = 0 if grid[cy][cx] else 255
            for dy in range(sc):
                for dx in range(sc):
                    img.putpixel((cx * sc + dx, cy * sc + dy), v)
    img.save(os.path.join(MAP, 'collision-heat.png'))

    # ---- overlay debug sur le rendu validé
    try:
        render = Image.open(os.path.join(MAP, 'level-render.png')).convert('RGB')
        ov = render.copy()
        cell_w = render.width // W
        cell_h = render.height // H
        px = ov.load()
        for cy in range(H):
            for cx in range(W):
                tint = (255, 40, 40) if grid[cy][cx] else (40, 255, 40)
                for yy in range(cy * cell_h, (cy + 1) * cell_h):
                    for xx in range(cx * cell_w, (cx + 1) * cell_w):
                        r, g, b = px[xx, yy]
                        px[xx, yy] = ((r + 2 * tint[0]) // 3,
                                      (g + 2 * tint[1]) // 3,
                                      (b + 2 * tint[2]) // 3)
        ov.save(os.path.join(MAP, 'collision-overlay.png'))
    except Exception as e:
        print('overlay: skip', e)

    print(f'grille {W}x{H}:', dict(stats))
    print('libre:', sum(r.count(False) for r in grid), '/', W * H)
    print('écrit: collision.json, collision-heat.png, collision-overlay.png')


if __name__ == '__main__':
    main()
