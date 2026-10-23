#!/usr/bin/env python3
"""map_proto2.py — Prototype rendu de carte Diablo 2 (TownN1.ds1, acte I).

Décode le conteneur "DT1" de la release digitale 2008 (v1.12) + le DS1 layout
version 18, et compose la carte isométrique en PNG (palette act1).

Découvertes clés (preuves dans evidence/MAP-SPEC.md):
- Conteneur tile (pas de signature C7AB0B00 dans cette release !):
    @0:  int32 (7, 6) ; @268: int32 nbTiles ; table @252, stride 96 ;
    blob tuile = [nsub × entête 20 o][données]
    entête sous-bloc 20 o: (a, b: int16 pos px), (H, L: int8 coords),
    g: uint16 flags, ln: uint32 longueur, off: uint32 offset relatif blob
  SOL (nsub=25): blocs 32x8 BRUTS (256 o), placement (a,b)=(64+16(L-H), 8(H+L))
  MUR: blocs 32xN RLE-lignes: (t, o, o octets), t+o<=32, reste transparent,
       (0,0)=ligne vide, 0x00 finaux = padding. Placement (a,b), b négatif = haut.
- DS1 v18: version@0, w=56@4, h=40@8 (réel 57x41 = (w+1)(h+1)), 2 ints 0,
  fileCount@20, 8 chemins .tg1, meta(2,1), 6 couches de 57*41 int32, section PATH.
  Couches: L4 = sol principal, L0 = sol overlay (rues), L1 = arbres/objets,
  L2/L5 = cellules rares (pierres, warp).
"""
import struct, os, sys
from PIL import Image

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAP = os.path.join(BASE, 'assets', 'map')

# ---------------------------------------------------------------- palette
def load_palette(gamma=0.55):
    d = open(os.path.join(MAP, 'pal_act1.dat'), 'rb').read()
    d = (d + b'\x00' * 768)[:768]
    # Pas de PL2 dans cette release: normalisation gamma pour la lisibilité.
    return [int(255 * (v / 255) ** gamma) for v in d]

# ---------------------------------------------------------------- conteneur DT1
class Tile:
    __slots__ = ('kind', 'blocks')
    def __init__(self):
        self.kind = 'floor'   # floor | wall
        self.blocks = []      # (a, b, flags, payload)

def decode_wall_block(data):
    """RLE-lignes: (t, o, o octets) par ligne de 32; rend 32xN bytes."""
    out = bytearray()
    j = 0
    n = len(data)
    while j + 1 < n:
        t, o = data[j], data[j + 1]
        j += 2
        if t >= 32 and o == 0:
            t &= 0x1F  # sécurité
        out += b'\x00' * t
        if j + o > n:  # tronqué (padding)
            out += data[j:n]; j = n
            break
        out += data[j:j + o]; j += o
    return bytes(out)

class ContainerDT1:
    """Conteneur de tuiles de la release digitale (ex-'floor.dt1')."""
    def __init__(self, path):
        d = self.d = open(path, 'rb').read()
        v1, v2 = struct.unpack_from('<ii', d, 0)
        assert v1 == 7 and v2 == 6, (path, v1, v2)
        self.nbtiles = struct.unpack_from('<i', d, 268)[0]
        self.tiles = []
        for k in range(self.nbtiles):
            r = 252 + 96 * k
            dataoff, datalen, nsub = struct.unpack_from('<3i', d, r)
            t = Tile()
            t.kind = 'floor' if nsub == 25 else 'wall'
            if datalen:
                for j in range(nsub):
                    h = d[dataoff + 20*j: dataoff + 20*j + 20]
                    a, b = struct.unpack_from('<hh', h, 0)
                    g, ln = struct.unpack_from('<HI', h, 8)
                    off = struct.unpack_from('<I', h, 16)[0]
                    data = d[dataoff + off: dataoff + off + ln]
                    if t.kind == 'wall':
                        data = decode_wall_block(data)
                    t.blocks.append((a, b, g, data))
            self.tiles.append(t)

def tg1_to_container(tg1_path):
    base = tg1_path.replace('/', '\\').split('\\')[-1][:-4]
    folder = tg1_path.replace('/', '\\').split('\\')[-2]
    if base == 'objects':
        return 'town_objects.dt1' if folder == 'town' else 'out_objects.dt1'
    alias = {'floor': 'floor.dt1', 'fence': 'fence.dt1', 'stonewall': 'stonewall.dt1',
             'river': 'river.dt1', 'treegroups': 'treegroups.dt1', 'warp': 'warp.dt1'}
    return alias[base]

