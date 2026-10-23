#!/usr/bin/env python3
"""heroes_walks.py — sheets d'animation de sélection (ForwardWalk/BackWalk).

Pour chaque classe : FrontEnd\\<dir>\\<XX>FW.DC6 (marche avant, sélection) et
<XX>BW.DC6 (marche arrière, désélection) rendus avec la palette fechar (BGR)
-> demo/assets/menu/sel/<CODE>-fw.png / -bw.png + entrées fw/bw dans heroes.json.
"""
import json, os, sys

sys.path.insert(0, os.path.dirname(__file__))
from stormlib_read import read_file
from dc6_mini import decode_dc6
from heroes_select_bg import render_frames  # réutilise le rendu de sheets

MENU = os.path.join(os.path.dirname(__file__), '..', 'demo', 'assets', 'menu')
ARCHIVES = ['engine/patch_d2.mpq', 'data/d2exp.mpq', 'data/d2data.mpq', 'data/d2char.mpq']
CLASSES = {'AM': 'amazon', 'AS': 'assassin', 'BA': 'barbarian',
           'DZ': 'druid', 'NE': 'necromancer', 'PA': 'paladin', 'SO': 'sorceress'}

def fechar_palette():
    d = read_file('data/d2data.mpq', r'data\global\palette\fechar\pal.dat')
    d = (d + b'\0' * 768)[:768]
    return [(d[i*3+2], d[i*3+1], d[i*3]) for i in range(256)]  # BGR -> RGB

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
            return data
    return None

def main():
    pal = fechar_palette()
    jp = os.path.join(MENU, 'heroes.json')
    heroes = json.load(open(jp))
    for code, d in CLASSES.items():
        for state, suf in (('fw', 'FW'), ('bw', 'BW')):
            name = rf'data\global\ui\FrontEnd\{d}\{code}{suf}.DC6'
            data = probe(name)
            if not data and code == 'AS':  # l'assassin n'a pas de BW listé partout
                name = rf'data\global\ui\FrontEnd\{d}\{code}FWs.DC6' if state == 'bw' else name
                data = probe(name) if state == 'bw' else data
            if not data:
                print(f'{code}/{state} ABSENT')
                continue
            frames = decode_dc6(data)
            info = render_frames(frames, pal, os.path.join(MENU, 'sel', f'{code}-{state}.png'))
            if info:
                info['fps'] = max(8, round(len(frames) / (2.4 if suf == 'FW' else 1.1)))  # durées od2 (barbForward 2500 ms, back 1000 ms)
                heroes[code][state] = info
                print(f'{code}/{state} OK {len(frames)} frames {info["w"]}x{info["h"]}')
    json.dump(heroes, open(jp, 'w'), indent=1)
    print('heroes.json mis à jour')

if __name__ == '__main__':
    main()
