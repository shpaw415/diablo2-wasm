#!/usr/bin/env python3
"""probe_battery.py — bulk hash-probe of MVP asset candidates across the D2 archives.
Reports HIT/MISS + size, writes /home/agent/workspace/diablo-2-wasm/assets/probe-report.json
"""
import sys, os, json

sys.path.insert(0, '/home/agent/workspace/diablo-2-wasm/tools')
from mpq_mini import MPQ

WS = '/home/agent/workspace/diablo-2-wasm'
ARCHIVES = {
    'd2data': WS + '/data/d2data.mpq',
    'd2exp': WS + '/data/d2exp.mpq',
    'd2char': WS + '/data/d2char.mpq',
    'patch_d2': WS + '/engine/patch_d2.mpq',
}

MODES = ['NU', 'WL', 'RN', 'A1', 'DD']
HERO_LAYERS = ['TR', 'HD', 'LG', 'RA', 'LA', 'RH', 'LH', 'SH']
MON_LAYERS = ['TR', 'HD', 'LG', 'RA', 'LA', 'RH', 'LH', 'SH', 'S1', 'S2']
HERO_WC = ['HT', '1HS', '2HS', '1SS', '2SS', '1HT', '2HT', '1JT', '2JT', 'ST', 'STF', 'BOW', 'XBW']
MON_WC = ['HT', 'A', 'B', '2H', '1HS']

def hero_paths():
    out = []
    for wc in HERO_WC:
        out.append(f'data\\global\\chars\\BA\\BANU{wc}.cof')
        out.append(f'data\\global\\chars\\BA\\BAWL{wc}.cof')
    for m in MODES:
        for wc in HERO_WC:
            out.append(f'data\\global\\chars\\BA\\BA{m}{wc}.cof')
            for layer in HERO_LAYERS:
                out.append(f'data\\global\\chars\\BA\\BA{m}{wc}{layer}.dcc')
    return out

def mon_paths(code):
    out = []
    for wc in MON_WC:
        for m in MODES + ['A2', 'GH', 'DT', 'SD', 'SQ']:
            out.append(f'data\\global\\monsters\\{code}\\{code}{m}{wc}.cof')
        for m in ['NU', 'WL', 'RN', 'A1', 'DD']:
            for layer in MON_LAYERS:
                out.append(f'data\\global\\monsters\\{code}\\{code}{m}{wc}{layer}.dcc')
    return out

def ds1_paths():
    names_town = ['town', 'town1', 'townne', 'townnw', 'townc', 'barb', 'barbcamp', 'rogue', 'rogues', 'camp', 'encampment']
    names_wild = ['bmoor', 'moor', 'bloodmoor', 'cold', 'coldplains', 'stony', 'stonyfield',
                  'dark', 'darkwood', 'black', 'blackmarsh', 'tamoe', 'fells', 'fens',
                  'field', 'fields', 'grass', 'meadow', 'pit', 'burial']
    dirs = ['data\\global\\tiles\\acts\\act1', 'data\\global\\tiles\\act1', 'data\\global\\tiles']
    out = []
    for d in dirs:
        for n in names_town + names_wild:
            out.append(f'{d}\\{n}.ds1')
            out.append(f'{d}\\{n}1.ds1')
    return out

def ui_paths():
    base = 'data\\global\\ui\\panel'
    cands = ['ctrlpnl.dc6', 'ctrlpnl2.dc6', 'control.dc6', 'panel.dc6', 'panelbtn.dc6',
             'hp.dc6', 'hpmono.dc6', 'manadel.dc6', 'globe.dc6', 'globe2.dc6',
             'spare.dc6', 'spare2.dc6', 'balloon.dc6', 'bufdc6.dc6', 'centerbtn.dc6',
             'ctrlsrv.dc6', 'movehere.dc6', '800ctrlpnl7.dc6', 'skillicon.dc6']
    out = [f'{base}\\{c}' for c in cands]
    out += [f'data\\global\\ui\\spells\\skillicon.dc6',
            f'data\\global\\ui\\panel\\lvlpnl.dc6',
            f'data\\global\\ui\\frontend\\ds1goto.dddc6']
    return out

def lvltile_paths():
    # DT1 candidates + palette
    out = ['data\\global\\palette\\act1\\pal.dat',
           'data\\global\\palette\\act1\\pal.pl2',
           'data\\global\\palette\\act1\\units_pal.dat']
    return out

def main():
    report = {}
    for arch, path in ARCHIVES.items():
        m = MPQ(path)
        cands = []
        if arch in ('d2data', 'd2exp'):
            cands += ds1_paths() + ui_paths() + lvltile_paths()
        if arch in ('d2data', 'd2char'):
            cands += hero_paths()
        if arch in ('d2data', 'd2exp'):
            cands += mon_paths('FA') + mon_paths('ZM') + mon_paths('SI')
        hits, miss = [], []
        for p in cands:
            b = m.block_info(p)
            if b and (b['flags'] & 0x80000000):
                hits.append({'path': p, 'fsize': b['fsize']})
            else:
                miss.append(p)
        report[arch] = {'probed': len(cands), 'hits': hits, 'n_miss': len(miss)}
        print(f'== {arch}: probed {len(cands)}, HIT {len(hits)}, MISS {len(miss)}')
        for h in hits:
            print(f"   HIT {h['path']} ({h['fsize']:,} B)")
    with open(WS + '/assets/probe-report.json', 'w') as f:
        json.dump(report, f, indent=1)
    print('report -> assets/probe-report.json')

if __name__ == '__main__':
    main()
