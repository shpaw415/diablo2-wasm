#!/usr/bin/env python3
"""bloodmoor_build.py — Blood Moor (acte I, Id 2) : composition type-DRLG simplifiée.

La Blood Moor est un niveau ALÉATOIRE (levels.txt : DrlgType=3, LevelType=2) :
le D2 retail la génère par drlg outdoor (grille de chunks + presets posés par
D2Common). Les presets réels existent dans les MPQ : data\\global\\tiles\\ACT1\\Outdoors\\*.ds1
(d2exp.mpq, v1.12) — chunks v18 9x9 [tag2: mur1..4+sol+ombre ; tag3: mur1..6+sol+ombre].

Approche (documentée dans evidence/MAP-SPEC-BLOODMOOR.md) :
  1. pool de presets complets (tag2/tag3) extraits de d2exp/d2data ;
  2. canevas 72x72 cellules (8x8 chunks) ;
  3. sol = pool des cellules de sol réelles des chunks (waypoint = plaine pure) ;
  4. anneau de bordure forestière (chunks bord* posés sur le pourtour) ;
  5. features intérieures : falaise stnclf6b, clairières bord6-9 (chemins en terre),
     rivière Lriver/Uriver, chunk waypoint ;
  6. dispersion déterministe d'arbres/pierres (cellules réelles transposées) ;
  7. réécriture en DS1 v18 binaire (assets/map/bloodmoor.ds1, liste DT1 unifiée
     embarquée, remap des indices cumulatifs par fichier) ;
  8. rendu + collision relus depuis CE fichier (un seul chemin de code).

Déterministe : seed fixe. Sorties :
  assets/map/bloodmoor.ds1, bloodmoor-render.png, bloodmoor-collision.json,
  bloodmoor-collision-heat.png
"""
import os, sys, struct, json, random, shutil
from PIL import Image

BASE = '/home/agent/workspace/diablo-2-wasm'
MAP = os.path.join(BASE, 'assets', 'map')
PRESET_DIR = os.path.join(MAP, 'presets_bloodmoor')
sys.path.insert(0, os.path.join(BASE, 'tools'))
from map_proto2 import ContainerDT1

SEED = 20261022
GRID = 8                      # 8x8 chunks
CH = 9                        # cellules par chunk (v18 : stocké 8 -> réel 9)
W = H = GRID * CH             # 72x72 cellules

# ---- liste DT1 unifiée (ordre = espace d'indices cumulatif du DS1 écrit) ----
# (fichier local, chemin tg1 embarqué dans le DS1, nb tuiles lu à l'exécution)
UNIFIED = [
    ('out_treegroups.dt1',      '\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.dt1'),
    ('out_stonewall.dt1',       '\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.dt1'),
    ('floor.dt1',               '\\d2\\data\\global\\tiles\\act1\\town\\floor.dt1'),
    ('out_stones.dt1',          '\\d2\\data\\global\\tiles\\act1\\outdoors\\stones.dt1'),
    ('out_river.dt1',           '\\d2\\data\\global\\tiles\\act1\\outdoors\\river.dt1'),
    ('bloodmoor_cliff2.dt1',    '\\d2\\data\\global\\tiles\\act1\\outdoors\\cliff2.dt1'),
    ('bloodmoor_corner.dt1',    '\\d2\\data\\global\\tiles\\act1\\outdoors\\corner.dt1'),
    ('bloodmoor_cliff1.dt1',    '\\d2\\data\\global\\tiles\\act1\\outdoors\\cliff1.dt1'),
]

# presets sources (extraits de d2exp.mpq, cf. MANIFEST section bloodmoor)
BORDER = ['bord10', 'bord11', 'bord12', 'bord1b', 'bord1c', 'bord2', 'bord2b',
          'bord2c', 'bord3', 'bord3b', 'bord3c', 'bord4c', 'bord5']
CLEARING = ['bord6', 'bord7', 'bord8', 'bord9']
RIVER = ['LriverN', 'LriverS', 'UriverN', 'UriverS']
CLIFF = ['stnclf6b']
PLAIN = ['waypoint']

