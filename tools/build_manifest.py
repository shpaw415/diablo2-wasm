#!/usr/bin/env python3
"""build_manifest.py — injecte modes:{NU,WL,RN,A1,DD:{cof,layers,missingLayers,...}}
dans manifest.json pour heroes {BA,SOR,PA} et monsters[] (FA,ZM,SI + SK,FS,YE).
Chemins relatifs au dépôt (assets/dcc/...), cohérents avec le moteur d2game
(`heroes[code].modes` / `monsters[].modes`). Garde `hero` et les blocs `walk`
existants intacts (rétro-compat, test hero_selection_default_stays_retrocompatible)."""
import json
import os

WS = '/home/agent/workspace/diablo-2-wasm'
DCC = os.path.join(WS, 'assets', 'dcc')
REPORT = json.load(open(os.path.join(DCC, 'extract-report.json')))

HERO_WC = {'BA': '1HS', 'SO': '1HS', 'PA': '1HS'}  # clé manifest ; COF SO = STF
COF_WC = {'BA': '1HS', 'SOR': 'STF', 'PA': '1HS'}
HERO_MODES = ['NU', 'WL', 'RN', 'A1', 'DD']
MON_MODES = ['NU', 'WL', 'A1', 'DD']
MONSTERS = ['FA', 'ZM', 'SI', 'SK', 'FS', 'YE']
# Bosses de quête (W11) : blocs manifest.bosses — PAS dans `monsters` (ils ne
# doivent pas auto-spawner : la logique de niveau/quête les pose). questEvent =
# événement émis par le moteur à la mort (écoute quête/fin, impl-monstres-boss).
BOSSES = {'AN': 'andariel_dead'}


_TOKENS = ['HD', 'TR', 'LA', 'RA', 'LH', 'RH', 'SH', 'S1', 'S2', 'S3',
           'S4', 'S5', 'S6', 'S7', 'S8']


def parse_cof(path):
    raw = open(path, 'rb').read()
    layers, nulls = [], 0
    for i in range(raw[0]):
        b = 28 + 9 * i
        t = raw[b]
        if t >= len(_TOKENS):
            nulls += 1
            continue
        layers.append((_TOKENS[t], raw[b + 5:b + 9].rstrip(b'\x00').decode('ascii', 'replace')))
    return {'frames': raw[1], 'dirs': raw[2], 'speed': raw[24], 'layers': layers, 'nulls': nulls}


def modes_block(code, key, modes):
    """construit modes{} depuis les COF locaux + présence des DCC locaux."""
    wc_key = COF_WC.get(key, 'HTH')
    out = {}
    for mode in modes:
        wc_cof = 'HTH' if mode == 'DD' else wc_key
        cof_local = f'assets/dcc/{code}{mode}{wc_cof}.COF'
        if not os.path.exists(os.path.join(WS, cof_local)):
            continue
        cof = parse_cof(os.path.join(WS, cof_local))
        layers, missing = {}, []
        for tok, _wc in cof['layers']:
            p = f'assets/dcc/{code}{mode}{tok}.dcc'
            if os.path.exists(os.path.join(WS, p)):
                layers[tok] = p
            else:
                missing.append(tok)
        # couches hors-COF extraites (LG, piége §7.3) : listées pour le moteur
        lg = f'assets/dcc/{code}{mode}LG.dcc'
        if 'LG' not in layers and 'LG' not in missing and os.path.exists(os.path.join(WS, lg)):
            layers['LG'] = lg  # hors enum COF, non composé mais présent
        out[mode] = {
            'cof': cof_local,
            'layers': layers,
            'missingLayers': missing,
            'frames': cof['frames'],
            'dirs': cof['dirs'],
            'speed': cof['speed'],
        }
    return out


