#!/usr/bin/env python3
"""make_boards.py — planches de validation par classe :
grille lignes = modes (NU WL RN A1 DD), colonnes = 8 premières frames (dir 0).
Sortie : assets/dcc/heroes-anim-<cls>.png (+ monstres-act1.png)."""
import os
import subprocess
import sys

from PIL import Image, ImageDraw

WS = '/home/agent/workspace/diablo-2-wasm'
DCC = os.path.join(WS, 'assets', 'dcc')
CLI = os.path.join(WS, 'engine-rs', 'target', 'debug', 'dcc_render')
TMP = '/tmp/opencode/boards'
os.makedirs(TMP, exist_ok=True)

MODES = ['NU', 'WL', 'RN', 'A1', 'DD']
HEROES = [('BA', '1HS', 'Barbare'), ('SO', 'STF', 'Sorcière'), ('PA', '1HS', 'Paladin')]
MONSTERS = [('FA', 'Fallen'), ('ZM', 'Zombie'), ('SI', 'QuillRat'),
            ('SK', 'Skeleton'), ('FS', 'FallenShaman'), ('YE', 'Brute')]
BG = '1A1A22'
LABEL_BG = (16, 16, 24)
FG = (220, 210, 180)


def strip(code, mode, wc, frames=8):
    out = f'{TMP}/{code}{mode}.png'
    wc_eff = 'HTH' if mode == 'DD' else wc  # DD = corps entier, wc HTH (§7.5)
    r = subprocess.run([CLI, 'compose', f'{DCC}/{code}{mode}{wc_eff}.COF', out,
                        '--dir', '0', '--frames', str(frames), '--dcc-dir', DCC,
                        '--stem', f'{code}{mode}', '--scale', '2', '--bg', BG,
                        '--pal', f'{WS}/assets/pal_act1.dat'],
                       capture_output=True, text=True)
    if r.returncode != 0 or not os.path.exists(out):
        raise SystemExit(f'compose fail {code}{mode}: {r.stdout} {r.stderr}')
    return Image.open(out)


def label_img(text, w, h, sub=''):
    img = Image.new('RGB', (w, h), LABEL_BG)
    d = ImageDraw.Draw(img)
    d.text((10, h // 2 - 12), text, fill=FG)
    if sub:
        d.text((10, h // 2 + 4), sub, fill=(150, 145, 130))
    return img


def board(rows, title, out, cols_override=None):
    """rows = [(label, sub, PIL strip)] ; empile verticalement + entête."""
    strips = [s for _, _, s in rows]
    w = max(s.width for s in strips) + 110
    head = 34
    h = sum(s.height + 6 for s in strips) + head
    img = Image.new('RGB', (w, h), LABEL_BG)
    d = ImageDraw.Draw(img)
    d.text((12, 9), title, fill=FG)
    y = head
    for label, sub, s in rows:
        img.paste(label_img(label, 110, s.height, sub), (0, y))
        x = 110 + (w - 110 - s.width) // 2
        img.paste(s, (x, y))
        y += s.height + 6
    img.save(out)
    print(f'{out} ({img.width}x{img.height})')


if __name__ == '__main__':
    which = sys.argv[1] if len(sys.argv) > 1 else 'all'
    if which in ('all', 'heroes'):
        for code, wc, name in HEROES:
            rows = []
            for mode in MODES:
                s = strip(code, mode, wc, frames=1 if mode == 'DD' else 8)
                rows.append((mode, {'NU': 'idle', 'WL': 'walk', 'RN': 'run',
                                    'A1': 'attaque', 'DD': 'mort'}[mode], s))
            board(rows, f'{name} ({code}) — modes NU/WL/RN/A1/DD, dir 0, 8 frames',
                  f'{DCC}/heroes-anim-{code.lower()}.png')
    if which in ('all', 'monsters'):
        rows = []
        for code, name in MONSTERS:
            cells = [strip(code, m, 'HTH', frames=4) for m in ('NU', 'WL', 'A1')]
            wmax = max(c.width for c in cells)
            hmax = max(c.height for c in cells)
            row = Image.new('RGB', (wmax * 3 + 12, hmax), LABEL_BG)
            for i, c in enumerate(cells):
                row.paste(c, (i * (wmax + 6), 0))
            rows.append((code, name, row))
        board(rows, 'Monstres acte I — colonnes: NU | WL | A1 (4 frames, dir 0)',
              f'{DCC}/monstres-act1.png')
