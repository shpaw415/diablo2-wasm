# impl-7classes — 4 nouvelles classes jouables (AM/AS/DZ/NE)

**Date :** 2026-10-23 · **Chantier :** passage de 3 à 7 classes jouables (modes NU/WL/A1/DD)
**Statut :** livré — sprites extraits, manifest 7 héros, starting_skills réels, tests verts (275/275 workspace).

---

## 1. Sprites — extraction des 4 classes manquantes

Pipeline du skill `shortcut-sprites-perso-d2` : sonde par hash (`tools/mpq_mini.py`),
extraction `engine-rs/target/debug/mpq_extract`, validation `dccdump`, composition `dcc_render`.

### Tokens MPQ réels (correction du skill)

| Classe | Clé manifest (UI) | Token chemins MPQ | Archive |
|---|---|---|---|
| Amazon | `AM` | `AM` | d2char.mpq |
| Assassin | `AS` | **`AI`** (pas AS !) | **d2exp.mpq** |
| Druid | `DZ` | **`DZ`** (pas DR !) | **d2exp.mpq** |
| Necromancer | `NE` | `NE` | d2char.mpq |

La sonde par hash sur `data\global\chars\<tok>\cof\<tok>nuhth.cof` dans d2exp.mpq ne
rend que `{ai, dz, ne}` : le token interne de l'Assassin est `AI`, celui du Druide `DZ`
(charstats dit « Assassin »/« Druid » en clair — la table ne donne pas le token).

### Weapon class des COF = wclass de l'arme de départ (charstats item1 → patch_weapons)

| Classe | Arme (item1) | wclass | COF extraits | RH comp |
|---|---|---|---|---|
| AM | `jav` Javelin | **1HT** | `AM{NU,WL,A1}1HT.COF` + `AMDDHTH.COF` | JAV |
| AS | `ktr` Katar | **HT1** | `AI{NU,WL,A1}HT1.COF` + `AIDDHTH.COF` | KTR |
| DZ | `clb` Club | **1HS** | `DZ{NU,WL,A1}1HS.COF` + `DZDDHTH.COF` | CLB |
| NE | `wnd` Wand | **1HS** | `NE{NU,WL,A1}1HS.COF` + `NEDDHTH.COF` | WND |

DD = wc HTH, corps entier couche TR seule, 1 frame (règle §7.2 du skill) — idem BA/PA/SO.

### Résultat

- **114 nouveaux fichiers** dans `assets/dcc/` (16 sets COF+DCC), noms `<TOK><MODE><LAYER>.dcc`
  (même pattern que PA/BA/SO). `baseWClass` charstats = `hth` pour les 7 classes : c'est le
  wclass de l'ARME de départ qui pilote le COF, pas `baseWClass` (mains nues).
- Couches notables : l'Assassin a **LH** (2e griffe) + SH (9 couches NU/WL/A1) ;
  DZ NU/WL sans S2 (manquant retail, marqué `missingLayers`) ; LG hors-COF extrait
  pour AM/NE A1 et AI/DZ DD (piége §7.3).
- **Piége retail** : `AIDDHTH.COF` déclare 8 dirs, `AIDDTR.dcc` en a 16 → `dcc_render`
  rejetait la composition. Le jeu n'est PAS concerné (`SpriteSet::compose` mappe chaque
  couche avec son propre nombre de dirs, clampé par le COF). Ajout d'un flag opt-in
  `--lenient-topology` à `dcc_render` (défaut reste strict) pour les planches de validation.
- **Validation** : `tools/verify_dcc.py` exit 0 — 258/258 DCC décodent, manifest
  cohérent dans les deux sens (`missingLayers` exacts).
- **Planche visuelle** : `assets/dcc/modes-grid-7classes.png` (4 classes × NU/WL/A1/DD,
  armes de départ et boucliers visibles — javelot, griffe+bouclier, massue, baguette).

## 2. manifest.json — 7 entrées `heroes`

`tools/build_manifest.py` étendu (c'est lui qui génère) : table `HEROES` (clé UI, token,
nb modes — 5 pour BA/SOR/PA avec RN, 4 pour AM/AS/DZ/NE), `COF_WC` par classe,
`stats_block()` lit `patch_charstats.txt`/`patch_weapons.txt`/`patch_armor.txt`.