def main():
    man = json.load(open(os.path.join(WS, 'assets', 'manifest.json')))
    added = []
    for key in ['BA', 'SOR', 'PA']:
        code = 'SO' if key == 'SOR' else key
        modes = modes_block(code, key, HERO_MODES)
        assert len(modes) == 5, f'{key}: modes incomplets {list(modes)}'
        man['heroes'][key]['modes'] = modes
        added.append(f'heroes.{key}')
    by_code = {m['code']: m for m in man['monsters']}
    for code in MONSTERS:
        modes = modes_block(code, code, MON_MODES)
        assert len(modes) == 4, f'{code}: modes monstre incomplets {list(modes)}'
        if code in by_code:
            by_code[code]['modes'] = modes
        else:
            man['monsters'].append({'code': code, 'modes': modes,
                                    'statsPendingMonstats': True})
        added.append(f'monsters.{code}')
    # Bosses : man['bosses'] = [{code,name,id,questEvent,stats,modes}] (W11).
    bosses = man.setdefault('bosses', [])
    bosses_by_code = {b['code']: b for b in bosses}
    for code, quest_event in BOSSES.items():
        modes = modes_block(code, code, MON_MODES)
        assert len(modes) == 4, f'boss {code}: modes incomplets {list(modes)}'
        ent = bosses_by_code.get(code)
        if ent is None:
            ent = {'code': code}
            bosses.append(ent)
            bosses_by_code[code] = ent
        ent['modes'] = modes
        ent['questEvent'] = quest_event
        added.append(f'bosses.{code}')
    # stats des nouveaux monstres depuis patch_monstats.txt (1re ligne du code)
    lines = open(os.path.join(WS, 'assets', 'patch_monstats.txt')).read().splitlines()
    hdr = lines[0].split('\t')
    ci = {w: hdr.index(w) for w in ['Code', 'NameStr', 'Id', 'Level', 'minHP', 'maxHP',
                                    'AC', 'A1MinD', 'A1MaxD', 'A1TH', 'Exp', 'Velocity',
                                    'Run', 'TreasureClass1', 'MonSound']}
    first = {}
    for l in lines[1:]:
        f = l.split('\t')
        if len(f) < len(hdr):
            continue
        c = f[ci['Code']]
        if (c in MONSTERS or c in BOSSES) and c not in first:
            first[c] = f
    for code in MONSTERS:
        f = first.get(code)
        ent = next(m for m in man['monsters'] if m['code'] == code)
        if not f:
            ent.pop('statsPendingMonstats', None)
            continue
        ent.setdefault('stats', {
            'level': int(f[ci['Level']]),
            'hp': int(f[ci['maxHP']]),
            'hpMin': int(f[ci['minHP']]),
            'ac': int(f[ci['AC']]),
            'dmgMin': int(f[ci['A1MinD']]),
            'dmgMax': int(f[ci['A1MaxD']]),
            'th': int(f[ci['A1TH']]),
            'exp': int(f[ci['Exp']]),
            'speed': int(f[ci['Velocity']]),
            'runSpeed': int(f[ci['Run']]),
            'treasureClass': f[ci['TreasureClass1']],
            'sound': f[ci['MonSound']],
        })
        ent.pop('statsPendingMonstats', None)
        ent.setdefault('name', f[ci['NameStr']])
        ent.setdefault('id', f[ci['Id']])
    # Stats/noms des bosses : même lecture patch_monstats, 1re ligne du Code
    # (AN -> `andariel`, pas `uberandariel`), TC1 = TC de drop dédiée réelle.
    for code in BOSSES:
        f = first.get(code)
        if not f:
            continue
        ent = next(b for b in man['bosses'] if b['code'] == code)
        ent['stats'] = {
            'level': int(f[ci['Level']]),
            'hp': int(f[ci['maxHP']]),
            'hpMin': int(f[ci['minHP']]),
            'ac': int(f[ci['AC']]),
            'dmgMin': int(f[ci['A1MinD']]),
            'dmgMax': int(f[ci['A1MaxD']]),
            'th': int(f[ci['A1TH']]),
            'exp': int(f[ci['Exp']]),
            'speed': int(f[ci['Velocity']]),
            'runSpeed': int(f[ci['Run']]),
            'treasureClass': f[ci['TreasureClass1']],
            'sound': f[ci['MonSound']],
        }
        ent['name'] = f[ci['NameStr']]
        ent['id'] = f[ci['Id']]
    with open(os.path.join(WS, 'assets', 'manifest.json'), 'w') as fp:
        json.dump(man, fp, indent=1, ensure_ascii=False)
    print('modes injectés pour:', ', '.join(added))


if __name__ == '__main__':
    main()