# ------------------------------------------------------------------ DS1 v18
def parse_ds1(path):
    d = open(path, 'rb').read()
    ver = struct.unpack_from('<i', d, 0)[0]
    assert ver == 18, ver
    w, h = struct.unpack_from('<ii', d, 4)
    W, H = w + 1, h + 1
    fc = struct.unpack_from('<i', d, 20)[0]
    p = 24
    files = []
    for _ in range(fc):
        e = d.index(b'\x00', p)
        files.append(d[p:e].decode())
        p = e + 1
    tag1, tag2 = struct.unpack_from('<ii', d, p)
    p += 8
    n = W * H
    nlayers = {1: 4, 2: 6, 3: 8}[tag1]
    layers = [struct.unpack_from('<%di' % n, d, p + 4 * n * i) for i in range(nlayers)]
    return {'W': W, 'H': H, 'tag': (tag1, tag2), 'files': files, 'layers': layers}


def cell_fields(v):
    return (v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF)


def make_cell(p1, p2, p3, orient):
    return p1 | (p2 << 8) | (p3 << 16) | (orient << 24)


# ------------------------------------------------------------------ composition
class Composer:
    def __init__(self):
        self.containers = {}
        self.counts = {}
        for fn, tg1 in UNIFIED:
            c = ContainerDT1(os.path.join(MAP, fn))
            self.containers[fn] = c
            self.counts[fn] = c.nbtiles
        self.ubase = {}
        tot = 0
        for fn, _ in UNIFIED:
            self.ubase[fn] = tot
            tot += self.counts[fn]
        self.total = tot
        # canvas tag2 : [mur1, mur2, mur3, mur4, sol, ombre]
        self.layers = [[0] * (W * H) for _ in range(6)]

    def tg1_file(self, tg1):
        base = tg1.replace('/', '\\').split('\\')[-1][:-4].lower()
        folder = tg1.replace('/', '\\').split('\\')[-2].lower()
        if base == 'floor':
            return 'floor.dt1'
        if base == 'objects':
            return 'town_objects.dt1' if folder == 'town' else 'out_objects.dt1'
        alias = {'treegroups': 'out_treegroups.dt1', 'stonewall': 'out_stonewall.dt1',
                 'stones': 'out_stones.dt1', 'river': 'out_river.dt1',
                 'cliff1': 'bloodmoor_cliff1.dt1', 'cliff2': 'bloodmoor_cliff2.dt1',
                 'corner': 'bloodmoor_corner.dt1', 'bridge': 'out_bridge.dt1'}
        return alias[base]

    def remap(self, preset_files, v, stats):
        """Remape une cellule du preset vers l'espace unifié. Retourne None si vide."""
        p1, p2, p3, orient = cell_fields(v)
        if p1 == 0:
            return None
        nib = p2 & 0x0F
        idx = p1 + nib
        # bases cumulatives du preset
        base = 0
        for tg1 in preset_files:
            fn = self.tg1_file(tg1)
            cnt = self.counts[fn]
            if idx < base + cnt:
                new_idx = self.ubase[fn] + (idx - base)
                new_p1 = new_idx - nib
                stats['remap'] += 1
                return make_cell(new_p1, p2, p3, orient)
            base += cnt
        stats['oob'] += 1
        return None

    def paste(self, name, cx0, cy0, stats):
        p = parse_ds1(os.path.join(PRESET_DIR, name + '.ds1'))
        tag1 = p['tag'][0]
        floor_i = {2: 4, 3: 6}[tag1]
        shadow_i = floor_i + 1
        wall_i = list(range(0, floor_i))
        pw, ph = p['W'], p['H']
        for cy in range(min(ph, H - cy0)):
            for cx in range(min(pw, W - cx0)):
                X, Y = cx0 + cx, cy0 + cy
                # sol : on écrase (le chunk apporte son sol réel)
                v = self.remap(p['files'], p['layers'][floor_i][cy * pw + cx], stats)
                if v is not None:
                    self.layers[4][Y * W + X] = v
                # ombre : copiée telle quelle (flags, pas d'indice tuile)
                self.layers[5][Y * W + X] = p['layers'][shadow_i][cy * pw + cx]
                # murs : ajout par dessus (cellules non vides uniquement)
                for wl, li in zip((1, 2, 3, 4, 0, 0), wall_i):
                    v = self.remap(p['files'], p['layers'][li][cy * pw + cx], stats)
                    if v is not None:
                        self.layers[wl][Y * W + X] = v

    def scatter_cell(self, layer_i, cellv, X, Y):
        if 0 <= X < W and 0 <= Y < H and cellv:
            self.layers[layer_i][Y * W + X] = cellv


