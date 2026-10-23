#!/usr/bin/env python3
"""heroes_select_bg.py — fond + poses officiels de l'écran de sélection D2 (FrontEnd).

Extrait via StormLib (sonde par nom exact, listfile OpenDiablo2 d2resource) :
  - data\\global\\ui\\FrontEnd\\charactercreationscreenEXP.dc6  (fond 800x600)
  - data\\global\\ui\\FrontEnd\\<dir>\\<XX>NU1/NU2/NU3.DC6      (repos/survol/sélectionné)
Rend (palette loading PL2 @0, 6 bits — voir assets/menu/README.md) :
  - demo/assets/menu/select-bg.png
  - demo/assets/menu/sel/<XX>-nu1.png / -nu2.png / -nu3.png (sheet horizontale si N frames)
  - demo/assets/menu/heroes.json v2 : {CODE:{file,frames,w,h,fps,nu1:{...},nu2:{...},nu3:{...}}}
"""
import json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__)))
from stormlib_read import read_file
from dc6_mini import decode_dc6

MENU = os.path.join(os.path.dirname(__file__), '..', 'demo', 'assets', 'menu')
ARCHIVES = ['engine/patch_d2.mpq', 'data/d2exp.mpq', 'data/d2data.mpq', 'data/d2char.mpq']
CLASSES = {  # code manifest -> (dir FrontEnd, token)
    'AM': ('amazon', 'AM'), 'AS': ('assassin', 'AS'), 'BA': ('barbarian', 'BA'),
    'DZ': ('druid', 'DZ'), 'NE': ('necromancer', 'NE'), 'PA': ('paladin', 'PA'),
    'SO': ('sorceress', 'SO'),
}

def probe(name):
    for mpq in ARCHIVES:
        p = os.path.join(os.path.dirname(__file__), '..', mpq)
        if not os.path.exists(p):
            continue
        try:
            data = read_file(p, name)
        except OSError:
            continue
        if data:
            return data, mpq
    return None, None

def load_pl2_loading(path):
    """palette loading PL2 @0 : 256 entrées RGBA 6 bits, stride 4, alpha=0."""
    data = open(path, 'rb').read()
    pal = []
    for i in range(256):
        r, g, b = data[i*4], data[i*4+1], data[i*4+2]
        pal.append((min(255, r * 255 // 252), min(255, g * 255 // 252), min(255, b * 255 // 252)))
    return pal

def render_frames(frames, pal, path, scale=1):
    """Sheet horizontale des frames (taille max), fond transparent."""
    if not frames:
        return None
    w = max(f['width'] for f in frames) * scale
    h = max(f['height'] for f in frames) * scale
    from PIL import Image
    img = Image.new('RGBA', (w * len(frames), h), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        fi = Image.new('RGBA', (f['width'], f['height']), (0, 0, 0, 0))
        px = fi.load()
        for y in range(f['height']):
            row = f['pixels'][y*f['width']:(y+1)*f['width']]
            for x, idx in enumerate(row):
                if idx:
                    r, g, b = pal[idx]
                    px[x, y] = (r, g, b, 255)
        if scale != 1:
            fi = fi.resize((f['width']*scale, f['height']*scale), Image.NEAREST)
        img.paste(fi, (i*w, h - fi.height))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.save(path)
    return {'file': os.path.relpath(path, MENU).replace('\\', '/'), 'frames': len(frames), 'w': w, 'h': h, 'fps': 8}

def render_bg(frames, pal, path, out_w=800, out_h=600):
    """Les fonds FrontEnd/CharSelect sont des rangées implicites : les tuiles
    se posent séquentiellement (x += w), nouvelle rangée quand on dépasse la
    largeur (ex. CharSelect : 3×256+32 = 800, hauteurs 256+256+88 = 600)."""
    from PIL import Image
    img = Image.new('RGBA', (out_w, out_h), (0, 0, 0, 255))
    x = y = row_h = 0
    for f in frames:
        if x + f['width'] > out_w:      # nouvelle rangée
            x = 0
            y += row_h
            row_h = 0
        fi = Image.new('RGBA', (f['width'], f['height']), (0, 0, 0, 0))
        px = fi.load()
        for yy in range(f['height']):
            row = f['pixels'][yy*f['width']:(yy+1)*f['width']]
            for xx, idx in enumerate(row):
                if idx:
                    r, g, b = pal[idx]
                    px[xx, yy] = (r, g, b, 255)
        img.paste(fi, (x, y))
        x += f['width']
        row_h = max(row_h, f['height'])
    img.save(path)
    return os.path.relpath(path, MENU).replace('\\', '/')

def main():
    fechar = read_file('data/d2data.mpq', r'data\global\palette\fechar\pal.dat')
    fechar = (fechar + b'\0'*768)[:768]
    # palette fechar = vraie palette des écrans FrontEnd ; le DAT D2 est stocké BGR
    pal = [(fechar[i*3+2], fechar[i*3+1], fechar[i*3]) for i in range(256)]
    out = {}

    # --- fond ---
    # le VRAI fond de l'écran de sélection : CharSelect\characterselectscreenEXP
    # (palette loading PL2 vérifiée visuellement — cadre doré, sol, ciel d'orage).
    # FrontEnd\charactercreationscreenEXP = l'écran de CRÉATION, pas celui-ci.
    name = r'data\global\ui\FrontEnd\charactercreationscreenEXP.dc6'
    data, src = probe(name)
    if not data:
        name = r'data\global\ui\CharSelect\characterselectscreenEXP.dc6'
        data, src = probe(name)
    if data:
        frames = decode_dc6(data)
        bg = render_bg(frames, pal, os.path.join(MENU, 'select-bg.png'))
        print(f'FOND OK {name} ({src}) {len(frames)} tuiles -> {bg}')
    else:
        print('FOND ABSENT :', name)
        bg = None
    out['background'] = bg

    # --- poses par classe ---
    for code, (d, tok) in CLASSES.items():
        entry = {}
        for state, suf in (('nu1', 'NU1'), ('nu2', 'NU2'), ('nu3', 'NU3')):
            n = rf'data\global\ui\FrontEnd\{d}\{tok}{suf}.DC6'
            data, src = probe(n)
            if not data and code == 'DZ':  # le druide n'a que NU3 dans certains MPQ
                n = rf'data\global\ui\FrontEnd\{d}\DZNU3.DC6'
                data, src = probe(n)
            if not data:
                print(f'{code}/{state} ABSENT ({n})')
                continue
            frames = decode_dc6(data)
            p = os.path.join(MENU, 'sel', f'{code}-{state}.png')
            info = render_frames(frames, pal, p)
            if info:
                entry[state] = info
                print(f'{code}/{state} OK {n} ({src}) {len(frames)} frames {info["w"]}x{info["h"]}')
        if entry:
            base = entry.get('nu1') or entry.get('nu3')
            entry.update({'file': base['file'], 'frames': base['frames'], 'w': base['w'], 'h': base['h'], 'fps': base['fps']})
            out[code] = entry

    jp = os.path.join(MENU, 'heroes.json')
    old = {}
    if os.path.exists(jp):
        old = json.load(open(jp))
    old.update(out)
    json.dump(old, open(jp, 'w'), indent=1)
    print('heroes.json mis à jour :', [k for k in old])

if __name__ == '__main__':
    main()
