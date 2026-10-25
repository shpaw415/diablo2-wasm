#!/usr/bin/env python3
"""heroes_walks_prog.py (v2) — courbe de progression RÉELLE des marches.

v1 utilisait les offsets DC6 = la boîte englobante : quand un bras ou une
jambe s'étend, la boîte glisse et le héros « bifurquait ». v2 mesure la
vraie racine : le CENTRE DES PIEDS (pixels non transparents du quart bas,
repli centroïde complet si le perso est aérien — roue de l'Assassine),
combiné à l'offset de blit signé de la frame. Chemin cumulé (hypot), puis
clamp monotone : la racine d'une marche ne recule jamais.

Sortie : prog[] normalisée 0..1 par fw/bw dans heroes.json — menu.js
échantillonne cette courbe pour glisser au rythme exact de l'original.
"""
import json, os, sys

sys.path.insert(0, os.path.dirname(__file__))
from stormlib_read import read_file
from dc6_mini import decode_dc6

MENU = os.path.join(os.path.dirname(__file__), '..', 'demo', 'assets', 'menu')
ARCHIVES = ['../engine/patch_d2.mpq', '../data/d2exp.mpq', '../data/d2data.mpq', '../data/d2char.mpq']
CLASSES = {'AM': 'amazon', 'AS': 'assassin', 'BA': 'barbarian',
           'DZ': 'druid', 'NE': 'necromancer', 'PA': 'paladin', 'SO': 'sorceress'}


def probe(name):
    for mpq in ARCHIVES:
        p = os.path.join(os.path.dirname(__file__), mpq)
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


def row_centroids(f, y0):
    """Centroïde x par rangée (rangées avec ≥2 px), de y0 à la fin."""
    w, h, px = f['width'], f['height'], f['pixels']
    out = []
    for y in range(y0, h):
        row = px[y * w:(y + 1) * w]
        sx = n = 0
        for x in range(w):
            if row[x]:
                sx += x
                n += 1
        if n >= 2:
            out.append(sx / n)
    return out


def median(v):
    s = sorted(v)
    m = len(s) // 2
    return s[m] if len(s) % 2 else (s[m - 1] + s[m]) / 2


def foot_anchor(f):
    """Ancrage racine = MÉDIANE des centroïdes des rangées les plus basses
    (15 % du bas = les pieds plantés) + offset de blit signé. La médiane
    résiste à un poing/défense qui entre dans la zone (coup de poing vers
    le bas du Barbare). Repli zone 30 % puis corps entier si aérien."""
    w, h = f['width'], f['height']
    for frac in (0.85, 0.70):
        c = row_centroids(f, int(h * frac))
        if len(c) >= 3:
            return (median(c) + signed(f['offsetX']), h * 0.9 + signed(f['offsetY']))
    c = row_centroids(f, 0)  # aérien — corps entier
    if len(c) < 3:
        return None
    return (median(c) + signed(f['offsetX']), h * 0.5 + signed(f['offsetY']))


def candidates(f, frac=0.85, gap=12):
    """Candidats-racine : grappes de centroïdes de rangées du bas (les deux
    pieds = 2 grappes ; le poing qui entre dans la zone = une grappe de plus)."""
    w, h, px = f['width'], f['height'], f['pixels']
    y0 = int(h * frac)
    rows = []
    for y in range(y0, h):
        row = px[y * w:(y + 1) * w]
        sx = n = 0
        for x in range(w):
            if row[x]:
                sx += x
                n += 1
        if n >= 2:
            rows.append((y, sx / n))
    if len(rows) < 3:  # aérien — corps entier
        for y in range(0, h):
            row = px[y * w:(y + 1) * w]
            sx = n = 0
            for x in range(w):
                if row[x]:
                    sx += x
                    n += 1
            if n >= 2:
                rows.append((y, sx / n))
    if not rows:
        return []
    rows.sort(key=lambda r: r[1])
    out = [[rows[0]]]
    for y, c in rows[1:]:
        if c - out[-1][-1][1] <= gap:
            out[-1].append((y, c))
        else:
            out.append([(y, c)])
    return [sum(c for _, c in g) / len(g) for g in out]


def prog_curve(frames):
    """Suivi par continuité : à chaque frame on garde la grappe la plus
    proche de l'ancre précédente — les pieds avancent par petits deltas,
    le poing qui apparaît d'un coup est rejeté. Chemin = |Δx| cumulé,
    lissé 3-tap, clamp monotone, normalisé 0..1."""
    if len(frames) < 2:
        return [0.0]
    prev_x = None
    world = []
    for f in frames:
        cands = candidates(f)
        if not cands:
            world.append(world[-1] if world else None)
            continue
        if prev_x is None:
            x = median(cands)
        else:
            x = min(cands, key=lambda c: abs(c - prev_x))
        world.append(x + signed(f['offsetX']))
        prev_x = x
    if any(w is None for w in world) or len(world) < 2:
        return None
    cum = [0.0]
    for i in range(1, len(world)):
        cum.append(cum[-1] + abs(world[i] - world[i - 1]))
    cum = [cum[0]] + [(cum[i - 1] + cum[i] + cum[i + 1]) / 3 for i in range(1, len(cum) - 1)] + [cum[-1]]
    total = cum[-1]
    if total <= 0:
        return None
    out, run = [], 0.0
    for c in cum:              # clamp monotone : la racine ne recule pas
        run = max(run, c)
        out.append(round(min(run / total, 1.0), 3))
    return out


def main():
    jp = os.path.join(MENU, 'heroes.json')
    heroes = json.load(open(jp))
    for code, cls in CLASSES.items():
        for state, suf in (('fw', 'FW'), ('bw', 'BW')):
            data = probe(rf'data\global\ui\FrontEnd\{cls}\{code}{suf}.DC6')
            cur = heroes.get(code, {}).get(state)
            if not data or not cur:
                print(f'{code}/{state} absent/skip')
                continue
            frames = decode_dc6(data)
            if len(frames) != cur.get('frames'):
                print(f'{code}/{state} {len(frames)} != {cur.get("frames")} — skip')
                continue
            curve = prog_curve(frames)
            if curve is None:
                cur.pop('prog', None)
                print(f'{code}/{state} sur place — prog retirée')
            else:
                cur['prog'] = curve
                wobble = sum(1 for a, b in zip(curve, curve[1:]) if b < a)
                print(f'{code}/{state} prog OK ({len(curve)} pts, {wobble} replis)')
    json.dump(heroes, open(jp, 'w'), indent=1)
    print('heroes.json mis à jour')


if __name__ == '__main__':
    main()
