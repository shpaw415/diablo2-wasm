#!/usr/bin/env python3
"""probe_menu.py — sonde les archives MPQ pour les DC6 du front-end (menu principal).

Sondage par hash (le (listfile) du digital v1.12 est tronqué/piège).
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mpq_mini import MPQ

ARCHIVES = [
    ('patch_d2', '/home/agent/workspace/diablo-2-wasm/engine/patch_d2.mpq'),
    ('d2data',   '/home/agent/workspace/diablo-2-wasm/data/d2data.mpq'),
    ('d2exp',    '/home/agent/workspace/diablo-2-wasm/data/d2exp.mpq'),
    ('d2char',   '/home/agent/workspace/diablo-2-wasm/data/d2char.mpq'),
]

# candidats: noms exacts testés tels quels (backslash MPQ)
CANDIDATES = []
def add(p): CANDIDATES.append(p)

# --- frontend ---
for base in [r'data\global\ui\frontend']:
    for n in ['titleiscreen.dc6', 'titlescreen.dc6', 'title.dc6', 'titleiscreendark.dc6',
              'titleleanleft.dc6', 'titleleanright.dc6', 'titleelevator.dc6',
              'menuact.dc6', 'menuacta.dc6', 'menuactb.dc6', 'menuactc.dc6',
              'frontendpanel.dc6', 'frontend.dc6', 'herosel.dc6', 'herosel1.dc6',
              'herosel2.dc6', 'herosel3.dc6', 'newhero.dc6', 'hmmainscreen.dc6',
              'fire.dc6', 'ember.dc6']:
        add(f'{base}\\{n}')
# --- mainmenu ---
for n in ['mainmenu.dc6', 'mainscreen.dc6', 'menu.dc6', 'menu0.dc6',
          'titleiscreen.dc6', 'title.dc6', 'broadmain.dc6']:
    add(rf'data\global\ui\mainmenu\{n}')
# --- panel (boutons) ---
for n in ['broadmain.dc6', 'broadskill.dc6', 'broadleft.dc6', 'broadright.dc6',
          'broadmap.dc6', 'menuwide.dc6', 'widebutton.dc6', 'mainmenu.dc6',
          'mainpanel.dc6', 'menustrings.dc6', 'ds1button.dc6', 'd2button.dc6']:
    add(rf'data\global\ui\panel\{n}')
# --- title (autre racine possible) ---
for n in ['titleiscreen.dc6', 'title.dc6', 'broadmain.dc6', 'mainmenu.dc6']:
    add(rf'data\global\ui\title\{n}')
    add(rf'data\global\ui\menu\{n}')
# --- hero selection ---
for n in ['classselbtns.dc6', 'herosel.dc6', 'sktdown.dc6', 'sktl.dc6', 'sktr.dc6',
          'skttab0.dc6', 'skttab1.dc6', 'skttab2.dc6', 'skttab3.dc6', 'skttab4.dc6',
          'skttab5.dc6', 'skttab6.dc6', 'campain.dc6', 'btnfull.dc6', 'btnbig.dc6']:
    add(rf'data\global\ui\panel\{n}')
# --- frontend variants connus du fandom ---
for n in ['titleiscreen.dc6', 'titledarklandscap.dc6', 'diablo2logo.dc6', 'logo.dc6']:
    add(rf'data\global\ui\frontend\{n}')

def main():
    for tag, path in ARCHIVES:
        if not os.path.exists(path):
            print(f'== {tag}: ABSENT ==')
            continue
        m = MPQ(path)
        found = []
        for name in CANDIDATES:
            bi = m.block_info(name)
            if bi and (bi['flags'] & 0x80000000) and not (bi['flags'] & 0x02000000):
                found.append((name, bi['fsize']))
        if found:
            print(f'== {tag} ({path}) ==')
            for name, fsize in found:
                print(f'  HIT {name}  fsize={fsize}')
        else:
            print(f'== {tag}: aucun candidat ==')

if __name__ == '__main__':
    main()
