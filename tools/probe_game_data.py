#!/usr/bin/env python3
"""probe_game_data.py — Probe MPQ archives for candidate paths (hash lookup) and
report HIT/MISS + size. Used to build docs/GAME-DATA.md and assets/manifest.json
with VERIFIED entries only.

Usage:
  python3 tools/probe_game_data.py <archive.mpq> <path> [path...]   # probe & report
  python3 tools/probe_game_data.py <archive.mpq> --extract <path> <outfile>
"""
import sys, os, json

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from mpq_mini import MPQ

ARCHIVES = {
    'd2data': '/home/agent/workspace/diablo-2-wasm/data/d2data.mpq',
    'd2exp': '/home/agent/workspace/diablo-2-wasm/data/d2exp.mpq',
    'd2char': '/home/agent/workspace/diablo-2-wasm/data/d2char.mpq',
    'patch_d2': '/home/agent/workspace/diablo-2-wasm/engine/patch_d2.mpq',
}


def probe(archive, paths):
    mpq = MPQ(ARCHIVES.get(archive, archive))
    out = []
    for p in paths:
        try:
            b = mpq.block_info(p)
        except Exception as e:
            b = None
        if b and (b['flags'] & 0x80000000):
            out.append((p, 'HIT', b['fsize']))
        else:
            out.append((p, 'MISS', None))
    return out


def extract(archive, path, outfile):
    mpq = MPQ(ARCHIVES.get(archive, archive))
    data = mpq.read_file(path)
    if data is None:
        print(f"NOT FOUND: {path}")
        return 1
    os.makedirs(os.path.dirname(outfile) or '.', exist_ok=True)
    with open(outfile, 'wb') as f:
        f.write(data)
    print(f"OK {path}: {len(data):,} bytes -> {outfile}")
    return 0


if __name__ == '__main__':
    a = sys.argv[1]
    args = sys.argv[2:]
    if args and args[0] == '--extract':
        sys.exit(extract(a, args[1], args[2]))
    for p, st, size in probe(a, args):
        print(f"{st}  {p}  {'' if size is None else f'{size:,} bytes'}")