- Clé = code UI (`AM/AS/DZ/NE`), champ `class` = token chemins (`AM/AI/DZ/NE`) —
  même convention que `SOR`→`SO`. Alias `walk/idle/attack/death` copiés des modes
  (structure BA/SOR/PA). Le menu démo lit `heroes{}` : data-driven, rien à changer.
- Stats réelles : `life = vit + hpadd`, `mana = int`, stamina brute ; colonnes
  « in fourths » /4 (`lifePerLevel`, `lifePerVitality`, `manaPerLevel`,
  `manaPerEnergy` = ManaPerMagic), `statPerLevel` brute, `toHitFactor` brute
  (négatif OK : NE -10) — même sémantique que les blocs BA/SOR/PA existants.
- Armes/boucliers : `startWeapon` (jav 1-5 1ht / ktr 4-7 ht1 / clb 1-6 1hs / wnd 2-4 1hs),
  `startShield` buc (4-6, blockRate 25) pour AM/AS/DZ — NE démarre sans bouclier
  (charstats item2 = hp1).

## 3. starting_skills — la table fait foi

`d2rules/src/skills.rs` : colonne `StartSkill` de charstats v1.12 →
`patch_skills.txt` → missile `srvmissile` seulement s'il existe dans la table
`d2missiles` (10 définitions MVP) :

- **SOR** `fire bolt` → `Fire Bolt`, missile `firebolt` ✓ (3–8 feu, niv 1)
- **NE** `raise skeleton` → `Raise Skeleton` — invocation SANS `srvmissile`
  (summon non câblé au MVP) ⇒ repli mêlée/`arrow` du moteur
- **AM/BA/PA/DZ/AS** : `StartSkill` VIDE dans la table ⇒ aucune compétence de
  départ ; l'attaque de base (mêlée) et le tir (repli `arrow` + dégâts d'arme de
  `hero_ranged_attack`) marchent sans skill. L'ancienne hypothèse MVP
  Bash/Sacrifice n'est pas dans charstats — retirée (tests réécrits en conséquence).

Code inconnu ⇒ vide (jamais de panic) — test existant conservé.

## 4. Tests & non-régression

- `cargo test --workspace` : **275 passed / 0 failed** (baseline 274 + apports).
- Nouveaux goldens (d2rules/tests.rs) :
  - `starting_skills_match_charstats_all_7_classes` — lit le `patch_charstats.txt`
    RÉEL (include_str) et compare 7/7 aux `starting_skills` via `skills_tree`
    (canonisation lowercase → code skills.txt) ;
  - `manifest_carries_seven_playable_heroes` — 7 codes, modes NU/WL/A1/DD avec
    couches, stats charstats (AM 50/15, AS 50/25, DZ 55/20, NE 45/25) chargées
    dans un `World` réel ;
  - `melee_classes_default_melee_and_fallback_arrow` mis à jour (BA/PA sans skill,
    repli `arrow` inchangé ; NE naît avec Raise Skeleton).

## Restant (hors périmètre de ce cycle)

- Effets des skills (summon Raise Skeleton, dégâts Bash/Sacrifice si réintroduits) ;
- modes RN pour AM/AS/DZ/NE (course — BA/SOR/PA seuls les ont) ;
- skills_tree : 210 skills, synergies (chantier parallèle impl-regles-skills —
  `skills_tree.rs`/`calc.rs`/`xp.rs` déjà en place et consommés par le golden 7/7) ;
- câblage Menu → 7 cartes (thread « Écran de sélection 7 classes »).

## Fichiers touchés

- `assets/dcc/` : +114 fichiers AM/AI/DZ/NE, `modes-grid-7classes.png`, `extract-report.json` (fusionné)
- `assets/manifest.json` : +4 héros (additif, BA/SOR/PA intacts)
- `tools/extract_modes.py` (+16 sets), `tools/build_manifest.py` (HEROES 7, stats_block), `tools/mode_grid.py` (heroes7)
- `engine-rs/crates/d2dcc/src/bin/dcc_render.rs` : flag `--lenient-topology` (défaut strict)
- `engine-rs/crates/d2rules/src/skills.rs` + `tests.rs` : table-fait-foi (StartSkill)
- PAS de commit git (consigne).
