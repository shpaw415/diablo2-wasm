#!/usr/bin/env python3
"""probe_menu2.py — brute-force hash probe des DC6 d'UI (boutons/panneaux) D2 1.12."""
import sys, os, itertools
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mpq_mini import MPQ

DIRS = ['frontend', 'panel', 'mainmenu', 'buttons', 'title', 'menu', 'spells', 'screens']
BASES = [r'data\global\ui', r'data\global\ui\{dir}', r'data\local\ui\{dir}']

CORE = ['btn', 'button', 'btnmain', 'btnwide', 'btnbig', 'btnfull', 'btnsmall',
        'broadmain', 'broad', 'main', 'mainmenu', 'mainscreen', 'menu', 'menubtn',
        'menuwide', 'wide', 'widebtn', 'panel', 'frontend', 'front', 'title',
        'titlescreen', 'titleiscreen', 'screen', 'logo', 'ok', 'cancel', 'enter',
        'exit', 'single', 'battle', 'bnet', 'config', 'credits', 'cinematics',
        'quit', 'multi', 'play', 'load', 'new', 'convert', 'hero', 'herosel',
        'heroselscreen', 'classsel', 'selhero', 'heronew', 'heroexp',
        'skltree', 'skt', 'ctrlpnl', 'controlpanel', 'ui', 'frame', 'border']
SUFS = ['', '0', '1', '2', '3', '4', '5', 'a', 'b', 'c', 'up', 'down', 'hot', 'flat',
        'dis', 'disabled', 'left', 'right', 'top', 'bottom', 'small', 'big', 'wide',
        'full', 'panel', 'screen', 'btn', 'button', 's', 'dc6']
EXTRA = [
    r'data\global\ui\frontend\titlescreen.dc6',
    r'data\global\ui\frontend\titleleanleft.dc6', r'data\global\ui\frontend\titleleanright.dc6',
    r'data\global\ui\frontend\titleelevator.dc6', r'data\global\ui\frontend\menuact.dc6',
    r'data\global\ui\frontend\menuacta.dc6', r'data\global\ui\frontend\menuactb.dc6',
    r'data\global\ui\frontend\menuactc.dc6', r'data\global\ui\frontend\menuactd.dc6',
    r'data\global\ui\frontend\herosel.dc6', r'data\global\ui\frontend\newhero.dc6',
    r'data\global\ui\frontend\selecthero.dc6', r'data\global\ui\frontend\hmmainscreen.dc6',
    r'data\global\ui\frontend\titledarklandscap.dc6',
    r'data\global\ui\panel\invcap.dc6',  # sanity (déjà connu)
]

names = set(EXTRA)
for d in DIRS:
    for c in CORE:
        for s in SUFS:
            names.add(rf'data\global\ui\{d}\{c}{s}.dc6')
            names.add(rf'data\global\ui\{d}\{c}_{s}.dc6' if s else rf'data\global\ui\{d}\{c}.dc6')
# per-button main-menu names (en, court)
for w in ['singleplayer','battlecry','multiplayer','blizzard','settings','exitgame',
          'credits','cinematics','webhelp','showloader','bnet','gateway']:
    names.add(rf'data\global\ui\frontend\{w}.dc6')
    names.add(rf'data\global\ui\panel\{w}.dc6')
    names.add(rf'data\global\ui\mainmenu\{w}.dc6')
names = sorted(names)
print(f'{len(names)} candidats')

ARCHIVES = [('patch_d2', 'engine/patch_d2.mpq'), ('d2data', 'data/d2data.mpq'),
            ('d2exp', 'data/d2exp.mpq'), ('d2char', 'data/d2char.mpq')]
total = 0
for tag, path in ARCHIVES:
    m = MPQ(path)
    found = []
    for name in names:
        bi = m.block_info(name)
        if bi and (bi['flags'] & 0x80000000) and not (bi['flags'] & 0x02000000):
            found.append((name, bi['fsize']))
    if found:
        print(f'== {tag} ==')
        for n, fs in found:
            print(f'  HIT {n}  fsize={fs}')
        total += len(found)
print('TOTAL', total)
