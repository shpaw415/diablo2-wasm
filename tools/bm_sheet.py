#!/usr/bin/env python3
"""bm_sheet.py — Planche contact des presets ACT1\Outdoors (Blood Moor chunks).

Rend chaque preset 8x8 (v18) avec le pipeline existant (map_proto2 + palette PL2),
et assemble une planche étiquetée pour choisir la composition de la Blood Moor.
"""
import os, sys, struct
from PIL import Image, ImageDraw

BASE = '/home/agent/workspace/diablo-2-wasm'
sys.path.insert(0, os.path.join(BASE, 'tools'))
MAP = os.path.join(BASE, 'assets', 'map')
PRESETS = '/tmp/opencode/bm_ds1'
DT1SRC = {'cliff1': '/tmp/opencode/bm_dt1/cliff1.dt1',
          'cliff2': '/tmp/opencode/bm_dt1/cliff2.dt1',
          'corner': '/tmp/opencode/bm_dt1/corner.dt1'}

from map_proto2 import DS1, ContainerDT1, Renderer

ALIAS = {'floor': 'floor.dt1', 'fence': 'fence.dt1', 'stonewall': 'out_stonewall.dt1',
         'river': 'out_river.dt1', 'treegroups': 'out_treegroups.dt1', 'warp': 'warp.dt1',
         'stones': 'out_stones.dt1', 'bridge': 'out_bridge.dt1',
         'cliff1': 'cliff1.dt1', 'cliff2': 'cliff2.dt1', 'corner': 'corner.dt1'}


def tg1_to_container(tg1_path):
    parts = tg1_path.replace('/', '\\').split('\\')
    base = parts[-1][:-4].lower()
    return ALIAS[base]


def load_palette():
    d = open(os.path.join(MAP, 'pal_act1_pl2.dat'), 'rb').read()
    return list(d[:768])


def main():
    pal = load_palette()
    # cache DT1 containers
    cache = {}

    def get_dt1(fn):
        if fn not in cache:
            p = DT1SRC.get(fn[:-4], os.path.join(MAP, fn))
            cache[fn] = ContainerDT1(p)
        return cache[fn]

    rows = []
    names = sorted(os.listdir(PRESETS))
    cols = 6
    TH = 240
    cell_w, cell_h = 340, TH + 18
    sheet = Image.new('RGB', (cols * cell_w, ((len(names) + cols - 1) // cols) * cell_h), (12, 12, 12))
    dr = ImageDraw.Draw(sheet)
    for i, nm in enumerate(names):
        try:
            ds1 = DS1(os.path.join(PRESETS, nm))
            tilesets = []
            for f in ds1.files:
                tilesets.extend(get_dt1(tg1_to_container(f)).tiles)
            r = Renderer(ds1, tilesets, pal, wall_head=60)
            r.draw_layer(4); r.draw_layer(0); r.draw_layer(2); r.draw_layer(5); r.draw_layer(1)
            img = r.img.convert('RGB')
            # crop utile
            img = img.crop((40, 40, img.width - 40, img.height - 40))
            s = min(cell_w / img.width, TH / img.height)
            img = img.resize((max(1, int(img.width * s)), max(1, int(img.height * s))))
            x = (i % cols) * cell_w
            y = (i // cols) * cell_h
            sheet.paste(img, (x + (cell_w - img.width) // 2, y + 16))
            dr.text((x + 6, y + 2), f'{nm} {ds1.w}x{ds1.h} f{len(ds1.files)}', fill=(255, 255, 120))
        except Exception as e:
            print(nm, 'ERR', e)
    out = '/tmp/opencode/bm_sheet.png'
    sheet.save(out)
    print('saved', out, sheet.size)


if __name__ == '__main__':
    main()
