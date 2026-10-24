# Vecteurs de test SYNTHÉTIQUES — format .d2s

Fichiers **fabriqués** par `make_vectors.py` depuis la spec d'`evidence-formats/d2s/evidence.md` — aucune donnée d'une vraie sauvegarde. Sert à tester le parseur, pas à valider la spec.

| Fichier | Contenu | Attendu du parseur | SHA-256 |
|---|---|---|---|
| `minimal_v0x60.d2s` | personnage minimal 1.10+ (9 stats, skills, 0 item) | exit 0, checksum VALIDE, stats rechargées identiques | `2da62046bb21d78e3814ecb4c0210e24ebdb15cfec7ece3dd51d050bb7821198` |
| `bad_checksum.d2s` | comme minimal, checksum = 0xDEADBEEF | exit 2, checksum INVALIDE, structure sinon OK | `712e969ee92ea7faf2effb840e9e8efc7f0d664e980090032a4cdb29ef0bb846` |
| `corrupt_magic.d2s` | signature 0x12345678 | warning signature, exit 2 (checksum recalculé différent) | `a06231fa51046c183c4f494f6e09850f174d8cc429b6529778f1da8458ff0c9d` |
| `version_1_09_gf.d2s` | version 92, stats au format 'gf' v1.09 | exit 0, branche 1.09, str=20 level=12 gold=1000 | `2ab12267ba7891d75323c631a68f2a48b51e5492a479012cdf5bbd72a67bd00d` |
| `magic_item.d2s` | 1 anneau magic (pref 5, suff 3, mods id 9 et 0) | exit 0, item parsé, mods [9, 0], corpse vide | `2770ac4a8d179fa37dd515a00ce0165649c82f134a4cc5bf838e4c57a2c799f1` |
| `socketed_armor.d2s` | casque cap normal socketé, 2 gemmes enfants | exit 0, item cap defense=33 sockets=2, corpse vide | `14641c246e6b8a8b4af4ebf637de6b9aca29737bb18adbc8634127f0304d964f` |
| `truncated.d2s` | coupé pendant le flux de stats | exit 1, « fin de fichier prématurée » | `9767fc55e5f9ce5e78139218873457cb2a1df58725bb8ecfa88a07db4020ddf6` |

Regénérer : `python3 make_vectors.py` (déterministe). Tester : `bash test_vectors.sh`.
