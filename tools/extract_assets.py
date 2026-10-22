#!/usr/bin/env python3
"""extract_assets.py — Extrait les actifs Diablo 2 nécessaires au jeu depuis VOTRE
propre installation (aucun asset n'est distribué dans ce repo).

Usage:
  # À partir des tomes d'installation (fichiers .mpq du/downloader Blizzard) :
  python3 extract_assets.py --tomes "/chemin/vers/installer" --out ../demo/assets

  # Ou à partir d'un jeu installé (d2data.mpq, d2char.mpq, d2exp.mpq, patch_d2.mpq) :
  python3 extract_assets.py --game "/chemin/vers/Diablo II" --out ../demo/assets

Ne publiez/partagez JAMAIS les fichiers extraits : © Blizzard Entertainment.
"""
import argparse, os, sys, struct, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from mpq_mini import MPQ  # noqa: E402

# ------------------------------------------------------------------ jeux de données
# Carte du Camp des Rousses : DS1 + DT1 (source : assets/map/MANIFEST.json du projet)
MAP_DT1 = [
    ("data\\global\\tiles\\act1\\town\\floor.dt1", "town_floor.dt1"),
    ("data\\global\\tiles\\act1\\town\\fence.dt1", "town_fence.dt1"),
    ("data\\global\\tiles\\act1\\town\\objects.dt1", "town_objects.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\treegroups.dt1", "out_treegroups.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\stonewall.dt1", "out_stonewall.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\river.dt1", "out_river.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\objects.dt1", "out_objects.dt1"),
    ("data\\global\\tiles\\act1\\barracks\\warp.dt1", "warp.dt1"),
]
MAP_EXTRA_DT1 = [
    ("data\\global\\tiles\\act1\\town\\river.dt1", "river.dt1"),
    ("data\\global\\tiles\\act1\\town\\stonewall.dt1", "stonewall.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\stones.dt1", "stones.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\treegroups.dt1", "treegroups.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\bridge.dt1", "out_bridge.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\fence.dt1", "out_fence.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\stones.dt1", "out_stones.dt1"),
    ("data\\global\\tiles\\act1\\outdoors\\tower.dt1", "out_tower.dt1"),
    ("data\\global\\tiles\\act1\\town\\treegroups.dt1", "town_trees.dt1"),
    ("data\\global\\tiles\\act1\\town\\objects.dt1", "town_objects.dt1"),
]
DS1_CANDIDATES = [
    "data\\global\\tiles\\act1\\town\\TownN1.ds1",
    "data\\global\\tiles\\act1\\town\\townN1.ds1",
]
# Sons : candidats par rôle (d2sfx.mpq). Le jeu tourne sans eux (avertissements).
SFX_CANDIDATES = {
    "ui_click.wav": ["data\\global\\sfx\\cursor\\button.wav", "data\\global\\sfx\\cursor\\itemclick.wav"],
    "attack.wav": ["data\\global\\sfx\\item\\weapon\\swingsword.wav", "data\\global\\sfx\\item\\weapon\\swingaxe.wav"],
    "hit.wav": ["data\\global\\sfx\\item\\weapon\\meleehit.wav", "data\\global\\sfx\\item\\weapon\\arrowswoosh.wav"],
}

def find_tome(root, name):
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            if f.lower() == name.lower():
                return os.path.join(dirpath, f)
    return None

def open_archive(candidates):
    for c in candidates:
        if c and os.path.isfile(c):
            try:
                return MPQ(c)
            except Exception:
                pass
    return None

