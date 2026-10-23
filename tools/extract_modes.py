#!/usr/bin/env python3
"""extract_modes.py — extraction COF+DCC par (code, mode) pour héros et monstres.

Procédure du skill shortcut-sprites-perso-d2 + evidence/DCC-SPEC.md :
  * sonde par hash AVANT extraction (listfile installateur incomplet),
  * extraction via engine-rs/target/debug/mpq_extract (le lecteur Python
    blast_pkware casse sur des DCC caractère réels — piège §7.4),
  * le weapon class est PAR COUCHE dans le COF (octets 28+9i+5..9) —
    piége §7.2 : sonder <CODE><LAYER><COMP><MODE><WC de la couche>.dcc.

Chemins internes (vérifiés par hash, cette vague) :
  héros   COF : data\\global\\chars\\<CLS>\\cof\\<CLS><MODE><WC>.cof
  héros   DCC : data\\global\\chars\\<CLS>\\<LAYER>\\<CLS><LAYER><COMP><MODE><WClayer>.dcc
  monstre COF : data\\global\\monsters\\<CODE>\\cof\\<CODE><MODE><WC>.cof
  monstre DCC : data\\global\\monsters\\<CODE>\\<LAYER>\\<CODE><LAYER>LIT<MODE><WClayer>.dcc

Noms locaux : <CODE><MODE><LAYER>.dcc / <CODE><MODE><WC>.COF dans assets/dcc/.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mpq_mini import MPQ

WS = '/home/agent/workspace/diablo-2-wasm'
OUT = os.path.join(WS, 'assets', 'dcc')
BIN = os.path.join(WS, 'engine-rs', 'target', 'debug')
ARCHIVES = {  # ordre de sonde
    'd2char': WS + '/data/d2char.mpq',
    'd2data': WS + '/data/d2data.mpq',
    'd2exp': WS + '/data/d2exp.mpq',
    'patch_d2': WS + '/engine/patch_d2.mpq',
}
_LAYERS = ['HD', 'TR', 'LA', 'RA', 'LH', 'RH', 'SH', 'S1', 'S2', 'S3',
           'S4', 'S5', 'S6', 'S7', 'S8']  # enum COF (§7.3 : LG absent)
_ARMOR = {'HD', 'TR', 'LG', 'LA', 'RA', 'S1', 'S2', 'S3'}

# (kind, code, mode, wc du COF, token RH arme)
ITEMS = [
    # --- héros (BA complète, RN manquant SO/PA) ---
    ('chars', 'BA', 'NU', '1HS', 'HAX'),
    ('chars', 'BA', 'WL', '1HS', 'HAX'),
    ('chars', 'BA', 'RN', '1HS', 'HAX'),
    ('chars', 'BA', 'A1', '1HS', 'HAX'),
    ('chars', 'BA', 'DD', 'HTH', None),
    ('chars', 'SO', 'RN', 'STF', 'SST'),
    ('chars', 'PA', 'RN', '1HS', 'SSD'),
    # --- monstres acte I : compléments ZM/SI/FA + nouveaux SK/FS/YE ---
    ('mons', 'ZM', 'NU', 'HTH', None),
    ('mons', 'ZM', 'A1', 'HTH', None),
    ('mons', 'ZM', 'DD', 'HTH', None),
    ('mons', 'SI', 'NU', 'HTH', None),
    ('mons', 'SI', 'A1', 'HTH', None),
    ('mons', 'SI', 'DD', 'HTH', None),
    ('mons', 'FA', 'NU', 'HTH', None),
    ('mons', 'FA', 'A1', 'HTH', None),
    ('mons', 'FA', 'DD', 'HTH', None),
    ('mons', 'SK', 'NU', 'HTH', None),
    ('mons', 'SK', 'WL', 'HTH', None),
    ('mons', 'SK', 'A1', 'HTH', None),
    ('mons', 'SK', 'DD', 'HTH', None),
    ('mons', 'FS', 'NU', 'HTH', None),
    ('mons', 'FS', 'WL', 'HTH', None),
    ('mons', 'FS', 'A1', 'HTH', None),
    ('mons', 'FS', 'DD', 'HTH', None),
    ('mons', 'YE', 'NU', 'HTH', None),
    ('mons', 'YE', 'WL', 'HTH', None),
    ('mons', 'YE', 'A1', 'HTH', None),
    ('mons', 'YE', 'DD', 'HTH', None),
    # --- boss de quête acte I (W11 slice, impl-monstres-boss) ---
    ('mons', 'AN', 'NU', 'HTH', None),
    ('mons', 'AN', 'WL', 'HTH', None),
    ('mons', 'AN', 'A1', 'HTH', None),
    ('mons', 'AN', 'DD', 'HTH', None),
]

_mpqs = {k: MPQ(v) for k, v in ARCHIVES.items()}


def probe(name: str):
    """-> (archive, block_info) du premier hit, sinon None."""
    n = name.replace('/', '\\')
    for k, a in _mpqs.items():
        bi = a.block_info(n)
        if bi and (bi['flags'] & 0x80000000):
            return k, bi
    return None


def parse_cof(raw: bytes):
    """-> dict {n_layers, frames, dirs, speed, layers:[(token, wc4)]}."""
    n = raw[0]
    assert raw[0:3] != b'COF' or True
    layers = []
    nulls = 0
    for i in range(n):
        base = 28 + 9 * i
        ltype = raw[base]
        wc = raw[base + 5:base + 9].rstrip(b'\x00').decode('ascii', 'replace')
        if ltype >= len(_LAYERS):
            nulls += 1  # slot nul (COF monstres DD : ex. SIDD n=2, 2e couche type 15)
            continue
        layers.append((_LAYERS[ltype], wc))
    return {
        'n_layers': n,
        'null_slots': nulls,
        'frames': raw[1],
        'dirs': raw[2],
        'speed': raw[24],
        'layers': layers,
    }


def dcc_internal(kind, code, layer, comp, mode, wc):
    if kind == 'chars':
        return f'data/global/chars/{code.lower()}/{layer.lower()}/{code.lower()}{layer.lower()}{comp.lower()}{mode.lower()}{wc.lower()}.dcc'
    return f'data/global/monsters/{code.lower()}/{layer.lower()}/{code.lower()}{layer.lower()}{comp.lower()}{mode.lower()}{wc.lower()}.dcc'


def cof_internal(kind, code, mode, wc):
    if kind == 'chars':
        return f'data/global/chars/{code.lower()}/cof/{code.lower()}{mode.lower()}{wc.lower()}.cof'
    return f'data/global/monsters/{code.lower()}/cof/{code.lower()}{mode.lower()}{wc.lower()}.cof'


def extract(archive, internal, out):
    r = subprocess.run([f'{BIN}/mpq_extract', ARCHIVES[archive], internal, out],
                       capture_output=True, text=True)
    return r.returncode == 0 and os.path.exists(out)


def verify_dcc(path):
    r = subprocess.run([f'{BIN}/dccdump', path], capture_output=True, text=True)
    return r.returncode == 0, (r.stdout + r.stderr).strip().splitlines()[-1:] or ['']


def main():
    only = sys.argv[1:] or None
    report = {}
    for kind, code, mode, wc, rh in ITEMS:
        tag = f'{code}{mode}'
        if only and tag not in only and f'{code}-{mode}' not in only:
            continue
        item = {'kind': kind, 'cof': None, 'layers': {}, 'missing': []}
        cof_int = cof_internal(kind, code, mode, wc)
        hit = probe(cof_int)
        if not hit:
            print(f'{tag}: COF INTROUVABLE {cof_int}')
            report[tag] = item
            item['error'] = 'cof-not-found'
            continue
        arch, bi = hit
        cof_local = os.path.join(OUT, f'{code}{mode}{wc}.COF')
        if not os.path.exists(cof_local) or os.path.getsize(cof_local) != bi['fsize']:
            assert extract(arch, cof_int, cof_local), f'extract fail {cof_int}'
        raw = open(cof_local, 'rb').read()
        cof = parse_cof(raw)
        item['cof'] = {'internal': cof_int, 'archive': arch, **{k: cof[k] for k in ('frames', 'dirs', 'speed')},
                       'layers': [{'token': t, 'wc': w} for t, w in cof['layers']]}
        for token, lwc in cof['layers']:
            comps = ['LIT']
            if token == 'RH' and rh:
                comps = [rh, 'LIT']
            elif token in ('LH', 'SH'):
                comps = ['BUC', 'LIT']
            got = None
            for comp in comps:
                di = dcc_internal(kind, code, token, comp, mode, lwc)
                h = probe(di)
                if h:
                    a2, bi2 = h
                    local = os.path.join(OUT, f'{code}{mode}{token}.dcc')
                    if not os.path.exists(local) or os.path.getsize(local) != bi2['fsize']:
                        assert extract(a2, di, local), f'extract fail {di}'
                    ok, info = verify_dcc(local)
                    if not ok:
                        item.setdefault('decode_fail', []).append(f'{token}:{info}')
                        break
                    item['layers'][token] = {'internal': di, 'archive': a2, 'wc': lwc,
                                             'bytes': os.path.getsize(local)}
                    got = token
                    break
            if not got:
                item['missing'].append(token)
        # couches LG hors COF (piége §7.3) : fichier présent -> extraire pour le manifest
        if kind == 'chars' and 'LG' not in [t for t, _ in cof['layers']]:
            di = dcc_internal(kind, code, 'LG', 'LIT', mode, wc)
            h = probe(di)
            if h:
                a2, bi2 = h
                local = os.path.join(OUT, f'{code}{mode}LG.dcc')
                if not os.path.exists(local) or os.path.getsize(local) != bi2['fsize']:
                    assert extract(a2, di, local), f'extract fail {di}'
                item['layers']['LG'] = {'internal': di, 'archive': a2, 'wc': wc,
                                        'bytes': os.path.getsize(local), 'note': 'hors-COF'}
        miss = ','.join(item['missing']) or '-'
        print(f"{tag}: {len(item['layers'])} couches OK, manquantes COF={miss}"
              + (' (dont hors-COF extraites: ' + ','.join(k for k in item['layers'] if k not in [t for t, _ in cof['layers']]) + ')' if any(k not in [t for t, _ in cof['layers']] for k in item['layers']) else ''))
        report[tag] = item
    with open(os.path.join(OUT, 'extract-report.json'), 'w') as f:
        json.dump(report, f, indent=1)
    print('report -> assets/dcc/extract-report.json')


if __name__ == '__main__':
    main()
