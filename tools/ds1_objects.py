#!/usr/bin/env python3
"""ds1_objects.py — probe DS1 v18 OBJECT section (waypoints, cages, NPC…).

Parse la fin d'un DS1 v18 après les 6 couches (même en-tête que
crates/d2game/src/ds1.rs) : section PATH, section OBJECTS
(id, x, y, flags — stride 4 ints v15+), section SUBSTITUTION.
Usage : python3 tools/ds1_objects.py assets/map/TownN1.ds1
"""
import struct
import sys


def i32(d, at):
    return struct.unpack_from("<i", d, at)[0]


def main(path):
    d = open(path, "rb").read()
    version = i32(d, 0)
    w = i32(d, 4) + 1  # v18 trap : taille réelle = stockée + 1
    h = i32(d, 8) + 1
    filecount = i32(d, 20)
    p = 24
    for _ in range(filecount):
        e = d.index(0, p)
        p = e + 1
    p += 8  # paire meta (2 ints)
    n = w * h
    p += 4 * 6 * n  # 6 couches
    print(f"{path}: v{version} {w}x{h} tuiles ({w*5}x{h*5} sous-tuiles), "
          f"{filecount} fichiers, couches finies @{p}")

    # --- OBJECTS section (réf. OpenDiablo2 d2ds1/ds1.go loadObjects) -------
    # stride 5 ints : type, id (objects.txt), x, y (SOUS-tuiles), flags.
    nobj = i32(d, p)
    p += 4
    objs = []
    for _ in range(nobj):
        typ, oid, x, y, flags = (i32(d, p), i32(d, p + 4), i32(d, p + 8),
                                 i32(d, p + 12), i32(d, p + 16))
        p += 20
        objs.append((typ, oid, x, y, flags))
    print(f"OBJECTS : {nobj} objet(s)")
    for typ, oid, x, y, flags in sorted(objs):
        print(f"  type={typ} id={oid:<4} x={x:<4} y={y:<4} flags=0x{flags & 0xFFFFFFFF:08x}")
    # fin des objets ; (substitutions/NPC non lus — hors besoin)
    print(f"fin OBJECTS @{p} ; restant dans le fichier : {len(d) - p} octets")


if __name__ == "__main__":
    main(sys.argv[1])