def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--tomes", help="dossier contenant les Installer Tome*.mpq (release digitale)")
    g.add_argument("--game", help="dossier du jeu installé (contient *.mpq)")
    ap.add_argument("--out", default="../demo/assets", help="dossier de sortie (défaut ../demo/assets)")
    args = ap.parse_args()
    out = os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)

    archives = {}  # clé -> MPQ
    if args.tomes:
        print("Extraction des MPQ de données imbriqués depuis les tomes (plusieurs minutes)...")
        jobs = [("Installer Tome 2.mpq", "Common\\d2data.mpq", "d2data"),
                ("Installer Tome.mpq", "Common\\d2sfx.mpq", "d2sfx"),
                ("Installer Tome 3.mpq", "Common\\d2char.mpq", "d2char")]
        for tome, inner, key in jobs:
            p = find_tome(args.tomes, tome)
            if not p:
                print(f"  !! tome absent : {tome}")
                continue
            a = MPQ(p)
            data = a.read_file(inner)
            tmp = os.path.join(out, f"_{key}.mpq")
            open(tmp, "wb").write(data)
            archives[key] = MPQ(tmp)
            os.remove(tmp)
            print(f"  {key}: {len(data):,} o")
        # d2exp (LoD) : optionnel
        es = find_tome(args.tomes, "Installer Tome.mpq")
        d2es = os.path.join(os.path.dirname(es), "..", "D2ES (V1.12) fr-FR", "Installer Tome.mpq") if es else None
    else:
        for key, fname in [("d2data", "d2data.mpq"), ("d2exp", "d2exp.mpq"),
                           ("d2char", "d2char.mpq"), ("patch_d2", "patch_d2.mpq"),
                           ("d2sfx", "d2sfx.mpq")]:
            p = find_tome(args.game, fname)
            if p:
                archives[key] = MPQ(p)
                print(f"  {key}: {p}")

    order = ["patch_d2", "d2data", "d2exp", "d2char"]  # ordre de priorité de lecture

    def extract(src_paths, dest, encrypted_ok=True):
        """Essaie chaque chemin source dans chaque archive (ordre de priorité)."""
        for path in src_paths:
            for key in order:
                mpq = archives.get(key)
                if not mpq:
                    continue
                try:
                    b = mpq.block_info(path)
                    if b and b["flags"] & 0x80000000:
                        data = mpq.read_file(path)
                        if data:
                            os.makedirs(os.path.dirname(dest), exist_ok=True)
                            open(dest, "wb").write(data)
                            return len(data), key
                except NotImplementedError:
                    continue
                except Exception:
                    continue
        return 0, None

    ok, skip = 0, 0

    # 1) Palette acte I
    n, src = extract(["data\\global\\palette\\act1\\pal.dat"], os.path.join(out, "map", "pal_act1.dat"))
    print(("OK " if n else "!! ") + f"palette ({src})"); ok += bool(n); skip += (not n)

    # 2) Carte : DS1 + DT1
    n, src = extract(DS1_CANDIDATES, os.path.join(out, "map", "TownN1.ds1"))
    print(("OK " if n else "!! ") + f"TownN1.ds1 ({src})"); ok += bool(n); skip += (not n)
    for srcp, name in MAP_DT1 + MAP_EXTRA_DT1:
        n, _ = extract([srcp], os.path.join(out, "map", name))
        ok += bool(n); skip += (not n)
        if not n: print(f"!! {name}")
    print(f"   carte : {len(MAP_DT1)+len(MAP_EXTRA_DT1)-(skip and 0 or 0)} tuiles demandées")

    # 3) Sprites héros/monstres depuis le manifest (chemins relatifs demo/assets)
    import json
    mf = os.path.join(os.path.dirname(HERE), "demo", "assets", "manifest.json")
    if not os.path.isfile(mf):
        mf = os.path.join(HERE, "manifest.json")
    sprites = []
    if os.path.isfile(mf):
        man = json.load(open(mf))
        def walk(o):
            if isinstance(o, str) and (o.endswith(".dcc") or o.endswith(".COF") or o.endswith(".cof")):
                sprites.append(o)
            elif isinstance(o, dict):
                for v in o.values(): walk(v)
            elif isinstance(o, list):
                for v in o: walk(v)
        walk(man.get("heroes", {})); walk(man.get("hero", {}))
        for m in man.get("monsters", []):
            for v in (m.get("sprites") or {}).values(): sprites.extend(v if isinstance(v, list) else [v])
    for rel in sorted(set(sprites)):
        fname = os.path.basename(rel)
        code = fname[:2].upper()
        if code in ("BA", "SO", "PA", "NE", "DZ", "AS", "DR"):
            srcp = f"data\\global\\chars\\{code}\\{fname}"
            code_dir = f"chars\\{code}"
        else:  # monstres : FA, ZM, SI...
            srcp = f"data\\global\\monsters\\{code}\\{fname}"
            code_dir = f"monsters\\{code}"
        n, _ = extract([srcp], os.path.join(out, "dcc", fname))
        if not n:  # repli : autres archives connues
            for alt in [f"data\\global\\{code_dir}\\{fname}"]:
                n, _ = extract([alt], os.path.join(out, "dcc", fname))
        ok += bool(n); skip += (not n)
        if not n: print(f"!! sprite absent : {fname}")

    # 4) Sons (best effort)
    if "d2sfx" in archives:
        for dest_name, cands in SFX_CANDIDATES.items():
            n, _ = extract(cands, os.path.join(out, "sfx", dest_name))
            ok += bool(n); skip += (not n)

    print(f"\nTerminé : {ok} fichiers extraits, {skip} manqués.")
    print("Lancez ensuite :  python3 -m http.server 8077 --directory ../demo")
    print("Puis ouvrez http://localhost:8077/menu.html")

if __name__ == "__main__":
    main()
