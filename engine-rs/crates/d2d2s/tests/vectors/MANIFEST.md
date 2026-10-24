# Vecteurs de test SYNTHÉTIQUES — format .d2s

Fichiers **fabriqués** par `make_vectors.py` depuis la spec d'`evidence-formats/d2s/evidence.md` — aucune donnée d'une vraie sauvegarde. Sert à tester le parseur, pas à valider la spec.

| Fichier | Contenu | Attendu du parseur | SHA-256 |
|---|---|---|---|
| `minimal_v0x60.d2s` | personnage minimal 1.10+ (9 stats, skills, 0 item) | exit 0, checksum VALIDE, stats rechargées identiques | `a737317b746645f6bdc605406eaf716dda8127d453d0a56a6a524cabdbde1e6c` |
| `bad_checksum.d2s` | comme minimal, checksum = 0xDEADBEEF | exit 2, checksum INVALIDE, structure sinon OK | `712e969ee92ea7faf2effb840e9e8efc7f0d664e980090032a4cdb29ef0bb846` |
| `corrupt_magic.d2s` | signature 0x12345678 | warning signature, exit 2 (checksum recalculé différent) | `a06231fa51046c183c4f494f6e09850f174d8cc429b6529778f1da8458ff0c9d` |
| `version_1_09_gf.d2s` | version 92, stats au format 'gf' v1.09 | exit 0, branche 1.09, str=20 level=12 gold=1000 | `0cfd0da3d19586aaab35a244c9d61f8b7f62a86452f9c768c4ae38b933bc40cd` |
| `magic_item.d2s` | 1 anneau magic (pref 5, suff 3, mods id 9 et 0) | exit 0, item parsé, mods [9, 0], corpse vide | `77f5f8187ea84969cb0c1643e31b183ce1ec976f88ba095ed85d862ec462115a` |
| `socketed_armor.d2s` | casque cap normal socketé, 2 gemmes enfants | exit 0, item cap defense=33 sockets=2, corpse vide | `856c080b347881c40bcb4c0866a588ed35d3377cf01694fc2a3c4979776baa5b` |
| `truncated.d2s` | coupé pendant le flux de stats | exit 1, « fin de fichier prématurée » | `0655ea2a3c009cc286cccfdce2166f4fbfdd931b498ec7cb489a163986c84c61` |

Regénérer : `python3 make_vectors.py` (déterministe) **puis recalculer les
checksums (formule corrigée §VALIDATION RÉELLE : zéros dans le champ, rotation
appliquée) et les SHA-256 ci-dessus** — fait le 2026-10-24 après la validation
sur Javazon.d2s réel. Tester : `bash test_vectors.sh`.
