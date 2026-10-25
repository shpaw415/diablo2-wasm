#!/usr/bin/env python3
"""heroes_walks_aligned.py — planches FW/BW alignées aux offsets originaux.

L'ancien rendu (render_frames) CENTRAIT chaque frame dans sa cellule : le
corps se décalait de ±20 px selon la pose (membre tendu = boîte élargie) —
la cause du glissement résiduel. Ici chaque frame est posée à sa vraie
place : blit top-left = origine + (offsetX, offsetY) (convention validée
empiriquement : la cohérence inter-frames est 2-3× meilleure qu'en
`origine - offset`). Le trajet du personnage vit donc DANS la planche.

heroes.json gagne par fw/bw : ax, ay (l'origine commune dans la cellule —
menu.js dessine la cellule pour que ce point tombe sur la position du héros)
et netx, nety (déplacement net de la séquence, appliqué à la fin).
"""
import json, os, sys

sys.path.insert(0, os.path.dirname(__file__))
from stormlib_read import read_file
from dc6_mini import decode_dc6
from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
MENU = os.path.join(BASE, '..', 'demo', 'assets', 'menu')
ARCHIVES = [os.path.join(BASE, '..', a) for a in ('engine/patch_d2.mpq', 'data/d2exp.mpq', 'data/d2data.mpq', 'data/d2char.mpq')]
CLASSES = {'AM': 'amazon', 'AS': 'assassin', 'BA': 'barbarian',
           'DZ': 'druid', 'NE': 'necromancer', 'PA': 'paladin', 'SO': 'sorceress'}
M = 12  # marge dans la cellule


def probe(name):
    for mpq in ARCHIVES:
        p = mpq
        if not os.path.exists(p):
            continue
        try:
            d = read_file(p, name)
            if d:
                return d
        except OSError:
            pass
    return None


def signed(v):
    return v - (1 << 32) if v > (1 << 31) else v


def foot_x(f):
    """Centre des pieds : médiane des centroïdes des rangées les plus basses
    (15 % du bas — les membres qui se balancent ne tirent pas la mesure)."""
    w, h, px = f['width'], f['height'], f['pixels']
    y0 = (h * 15) // 16
    cs = []
    for y in range(y0, h):
        row = px[y * w:(y + 1) * w]
        sx = n = 0
        for x in range(w):
            if row[x]:
                sx += x
                n += 1
        if n >= 2:
            cs.append(sx / n)
    if not cs:
        return w / 2
    cs.sort()
    m = len(cs) // 2
    return cs[m] if len(cs) % 2 else (cs[m - 1] + cs[m]) / 2


def main():
    pal_d = read_file(os.path.join(BASE, '..', 'data', 'd2data.mpq'), r'data\global\palette\fechar\pal.dat')
    pal_d = (pal_d + b'\0' * 768)[:768]
    pal = [(pal_d[i * 3 + 2], pal_d[i * 3 + 1], pal_d[i * 3]) for i in range(256)]
    jp = os.path.join(MENU, 'heroes.json')
    heroes = json.load(open(jp))
    for code, cls in CLASSES.items():
        for state, suf in (('fw', 'FW'), ('bw', 'BW')):
            data = probe(rf'data\global\ui\FrontEnd\{cls}\{code}{suf}.DC6')
            if not data:
                print(f'{code}/{state} ABSENT — planche inchangée')
                continue
            frames = decode_dc6(data)
            offs = [(signed(f['offsetX']), signed(f['offsetY'])) for f in frames]
            min_ox = min(o[0] for o in offs)
            min_oy = min(o[1] for o in offs)
            W = max(f['width'] + o[0] - min_ox for f, o in zip(frames, offs)) + 2 * M
            H = max(f['height'] + o[1] - min_oy for f, o in zip(frames, offs)) + 2 * M
            img = Image.new('RGBA', (W * len(frames), H), (0, 0, 0, 0))
            for i, (f, (ox, oy)) in enumerate(zip(frames, offs)):
                fi = Image.new('RGBA', (f['width'], f['height']), (0, 0, 0, 0))
                pxf = fi.load()
                px = f['pixels']
                for y in range(f['height']):
                    row = px[y * f['width']:(y + 1) * f['width']]
                    for x, idx in enumerate(row):
                        if idx:
                            r, g, b = pal[idx]
                            pxf[x, y] = (r, g, b, 255)
                img.paste(fi, (i * W + ox - min_ox + M, oy - min_oy + M), fi)
            out = os.path.join(MENU, 'sel', f'{code}-{state}.png')
            os.makedirs(os.path.dirname(out), exist_ok=True)
            img.save(out)
            cur = heroes.setdefault(code, {}).get(state, {})
            # ancre : les pieds de la frame 0 tombent sur la position héros
            ax = -min_ox + M + offs[0][0] + foot_x(frames[0])
            ay = -min_oy + M + offs[0][1] + frames[0]['height']
            # net de fin : pieds dernière frame - pieds frame 0
            netx = (offs[-1][0] + foot_x(frames[-1])) - (offs[0][0] + foot_x(frames[0]))
            nety = (offs[-1][1] + frames[-1]['height']) - (offs[0][1] + frames[0]['height'])
            cur.update({
                'file': f'sel/{code}-{state}.png',
                'frames': len(frames),
                'w': W,
                'h': H,
                'ax': round(ax),
                'ay': round(ay),
                'netx': round(netx),
                'nety': round(nety),
            })
            cur.pop('prog', None)  # la trajectoire vit dans la planche
            print(f'{code}/{state}: {len(frames)} frames, cellule {W}x{H}, '
                  f'ancre ({cur["ax"]},{cur["ay"]}), net ({cur["netx"]},{cur["nety"]})')
    json.dump(heroes, open(jp, 'w'), indent=1)
    print('heroes.json mis à jour (planches alignées)')


if __name__ == '__main__':
    main()
