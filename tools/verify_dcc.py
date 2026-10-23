#!/usr/bin/env python3
"""verify_dcc.py — validation SPRITES COMPLETS (mission §4).

[1] chaque .dcc de assets/dcc décode sans erreur via dccdump (crates/d2dcc),
[2] chaque .COF local → couches <CODE><MODE><LAYER>.dcc présentes ou manquantes,
[3] chaque fichier référencé par manifest.json existe et décode,
[4] pour chaque entrée {cof,layers,missingLayers} du manifest : missingLayers
    exact dans les DEUX sens (couche absente non marquée, ou marquée à tort).

Exit 1 au premier vrai échec. [2] est informatif : les couches absentes du
retail (LH/S3/S4… sans DCC dans les MPQ) sont légitimes si le manifest les marque.
"""
import glob
import json
import os
import subprocess
import sys

WS = '/home/agent/workspace/diablo-2-wasm'
DCC = os.path.join(WS, 'assets', 'dcc')
BIN = os.path.join(WS, 'engine-rs', 'target', 'debug')
MANIFEST = os.path.join(WS, 'assets', 'manifest.json')
_TOKENS = ['HD', 'TR', 'LA', 'RA', 'LH', 'RH', 'SH', 'S1', 'S2', 'S3',
           'S4', 'S5', 'S6', 'S7', 'S8']  # enum COF §7.3 (LG absent de l'enum)

fails = []


def dcc_ok(path):
    """-> (ok, dernière ligne de sortie en cas d'erreur)."""
    r = subprocess.run([f'{BIN}/dccdump', path], capture_output=True, text=True)
    if r.returncode == 0:
        return True, ''
    lines = (r.stderr or r.stdout).strip().splitlines()
    return False, lines[-1][:120] if lines else 'erreur inconnue'


def parse_cof_layers(path):
    """-> liste de tokens déclarés (slots nuls ignorés)."""
    raw = open(path, 'rb').read()
    out = []
    for i in range(raw[0]):
        t = raw[28 + 9 * i]
        if t < len(_TOKENS):
            out.append(_TOKENS[t])
    return out


# ---- [1] tous les DCC locaux décodent
dccs = sorted(glob.glob(os.path.join(DCC, '*.dcc')))
results = [(p, dcc_ok(p)) for p in dccs]
bad = [(p, e) for p, (ok, e) in results if not ok]
print(f'[1] dccdump: {len(dccs) - len(bad)}/{len(dccs)} DCC décodent sans erreur')
for p, (_, e) in bad:
    fails.append(f'{os.path.basename(p)} ne décode pas: {e}')

# ---- [2] (informatif) couches COF → fichiers locaux
absent = []
cofs = sorted(glob.glob(os.path.join(DCC, '*.COF')))
for p in cofs:
    stem = os.path.basename(p)
    code, mode = stem[:2], stem[2:4]
    for tok in parse_cof_layers(p):
        if not os.path.exists(os.path.join(DCC, f'{code}{mode}{tok}.dcc')):
            absent.append((stem, tok))
print(f'[2] COF→couches locales: {len(cofs)} COF, {len(absent)} couches sans fichier local '
      f'(légitime si marquées missingLayers, cf [4])')

# ---- [3]+[4] manifest : existence, décodage, missingLayers exacts
m = json.load(open(MANIFEST))
entries = []  # (chemin_json, dict{cof,layers,missingLayers})


def walk(node, path='$'):
    if isinstance(node, dict):
        if isinstance(node.get('cof'), str) and isinstance(node.get('layers'), dict):
            entries.append((path, node))
        for k, v in node.items():
            walk(v, f'{path}.{k}')
    elif isinstance(node, list):
        for i, v in enumerate(node):
            walk(v, f'{path}[{i}]')


walk(m)

missing_files, bad_dcc, bad_ml = [], [], []
checked = 0
for path, e in entries:
    for k, v in e['layers'].items():
        checked += 1
        if not os.path.exists(os.path.join(WS, v)):
            missing_files.append((path, k, v))
        elif v.endswith('.dcc') and not dcc_ok(os.path.join(WS, v))[0]:
            bad_dcc.append((path, v))
    if not os.path.exists(os.path.join(WS, e['cof'])):
        missing_files.append((path, 'cof', e['cof']))
    # [4] bidirectionnel contre le COF réel
    stem = os.path.basename(e['cof'])
    code, mode = stem[:2], stem[2:4]
    marked = set(e.get('missingLayers') or [])
    really_absent = {t for t in parse_cof_layers(os.path.join(WS, e['cof']))
                     if not os.path.exists(os.path.join(DCC, f'{code}{mode}{t}.dcc'))}
    if marked != really_absent:
        bad_ml.append((path, stem, sorted(marked), sorted(really_absent)))

print(f'[3] manifest: {checked} couches + {len(entries)} entrées COF vérifiées, '
      f'{len(missing_files)} fichier(s) manquant(s), {len(bad_dcc)} DCC en erreur')
for path, k, v in missing_files:
    fails.append(f'manifest {path} [{k}]: fichier absent {v}')
for path, v in bad_dcc:
    fails.append(f'manifest {path}: {v} ne décode pas')

print(f'[4] missingLayers exacts: {len(entries) - len(bad_ml)}/{len(entries)} entrées OK')
for path, stem, marked, real in bad_ml:
    fails.append(f'manifest {path} ({stem}): marqué={marked} vs réel={real}')

if fails:
    print('\nÉCHECS:')
    for f in fails:
        print('  FAIL', f)
    sys.exit(1)
print('OK — tous les fichiers du manifest existent, décodent, et missingLayers est exact.')
