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

---

# Vecteurs ORACLE — `oracle/` (étape 19a, 2026-10-25)

14 saves `.d2s` **générés par le binaire oracle** `d2w-oracle/d2s-file-generator`
(dépôt GPL-3.0 exécuté HORS arbre : ses octets = DONNÉES de validation, aucun
code copié dans d2d2s ; les cas de test du générateur proviennent du jeu réel
LoD v1.14d). Legacy 0x60, SC sauf mention HC — layout « retail 1.14d » :
stats préfixées « gf » dès l'octet 765, squelette items final
`JM+0 / JM+0 (corpse) / jf nu (merc vide) / kf+0`. Testés par
`tests/oracle_saves.rs` (parse strict, quêtes/waypoints/NPC/stats par
paramètres, round-trip octet-à-octet).

| Fichier | Paramètres (classe, mode, niveau, difficulté terminée, or) | SHA-256 |
|---|---|---|
| `AmaFresh.d2s` | Amazone, SC, 1, NONE, 0 | `4b310b4b92dd1dc35180dda73088d12504c0a99a2a874a098af14a2f024bc5be` |
| `SorFresh.d2s` | Sorcière, SC, 1, NONE, 0 | `9f41ca2407c3a1a7b9ddba11d98b25cd354729647526efe559b3063527c6b1d9` |
| `Nec-FreshG.d2s` | Nécromancien, SC, 1, NONE, 123 456 | `0fd8ae4cc91ea2c8d9463f04c9c2bab926222bd41eca2e5aa62823cf3dc6b138` |
| `PalMidNorm.d2s` | Paladin, SC, 30, NORMAL, 0 | `63404b96506bbb224a1f2288f1b235e0433fd2934640600c91a59360dc5fe988` |
| `BarMidNorm.d2s` | Barbare, SC, 30, NORMAL, 800 | `5608ecb59af490b5b9a3f1f99662657eec98830ed291be2311088fb6d666e52d` |
| `DruMidNight.d2s` | Druide, SC, 40, NIGHTMARE, 0 | `0c7bf1da168425f322cff3f7867440cc3b474ee26ff632dd0d49d26895079c5d` |
| `AssMidNight.d2s` | Assassine, SC, 40, NIGHTMARE, 424 242 | `abce987913ed77465f7cbe8cf3b9dab17b2f173b957a52d50cafe7b0f4596700` |
| `AmaHellSix.d2s` | Amazone, SC, 60, HELL, 0 | `0da7ae42f29c7b139271bfc14a852ebcb93e386ddd03039cde6f02513282384d` |
| `SorHellGold.d2s` | Sorcière, SC, 60, HELL, 1 000 000 | `088cda3523514f6704f518b934d4a21fa4665cbfbe90e24c5a921e4ebd2dbccb` |
| `NecHellEnd.d2s` | Nécromancien, SC, 99, HELL, 0 | `24d921319c04b6457cfafc6891d4013a268c19dff2f8a811e14a1636d95ab4f6` |
| `PalHellEnd.d2s` | Paladin, HC, 99, HELL, 0 | `f9c9b6df08d062f32ed65e1c94b5ec584c09a2cd43e1ae2202f415163e796a7c` |
| `BarHellRich.d2s` | Barbare, HC, 99, HELL, 2 500 000 (max) | `880e7f37c2a1b20015ef1ed129cb1a5a54beb4b887d4eb77a2cec982ffcbf88b` |
| `DruHellEnd.d2s` | Druide, SC, 99, HELL, 555 555 | `e798f724aa8f70f95b073c4f04bd2a08c2f15ea2e6609f792bd75c17213c6773` |
| `AssHellEnd.d2s` | Assassine, SC, 99, HELL, 0 | `ef9bfb149f7dc3f791c164ceb8a2b90183aab9d78a853ab6af01cf5c2d41ee94` |

Regénérer : binaire `target/release/d2s-file-generator` de l'oracle, réponses
pipées (nom/classe 1-7/SC-HC/niveau/difficulté/or) avec `cwd` = ce dossier —
contraintes min du générateur : NORMAL ≥ 20, NIGHTMARE ≥ 40, HELL ≥ 60.
Les timestamps/map_id sont horodatés à la génération : ne PAS regénérer
à la légère (les SHA-256 ci-dessus seraient périmés).

## Saves ORACLE 19c — profils Barbare (`oracle/Bar19c*`, étape 19c 2026-10-25)

3 saves complémentaires (même générateur GPL-3.0 exécuté hors arbre, mêmes
paramètres que la table 19a — BA/SC/or 0, nom `Testba`, ts/map_id 0) pour le
DIFF de l'ÉCRITURE `d2rules/save_d2s.rs` (cf. `docs/d2s-export-interop.md`) :

| Fichier | Paramètres (classe, mode, niveau, difficulté terminée, or) | SHA-256 |
|---|---|---|
| `Bar19cFresh.d2s` | Barbare, SC, 1, NONE, 0 | `2dee66eb1090329f52ac5cb656a4bf8cd93c5f406640ab79b8f93917d3085761` |
| `Bar19cNight.d2s` | Barbare, SC, 30, NORMAL, 0 | `b9f433cb2b7d299c0d75ca76a25f38222448319231410c15394d53eac55458da` |
| `Bar19cHell.d2s` | Barbare, SC, 99, HELL, 0 | `c4fa7a0a28a389d284142ece0fdceaa74521325b5db2459b039dd1503ade7a4f` |