# ---------------------------------------------------------------- DS1 v18
class DS1:
    def __init__(self, path):
        d = open(path, 'rb').read()
        self.version = struct.unpack_from('<i', d, 0)[0]
        assert self.version == 18, self.version
        self.w_stored, self.h_stored = struct.unpack_from('<ii', d, 4)
        self.w, self.h = self.w_stored + 1, self.h_stored + 1   # piège v18
        filecount = struct.unpack_from('<i', d, 20)[0]
        p = 24
        self.files = []
        for _ in range(filecount):
            e = d.index(b'\x00', p)
            self.files.append(d[p:e].decode()); p = e + 1
        self.meta = struct.unpack_from('<2i', d, p); p += 8
        n = self.w * self.h
        self.layers = []
        for _ in range(6):
            self.layers.append(struct.unpack_from('<%di' % n, d, p)); p += 4 * n
        self.path_off = p * 4 + 392 - 8 * 0  # section PATH (phase 2)

def cell_fields(v):
    return (v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF)

# ---------------------------------------------------------------- rendu
class Renderer:
    def __init__(self, ds1, tilesets, pal, wall_head=420):
        self.ds1, self.tilesets, self.pal = ds1, tilesets, pal
        W, H = ds1.w, ds1.h
        self.CW, self.CH = 80, 40
        self.ox, self.oy = H * self.CW, wall_head
        self.img_w = (W + H) * self.CW
        self.img_h = (W + H) * self.CH + wall_head + 80
        self.img = Image.new('P', (self.img_w, self.img_h), 0)
        self.img.putpalette(pal)
        self.px = self.img.load()
        self.stats = {'drawn': 0, 'hidden': 0, 'oob': 0, 'empty': 0}

    def draw_floor_block(self, dst_x, dst_y, data):
        px, iw, ih = self.px, self.img_w, self.img_h
        for i in range(len(data)):
            v = data[i]
            if v == 0:
                continue
            x = dst_x + (i & 31); y = dst_y + (i >> 5)
            if 0 <= x < iw and 0 <= y < ih:
                px[x, y] = v

    def draw_wall_block(self, dst_x, dst_y, data):
        px, iw, ih = self.px, self.img_w, self.img_h
        rows = len(data) // 32
        for r in range(rows):
            base = r * 32
            row = data[base:base + 32]
            if row == b'\x00' * 32:
                continue
            y = dst_y + r
            for i in range(32):
                v = row[i]
                if v:
                    x = dst_x + i
                    if 0 <= x < iw and 0 <= y < ih:
                        px[x, y] = v

    def cell_index(self, p1, p2):
        """Index de tuile: p1 + variante p2&0x0F (familles de tuiles consécutives)."""
        return p1, p1 + (p2 & 0x0F)

    def draw_layer(self, li, use_variant=True):
        ds1 = self.ds1
        W, H = ds1.w, ds1.h
        cells = ds1.layers[li]
        for cy in range(H):
            for cx in range(W):
                v = cells[cy * W + cx]
                p1, p2, p3, orient = cell_fields(v)
                if p1 == 0:
                    self.stats['empty'] += 1; continue
                if orient & 0x80:
                    self.stats['hidden'] += 1; continue
                base, idx = self.cell_index(p1, p2)
                if idx >= len(self.tilesets) or base >= len(self.tilesets):
                    self.stats['oob'] += 1; continue
                tile = self.tilesets[idx]
                if not tile.blocks:
                    tile = self.tilesets[base]
                sx = (cx - cy) * self.CW + self.ox
                sy = (cx + cy) * self.CH + self.oy
                draw = self.draw_wall_block if tile.kind == 'wall' else self.draw_floor_block
                for (a, b, g, data) in tile.blocks:
                    draw(sx + a, sy + b, data)
                self.stats['drawn'] += 1

    def save(self, path, scale=2):
        img = self.img
        if scale != 1:
            img = img.resize((self.img_w // scale, self.img_h // scale), Image.NEAREST)
        img.convert('RGB').save(path)
        print("stats:", self.stats)
        print("saved", path, img.size)

# ---------------------------------------------------------------- main
def main():
    pal = load_palette()
    ds1 = DS1(os.path.join(MAP, 'TownN1.ds1'))
    print(f"DS1 v{ds1.version}: {ds1.w}x{ds1.h} meta={ds1.meta} files={len(ds1.files)}")
    tilesets = []
    for f in ds1.files:
        fn = tg1_to_container(f)
        c = ContainerDT1(os.path.join(MAP, fn))
        print(f"  {f} -> {fn}: {c.nbtiles} tuiles")
        tilesets.extend(c.tiles)
    print("cumul:", len(tilesets))
    # Ordre de peinture iso: sol principal, rues, puis objets/arbres par ligne.
    r = Renderer(ds1, tilesets, pal)
    r.draw_layer(4)          # sol principal (herbe)
    r.draw_layer(0)          # rues / chemins
    r.draw_layer(2)          # cellules rares (pierres?)
    r.draw_layer(5)          # cellules rares (warp?)
    r.draw_layer(1)          # arbres / objets hauts
    out = os.path.join(MAP, 'level-render.png')
    r.save(out, scale=2)

if __name__ == '__main__':
    main()
