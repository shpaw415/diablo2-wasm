# Implémentation — AnimData.d2 (audit od2-features-formats #19 → résolu)

> 2026-10-22. Manque n°1 de l'audit (`docs/parity/od2-features-formats.md` l.19) :
> FramesPerDirection/Speed/events par token viennent de `AnimData.d2`, pas du COF.

## Fait

- **Extraction du vrai fichier** : `data\global\animdata.d2` (pas dans `excel/` — la
  sonde par hash `d2tool probe` confirme le hit dans `d2data.mpq`, `d2exp.mpq` et
  `patch_d2.mpq`). Extrait de `patch_d2.mpq` (priorité la plus haute ; identique
  SHA-256 à la version de `d2exp.mpq` : `36cb704b85a7a478…`, 570 304 o) via
  `tools/stormlib_read.py` → **`assets/animdata/AnimData.d2`**.
  La version `d2data.mpq` (base, 411 104 o) est l'ancienne : le patch v1.12 ajoute
  ~1 000 records.
- **Port Rust** : nouvelle crate **`engine-rs/crates/d2animdata`** (recommandation
  de l'audit §3.1), port byte-faithful du paquet Go `d2animdata`
  (`animdata.go`/`block.go`/`record.go`/`events.go`/`hash.go`) :
  - 256 blocs × ≤ 67 records ; record 160 o = nom 8 o (NUL requis, Go l.146) +
    u32 frames_per_direction + u16 speed + 2 pad + 144 events ;
  - `hash_name` (somme des octets uppercasés mod 256) — vérifié : les 3 558
    records du vrai fichier sont dans le bloc que le hash prédit ;
  - validations Go conservées : > 67 records/bloc → erreur, nom non terminé →
    erreur, fichier non consommé exactement → `LengthMismatch` ;
  - `get_records` (tous les homonymes, ordre fichier), `get_record` (le **dernier**
    gagne, sémantique Go), `get_by_token` (chemin hash, insensible à la casse),
    `fps()` = 25 × speed ÷ 256, `frame_duration_ms()`, `event()/triggered_events()`
    (events.go : 0 none, 1 attack, 2 missile, 3 sound, 4 skill).
  - Écart volontaire : le champ Go `hashTable [256]byte` (toujours écrit à
    l'index 0 — mort-né) n'est pas porté ; `hash_name` + `get_by_token`
    couvrent l'intention.
- **Branchement minimal** (fallback garanti) :
  - `d2game/src/sprite.rs` : `SpriteSet.animdata : Option<AnimationDataRecord>` +
    `apply_animdata()` ; `frames()` honore le record s'il est branché, sinon
    octets COF (comportement historique) ; nouveaux `fps()`/`frame_duration_ms()`
    (`Some` seulement si animdata).
  - `d2game/src/game.rs` : asset optionnel `assets/animdata/AnimData.d2` —
    fourni au `provide` ⇒ `finalize` le parse et branche chaque set par le stem
    de son COF (`assets/dcc/BAWL1HS.COF` → `BAWL1HS`) ; absent ⇒ rien (aucun
    changement) ; illisible ⇒ note, jeu en repli COF. Le token absent du
    fichier (ex. `BARNUHTH`, la course barbare HTH n'existe pas en 1.12) ne
    branche rien.
  - `demo/game.js` : le chemin est ajouté à la liste **optionnelle** de
    `collectPaths` — fourni au boot quand présent, silencieux sinon.
- **Pourquoi c'est le bon fix** (preuve terrain) : les octets speed des COF
  retail sont incohérents — `FAWLHTH.COF` et `SIWLHTH.COF` portent **0** là où
  AnimData.d2 dit **256** (25 fps) ; `BAWL1HS.COF`/`ZMWLHTH.COF` coïncident
  (168 / 104). Valeurs officielles v1.12 des sets du MVP :

| Token | frames/dir | speed | FPS | durée/frame |
|---|---:|---:|---:|---:|
| BAWL1HS | 8 | 168 | 16,406 | 60,95 ms |
| FAWLHTH | 10 | 256 | 25 | 40 ms |
| ZMWLHTH | 12 | 104 | 10,156 | 98,46 ms |
| SIWLHTH | 9 | 256 | 25 | 40 ms |

## Tests (`cargo test --workspace`, depuis `engine-rs/`)

- `crates/d2animdata` — **12 verts** (6 unitaires + 5 intégration + doctest) :
  1. `parse_real_v112_file` — le vrai fichier : 3 558 records, 3 529 tokens
     (29 homonymes), 256 blocs dont 1 plein à 67 et 120 vides, consommation
     exacte.
  2. `known_game_tokens_lookup` — BAWL1HS 8/168, FAWLHTH 10/256, ZMWLHTH
     12/104, SIWLHTH 9/256, `BARNUHTH` absent, `get_by_token` insensible à la
     casse (`get_record` reste à match exact, comme la map Go).
  3. `hash_placement_holds_for_every_record` — 3 558/3 558 bloc == `hash_name`.
  4. `coherence_with_independent_golden_dump` — parité record à record avec un
     dump Python indépendant (`tests/golden/animdata_v112.txt`, 3 558 lignes,
     events déclenchés compris).
  5. `coherence_with_opendiablo2_go_testdata` — le `testdata/AnimData.d2` du
     repo Go lui-même parse avec les mêmes 3 529 tokens et les mêmes valeurs
     par token (multiset ; seul l'ordre intra-bloc des 2 homonymes de
     `VMGHHTH` diffère — fichier re-marshalé côté Go).
  6. + erreurs Go reproduites (68 records, nom sans NUL, trailing bytes,
     troncature) et sémantique dernier-homonyme.
- `d2game` — +4 verts : `animdata_record_takes_precedence_over_cof_fallback`
  (priorité record > COF, fps/durée), `animdata_provided_branches_official_
  frames_and_speed` (jeu complet réel : héros et FA/ZM/SI branchés, note
  émise, tick+render OK), `animdata_corrupt_falls_back_to_cof_with_note`.

### Résultats

```
$ cargo test -p d2animdata
test result: ok. 6 passed (lib) · 5 passed (game_files) · 1 doctest
$ cargo test --workspace
… 143 passed, 0 failed, 1 ignored (test wasm préexistant) — dont d2game 38/38
```

Les 109 tests de la baseline restent verts. Note de session : pendant ce
travail, une autre session refactorait simultanément `compose.rs`/`wasm.rs`
(mêmes fichiers) — deux tests de switch (`switch_to_bloodmoor`,
`level_switch_carries_hero_stats`) ont transitoirement échoué sur cet
en-cours, puis repassent dans l'arbre final ; le périmètre animdata n'a
jamais été touché par ces échecs (champ Optionnel + notes seulement).

## Suivi (phase 3)

- Consommer `fps()`/`frame_duration_ms()` dans la boucle d'animation de
  `d2rules` (aujourd'hui `FRAME_MS = 100 ms` constant, doc lib.rs l.14) —
  branchement volontairement laissé hors de ce change (contrainte « ne rien
  casser », le timing vit dans d2rules qui ne dépend pas des sprites).
- `triggered_events()` est prêt pour les SFX d'attaque (frames son) et les
  spawns de missiles.
