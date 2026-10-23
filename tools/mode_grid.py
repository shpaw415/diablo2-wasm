#!/usr/bin/env python3
"""mode_grid.py — planches de validation visuelle (mission SPRITES COMPLETS §2).

  assets/dcc/modes-grid-ba.png   : lignes = NU/WL/RN/A1/DD (barbare), 4 frames chacune
  assets/dcc/monsters-grid.png   : 6 monstres acte I, frame de marche (WL, dir 0)

Rendu : engine-rs/target/debug/dcc_render compose (COF + couches locales assets/dcc),
assemblage/labels via PIL. La palette BGR pal_act1.dat est le piege connu (§7 du skill).
"""
import os
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

WS = '/home/agent/workspace/diablo-2-wasm'
DCC = os.path.join(WS, 'assets', 'dcc')
RENDER = os.path.join(WS, 'engine-rs', 'target', 'debug', 'dcc_render')
PAL = os.path.join(WS, 'assets', 'pal_act1.dat')
BG_HEX = '1A1A22'   # dcc_render (hex nu)
BG = '#1A1A22'      # PIL
FG = (230, 230, 235)

MODES = ['NU', 'WL', 'RN', 'A1', 'DD']
MODE_LABELS = {'NU': 'NU idle', 'WL': 'WL marche', 'RN': 'RN course',
               'A1': 'A1 attaque', 'DD': 'DD cadavre'}
MONSTERS = [('FA', 'Fallen'), ('ZM', 'Zombie'), ('SI', 'QuillRat'),
            ('SK', 'Skeleton'), ('FS', 'FallenShaman'), ('YE', 'Brute')]


def _font(size):
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def render_strip(cof, stem, frames, scale=3):
    """-> PIL.Image, bande horizontale de `frames` frames composites."""
    out = os.path.join('/tmp/opencode', f'strip_{stem}.png')
    r = subprocess.run(
        [RENDER, 'compose', os.path.join(DCC, cof), out,
         '--dir', '0', '--scale', str(scale), '--bg', BG_HEX, '--pal', PAL,
         '--dcc-dir', DCC, '--stem', stem, '--frames', str(frames)],
        capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f'dcc_render compose a echoue sur {cof} (rc={r.returncode}):\n'
                 f'stdout: {r.stdout}\nstderr: {r.stderr}')
    return Image.open(out)


def compose_grid(rows, out_path, label_w=150, pad=12):
    """rows = [(label, PIL.Image)] -> une planchePNG empilee avec labels."""
    h = max(im.height for _, im in rows)
    w = label_w + max(im.width for _, im in rows) + 2 * pad
    board = Image.new('RGB', (w, pad + len(rows) * (h + pad)), BG)
    d = ImageDraw.Draw(board)
    y = pad
    for label, im in rows:
        d.text((12, y + im.height // 2 - 8), label, fill=FG, font=_font(18))
        board.paste(im, (label_w, y))
        d.line([(label_w - 6, y), (w - 6, y)], fill=(60, 60, 72), width=1)
        y += h + pad
    board.save(out_path)
    print(f'wrote {out_path} ({board.width}x{board.height})')


def ba_modes_grid():
    rows = []
    for mode in MODES:
        cof, stem = f'BA{mode}1HS.COF', f'BA{mode}'
        if mode == 'DD':
            cof = 'BADDHTH.COF'  # cadavre : WC hth, corps entier (§7.2)
        rows.append((MODE_LABELS[mode], render_strip(cof, stem, 4)))
    compose_grid(rows, os.path.join(DCC, 'modes-grid-ba.png'))


def monsters_grid():
    rows = []
    for code, name in MONSTERS:
        stem = f'{code}WL'
        rows.append((f'{name} ({code})', render_strip(f'{stem}HTH.COF', stem, 1)))
    compose_grid(rows, os.path.join(DCC, 'monsters-grid.png'), label_w=220)


if __name__ == '__main__':
    which = sys.argv[1] if len(sys.argv) > 1 else 'both'
    if which in ('ba', 'both'):
        ba_modes_grid()
    if which in ('mons', 'both'):
        monsters_grid()