def main():
    rng = random.Random(SEED)
    stats = {'remap': 0, 'oob': 0}
    comp = Composer()

    # ---- cellules pools (réelles, déjà en espace unifié après paste-like remap)
    def pool_from(names, layer_pick):
        """Extrait des cellules réelles (mur/sol) des chunks pour dispersion."""
        walls, floors = [], []
        for nm in names:
            p = parse_ds1(os.path.join(PRESET_DIR, nm + '.ds1'))
            tag1 = p['tag'][0]
            floor_i = {2: 4, 3: 6}[tag1]
            base = 0
            bases = []
            for tg1 in p['files']:
                fn = comp.tg1_file(tg1)
                bases.append((base, comp.counts[fn], fn))
                base += comp.counts[fn]
            def remap_cell(v):
                p1, p2, p3, orient = cell_fields(v)
                if p1 == 0:
                    return None
                nib = p2 & 0x0F
                idx = p1 + nib
                for b, cnt, fn in bases:
                    if idx < b + cnt:
                        return make_cell(comp.ubase[fn] + (idx - b) - nib, p2, p3, orient)
                return None
            for cy in range(p['H']):
                for cx in range(p['W']):
                    for li in range(0, floor_i):
                        c = remap_cell(p['layers'][li][cy * p['W'] + cx])
                        if c is not None:
                            walls.append((li, c))
                    c = remap_cell(p['layers'][floor_i][cy * p['W'] + cx])
                    if c is not None:
                        floors.append(c)
        return walls, floors

    wall_pool, floor_pool = pool_from(PLAIN + CLEARING + RIVER + BORDER + CLIFF, None)
    # sols de plaine : le waypoint = herbe pure ; on s'en sert de base
    plain_walls, plain_floors = pool_from(PLAIN, None)
    assert plain_floors, 'waypoint doit fournir du sol'
    # sol "pur" = uniquement cellules du fichier floor (herbe), pas stones/cliff
    def is_grass(c):
        p1 = c & 0xFF
        lo = comp.ubase['floor.dt1']
        return lo <= p1 < lo + comp.counts['floor.dt1']
    grass_pool = [c for c in plain_floors if is_grass(c)]
    allfloor_pool = [c for c in floor_pool if is_grass(c)]
    stone_pool = [c for c in floor_pool if not is_grass(c)]
    tree_pool = [(li, c) for (li, c) in wall_pool
                 if (c & 0xFF) < comp.ubase['out_stonewall.dt1']]      # treegroups
    rock_pool = [(li, c) for (li, c) in wall_pool
                 if comp.ubase['out_stonewall.dt1'] <= (c & 0xFF) < comp.ubase['floor.dt1']]
    print(f'pools: sol_grass={len(grass_pool)} sol_all={len(allfloor_pool)} '
          f'stones={len(stone_pool)} arbres={len(tree_pool)} rochers={len(rock_pool)}')

    # ---- 1. sol de base : herbe partout (pool réel de cellules d'herbe, brassage seedé)
    for Y in range(H):
        for X in range(W):
            comp.layers[4][Y * W + X] = allfloor_pool[rng.randrange(len(allfloor_pool))]

    # ---- 2. anneau de bordure forestière (chunks bord* sur le pourtour 8x8)
    ring = []
    for i in range(GRID):
        ring.append(BORDER[i % len(BORDER)])                       # N
        ring.append(BORDER[(i + 3) % len(BORDER)])                 # S
    for i in range(GRID - 2):
        ring.append(BORDER[(i + 5) % len(BORDER)])                 # W
        ring.append(BORDER[(i + 1) % len(BORDER)])                 # E
    pos = 0
    for i in range(GRID):
        comp.paste(ring[pos], i * CH, 0, stats); pos += 1            # N
        comp.paste(ring[pos], i * CH, (GRID - 1) * CH, stats); pos += 1  # S
    for j in range(1, GRID - 1):
        comp.paste(ring[pos], 0, j * CH, stats); pos += 1            # W
        comp.paste(ring[pos], (GRID - 1) * CH, j * CH, stats); pos += 1

    # ---- 3. features intérieures
    comp.paste('waypoint', 3 * CH, 3 * CH, stats)                    # le waypoint !
    comp.paste('stnclf6b', 1 * CH, 1 * CH, stats)                    # falaise NO
    comp.paste('stnclf6b', 5 * CH, 5 * CH, stats)                    # falaise SE
    comp.paste('LriverN', 4 * CH, 2 * CH, stats)                     # rivière (2x2)
    comp.paste('UriverN', 5 * CH, 2 * CH, stats)
    comp.paste('LriverS', 4 * CH, 3 * CH, stats)
    comp.paste('UriverS', 5 * CH, 3 * CH, stats)
    comp.paste('bord6', 2 * CH, 5 * CH, stats)                       # clairières/chemins
    comp.paste('bord7', 5 * CH, 1 * CH, stats)
    comp.paste('bord8', 1 * CH, 4 * CH, stats)
    comp.paste('bord9', 3 * CH, 1 * CH, stats)

    # ---- 4. dispersion déterministe (bosquets, rochers, buissons)
    for _ in range(420):
        X, Y = rng.randrange(1, W - 1), rng.randrange(1, H - 1)
        # plus dense près des bords (ceinture forestière irrégulière)
        d = min(X, Y, W - 1 - X, H - 1 - Y)
        density = 0.85 if d < CH else (0.30 if d < 2 * CH else 0.12)
        if rng.random() > density:
            continue
        li, c = tree_pool[rng.randrange(len(tree_pool))]
        comp.scatter_cell(li if li in (1, 2, 3, 4) else 1, c, X, Y)
    for _ in range(90):
        X, Y = rng.randrange(2, W - 2), rng.randrange(2, H - 2)
        li, c = rock_pool[rng.randrange(len(rock_pool))]
        comp.scatter_cell(li if li in (1, 2, 3, 4) else 1, c, X, Y)
    for _ in range(50):                                              # taches de pierres au sol
        X, Y = rng.randrange(2, W - 2), rng.randrange(2, H - 2)
        comp.scatter_cell(4, stone_pool[rng.randrange(len(stone_pool))], X, Y)

    # ---- 5. écriture DS1 v18 (tag 2,1 comme TownN1 ; section PATH absente)
    files_bytes = b''
    for fn, tg1 in UNIFIED:
        files_bytes += tg1.encode('latin-1') + b'\x00'
    fc = len(UNIFIED)
    out = bytearray()
    out += struct.pack('<iiiiii', 18, W - 1, H - 1, 0, 0, fc)
    out += files_bytes
    out += struct.pack('<ii', 2, 1)
    for lay in comp.layers:
        # int32 signés : orient>=0x80 -> bit de poids fort -> négatif
        out += struct.pack('<%dI' % (W * H), *lay)
    ds1_path = os.path.join(MAP, 'bloodmoor.ds1')
    open(ds1_path, 'wb').write(bytes(out))
    print(f'écrit {ds1_path} ({len(out):,} o), remap={stats["remap"]} oob={stats["oob"]}')
    assert stats['oob'] == 0, 'cellules hors bornes au remap !'

    # ---- 6. relecture (round-trip) et vérifications
    chk = parse_ds1(ds1_path)
    assert chk['W'] == W and chk['H'] == H and chk['tag'] == (2, 1)
    floor_l = chk['layers'][4]
    empty = sum(1 for v in floor_l if (v & 0xFF) == 0)
    bad = 0
    for li in range(5):                       # murs + sol : indice de tuile valide
        for v in chk['layers'][li]:
            p1, p2 = v & 0xFF, (v >> 8) & 0xFF
            if p1 and p1 + (p2 & 0x0F) >= comp.total:
                bad += 1
    print(f'round-trip: {chk["W"]}x{chk["H"]} tag={chk["tag"]} sol vide={empty}/{W*H} '
          f'idx hors bornes={bad}')
    assert empty == 0 and bad == 0


if __name__ == '__main__':
    main()
