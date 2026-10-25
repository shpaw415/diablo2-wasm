#!/usr/bin/env python3
"""extract_ui_fix.py — Extraction des sprites UI RÉELS de D2 v1.12 (d2w-fix-ui).

Sonde par nom exact via StormLib (priorité patch_d2 > d2exp > d2data > d2char),
décodage DC6 via tools/dc6_mini.py, palette `sky` (référence OpenDiablo2 :
tous les éléments du HUD de jeu utilisent data/global/palette/sky/pal.dat),
rendu PNG RGBA dans demo/assets/ui/ (+ manifest ui-manifest.json).

Usage : python3 tools/extract_ui_fix.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from stormlib_read import read_file  # noqa: E402
from dc6_mini import decode_dc6, load_palette  # noqa: E402
from PIL import Image  # noqa: E402

OUT = os.path.join(ROOT, 'demo', 'assets', 'ui')
ARCHIVES = [os.path.join(ROOT, p) for p in
            ('engine/patch_d2.mpq', 'data/d2exp.mpq', 'data/d2data.mpq', 'data/d2char.mpq')]

# (nom logique, chemin MPQ, frames attendues [None = tout garder])
UI_DC6 = {
    # --- barre de contrôle du bas (800x600) ---
    'panel':        'data\\global\\ui\\PANEL\\800ctrlpnl7.dc6',
    'overlap':      'data\\global\\ui\\PANEL\\overlap.dc6',
    'orbs':         'data\\global\\ui\\PANEL\\hlthmana.dc6',
    'addskill':     'data\\global\\ui\\PANEL\\level.dc6',
    'runbutton':    'data\\global\\ui\\PANEL\\runbutton.dc6',
    'menubutton':   'data\\global\\ui\\PANEL\\menubutton.dc6',
    'minipanel':    'data\\global\\ui\\PANEL\\minipanel.dc6',
    'minipanel_s':  'data\\global\\ui\\PANEL\\minipanel_s.dc6',
    'minipanelbtn': 'data\\global\\ui\\PANEL\\minipanelbtn.dc6',
    'goldcoinbtn':  'data\\global\\ui\\panel\\goldcoinbtn.dc6',
    # --- panneau inventaire (droite 400x600) ---
    'invchar6':     'data\\global\\ui\\PANEL\\invchar6.dc6',
    'invchar6tab':  'data\\global\\ui\\PANEL\\invchar6tab.dc6',
    'inv_armor':    'data\\global\\ui\\PANEL\\inv_armor.dc6',
    'inv_belt':     'data\\global\\ui\\PANEL\\inv_belt.dc6',
    'inv_boots':    'data\\global\\ui\\PANEL\\inv_boots.dc6',
    'inv_helm':     'data\\global\\ui\\PANEL\\inv_helm_glove.dc6',
    'inv_ring':     'data\\global\\ui\\PANEL\\inv_ring_amulet.dc6',
    'inv_weapons':  'data\\global\\ui\\PANEL\\inv_weapons.dc6',
    # --- arbre de compétences (droite 400x600) + icônes ---
    'sktree_ba':    'data\\global\\ui\\SPELLS\\skltree_b_back.dc6',
    'sktree_so':    'data\\global\\ui\\SPELLS\\skltree_s_back.dc6',
    'sktree_pa':    'data\\global\\ui\\SPELLS\\skltree_p_back.dc6',
    'sktree_am':    'data\\global\\ui\\SPELLS\\skltree_a_back.dc6',
    'sktree_as':    'data\\global\\ui\\SPELLS\\skltree_i_back.dc6',
    'sktree_dz':    'data\\global\\ui\\SPELLS\\skltree_d_back.dc6',
    'sktree_ne':    'data\\global\\ui\\SPELLS\\skltree_n_back.dc6',
    'skillicon':    'data\\global\\ui\\SPELLS\\skillicon.dc6',
    'skillicon_ba': 'data\\global\\ui\\SPELLS\\baskillicon.dc6',
    'skillicon_so': 'data\\global\\ui\\SPELLS\\soskillicon.dc6',
    'skillicon_pa': 'data\\global\\ui\\SPELLS\\paskillicon.dc6',
    'skillicon_am': 'data\\global\\ui\\SPELLS\\amskillicon.dc6',
    'skillicon_as': 'data\\global\\ui\\SPELLS\\asskillicon.dc6',
    'skillicon_dz': 'data\\global\\ui\\SPELLS\\drskillicon.dc6',
}
# Objets (inventaire/ceinture) : data\global\items\<invfile>.dc6 — palette units.
# invfile vérifiés par hash MPQ (digital v1.12) : potions hp1-5/mp1-5, vps, yps,
# key, goldpile, invpor (parchemin de ville), invscr (parchemin d'identification),
# invbok (livre/tome). rvs/rvl/wms absents du digital release — repli bloc coloré.
ITEMS = {'invhp1': 'hp1', 'invhp2': 'hp2', 'invhp3': 'hp3', 'invhp4': 'hp4', 'invhp5': 'hp5',
         'invmp1': 'mp1', 'invmp2': 'mp2', 'invmp3': 'mp3', 'invmp4': 'mp4', 'invmp5': 'mp5',
         'invvps': 'vps', 'invyps': 'yps', 'invkey': 'key',
         'invpor': 'tsc', 'invscr': 'isc', 'invbok': 'tbk', 'invbok': 'ibk',
         'goldpile': 'gold'}
# Tables excel utiles au placement (grille inventaire par classe)
TABLES = {
    'inventory.txt': 'data\\global\\excel\\inventory.txt',
    'belts.txt': 'data\\global\\excel\\belts.txt',
}


def probe(name):
    for p in ARCHIVES:
        if not os.path.exists(p):
            continue
        try:
            data = read_file(p, name)
        except OSError:
            continue
        if data:
            return data, os.path.basename(p)
    return None, None


def save_frames(key, data, pal, manifest):
    frames = decode_dc6(data)
    entries = []
    for i, f in enumerate(frames):
        img = Image.new('RGBA', (f['width'], f['height']), (0, 0, 0, 0))
        px = img.load()
        for y in range(f['height']):
            row = f['pixels'][y * f['width']:(y + 1) * f['width']]
            for x, idx in enumerate(row):
                if idx:
                    r, g, b = pal[idx]
                    px[x, y] = (r, g, b, 255)
        fname = f'{key}.png' if len(frames) == 1 else f'{key}_f{i}.png'
        img.save(os.path.join(OUT, fname))
        entries.append({'file': fname, 'w': f['width'], 'h': f['height'],
                        'offX': f['offsetX'], 'offY': f['offsetY']})
    manifest[key] = entries
    sizes = ', '.join(f"{e['w']}x{e['h']}" for e in entries[:8])
    print(f'OK  {key:14s} {len(frames):2d} frame(s) [{sizes}]')


def main():
    os.makedirs(OUT, exist_ok=True)
    manifest = {}
    # palettes
    for pname, path in [('sky', 'data\\global\\palette\\sky\\pal.dat'),
                        ('units', 'data\\global\\palette\\units\\pal.dat')]:
        data, src = probe(path)
        if data:
            open(os.path.join(OUT, f'pal_{pname}.dat'), 'wb').write(data)
            print(f'OK  pal_{pname} ({len(data):,} o, {src})')
        else:
            print(f'!!  palette {pname} introuvable')
    pal_sky = load_palette(os.path.join(OUT, 'pal_sky.dat')) if os.path.exists(
        os.path.join(OUT, 'pal_sky.dat')) else None
    pal_units = load_palette(os.path.join(OUT, 'pal_units.dat')) if os.path.exists(
        os.path.join(OUT, 'pal_units.dat')) else None
    if not pal_sky:
        print('!! pas de palette sky — abandon'); return 1

    for key, path in UI_DC6.items():
        data, src = probe(path)
        if not data:
            print(f'!!  {key:14s} ABSENT ({path})')
            continue
        save_frames(key, data, pal_sky, manifest)

    for invfile, code in ITEMS.items():
        data, src = probe(f'data\\global\\items\\{invfile}.dc6')
        if not data:
            print(f'!!  item {code} ({invfile}) ABSENT')
            continue
        # palette units pour les objets (référence OD2/jeu : icônes d'objets units)
        save_frames(f'item_{code}', data, pal_units or pal_sky, manifest)

    for dest, path in TABLES.items():
        data, src = probe(path)
        if data:
            open(os.path.join(OUT, dest), 'wb').write(data)
            print(f'OK  {dest} ({len(data):,} o, {src})')

    json.dump(manifest, open(os.path.join(OUT, 'ui-manifest.json'), 'w'), indent=1)
    print(f'\n→ {len(manifest)} sprites, manifest demo/assets/ui/ui-manifest.json')
    return 0


if __name__ == '__main__':
    sys.exit(main())
