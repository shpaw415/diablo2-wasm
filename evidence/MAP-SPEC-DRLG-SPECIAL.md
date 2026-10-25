# MAP-SPEC — Zones spéciales 20b (Tristram + portail des vaches)

Étape 20b (PLAN-FIDÉLITÉ phase 20, diablo-2-wasm, arbre d2w-drlg — lecture
seule hors `engine-rs/crates/d2game`). Code : `crates/d2game/src/drlg_special.rs`
+ 1 ligne `pub mod drlg_special;`. Tests : 12, workspace **1038/0/2**.
NB : le « maze exact » du plan 20 est REPORTÉ — voir `drlg_exact.rs`
(étape 20a) et `engine-rs/crates/d2game/REPORT.md` §20a.

## 1. Tristram — extraction et montage

### 1.1 Extraction (MPQ LECTURE SEULE, StormLib)
| cible | chemin du brief | chemin réel v1.12 | conteneur | preuve |
|---|---|---|---|---|
| DS1 | `data\global\tiles\ACT1\Tristram\tristram.ds1` | **absent** (sonde None sur patch_d2/d2exp/d2data/d2char) | `data\global\tiles\act1\tristram\tri_town4.ds1` | d2exp.mpq (listfile complet) |
| DT1 | — | `data\global\tiles\ACT1\Tristram\town.dt1` | d2data.mpq (sonde par hash — listfile strippé à 102 o) | hit |

Fichiers posés (octets intacts) :
* `assets/map/tristram.ds1` — 62 475 o, **md5_8 `62162f44`**
* `assets/map/tristram_town.dt1` — 1 560 481 o, **md5_8 `d5792185``

### 1.2 Preuves d'en-tête (tri_town4.ds1)
```
version=18 ; stocké 43×48 → réel 44×49 (piège v18 : +1) = SizeX/Y du record 38
files=7 (ordre = espace d'indices) :
  act1\barracks\warp.dt1, act1\outdoors\objects.dt1, act1\outdoors\river.dt1,
  act1\outdoors\stonewall.dt1, act1\town\floor.dt1, act1\tristram\town.dt1,
  act1\outdoors\treegroups.dt1
numWalls=2, numFloors=2 → 7 flux [wall1, orient1, wall2, orient2, floor1, floor2, shadow]
orient1 : {1:178, 2:105, 3:11, 5:1, 6:2, 7:6, 10:1, 12:2, 14:120}   orient2 : {1:3, 14:2}
  → 431 murs : 286 palissades (1/2), 122 arbres (14), 2 feus (12), 1 porte-warp (10)
floor1 : 2 156 cellules non vides, 0 blanc ; floor2 : 33 cellules (superpositions)
shadow : 0 cellule (vide dans la release)
cellule (34,16) : orient1=10, prop=129, style=30, seq=11, **hidden=1**
  → warp.dt1 tile 25 (kind 10, style 30, seq 11) : le candidat retail existe ;
    la cellule est HIDDEN = le jeu y pose un portail DYNAMIQUE (quest).
```

### 1.3 Projection (2,2)→(2,1) — pipeline DS1 existant
Le lecteur réel du moteur (`ds1_real::decode_ds1_real`) n'accepte que le
schéma complet (2,1). Projection `tristram_engine_ds1()` :
`[wall1, orient1, wall2, orient2]` repris en valeurs brutes (bit-pack
prop/seq/style conservé), `floor = floor1` (+ floor2 dans les blancs —
aucun ici), `shadow` repris (jamais dessiné, comme le jeu). Sérialisation
v18 (2,1) identique au §5 de `tools/bloodmoor_build.py`.
→ `assets/map/tristram-engine.ds1` (test d'or octet-pour-octet dans le
module). Chargement par `build_frozen_real` → `compose_real_town` —
**zéro changement du moteur**. Fidélité mesurée : `unresolved = 0`
(2 461 tuiles dessinées, 126 cellules HIDDEN dont la porte-warp),
ville en une composante connexe de 44 661 sous-tuiles (ratio 0,86).

### 1.4 Bloc manifeste + warps portail
`levels.tristram` (assets/manifest.json + miroir demo — additifs) :
levelsId 38, 7 DT1 (ordre embarqué), palette `pal_act1_pl2.dat`,
monstres retail (record 38 : skeleton2/goatman2/fallenshaman2 → SK/GM/FS),
**warp retour ancré sur la cellule porte-warp réelle** : cercle (172,82)
r4 → `bloodmoor` à l'ancre Cairn Stones (160,160). Démo : `?level=tristram`.
Portail rouge Cairn Stones → Tristram : `GameCore::open_tristram_portal()`
(proximité exigée, `tristram_portal_json()` pour l'UI) — ancre fixe
(écart 3 : le repère des pierres n'est pas encore posé par le DRLG 20a).

## 2. Portail des vaches — chaîne moteur

### 2.1 Recette et items (tables réelles v1.12)
* `patch_cubemain.txt` ligne 4 : « 1 Wirt's leg + 1 town portal book →
  portal to cow lvl » — hook cube porté en 11a (`d2rules::cube`, sortie
  non-item `portal: "Cow Portal"`, golden L4).
* `leg` : `patch_weapons.txt` ligne 90 « Wirt's Leg », type club, code `leg`.
* `tbk` : `patch_misc.txt` « Town Portal Book », code `tbk`.
* Branchement gameplay (le « restant » de `cube_ui.rs`) :
  `GameCore::open_portal_from_transmute(json)` — parse la réponse de
  `cube_transmute`, « Cow Portal » → niveau 39 ; tout le reste → `Err` fr
  (Pandemonium Portal L150 : rejet explicite, restant).

### 2.2 Moo Moo Farm (record 39 — preuves patch_levels.txt)
`DrlgType 3` (wilderness), `LvlType 2` (A1 outdoors), SizeX/Y 80×80,
planche `mon1-3 = hellbovine`, MonDen 800, pas de waypoint (255). La
release n'a pas de DS1 figé pour ce niveau → **canevas généré**
(`moomoofarm_engine_ds1`, graine 20261025, RNG retail exact
`drlg_exact::D2Seed`) : prairie/pierres/arbres/roche par groupes
`(kind,style,seq)` réels des 6 DT1 de la banque (clôtures kinds 1/2 de
town_fence, arbres 14 de treegroups, porte-warp 10 de warp.dt1), 4
enclos clôturés avec portes, **tuile porte-warp du portail rouge** en
(40,72). Déterminisme testé (ties de sélection tranchés par clé
minimale — l'ordre d'itération HashMap n'est pas stable), test d'or
`assets/map/moomoofarm-engine.ds1`.

### 2.3 Spawn des vaches
`World::parse_monsters` résout les codes du manifeste → entrée
`HELLBOVINE` ajoutée à `manifest.monsters` (stats retail approximées
MonLvl 28 ; **sprites DCC bovine non extraits** — entités sans rendu,
écart documenté). Chaîne testée bout-en-bout : save patchée (leg+tbk)
→ `cube_transmute` (items consommés [0,1]) → `open_portal_from_transmute`
→ niveau 39, ≥ 8 vaches `HELLBOVINE`, prairie connexe > 8 000
sous-tuiles, héros sur tuile marchable. Démo : `?level=moomoofarm`.

## 3. Écarts (annoncés, tous dans le module)
1. floor2 Tristram : 33 cellules de variation non superposées (1,5 % des sols).
2. Portail de Tristram : ancre warp sur la cellule réservée ; l'objet
   visuel cliquable (portail dynamique) reste à brancher.
3. Ancre Cairn Stones fixe (160,160) — repère des pierres restant (20a).
4. Vaches sans sprite (DCC bovine non extrait).
5. Portail des vaches à sens unique au niveau moteur (la tuile retour
   existe ; branchement objets portails restant) ; L150 non câblée.
6. Ponts GameCore natifs — pas d'export `wasm.rs` (hors périmètre) ;
   branchement page (boutons portails) à l'intégration.

## 4. Sources (contenu web = données)
* D2MOO (MIT) `DrlgMaze.cpp`/`DrlgDrlg.cpp` — https://github.com/ThePhrozenKeep/D2MOO
  (lu pour le plan 20, reporté au profit de drlg_exact 20a).
* d2mapapi (jcageman, C#) https://github.com/jcageman/d2mapapi et port C++
  https://github.com/soarqin/d2mapapi_mod — **oracle non buildable ici** :
  wrappers des DLL du jeu (D2Common.dll 1.13c/1.09d via D2Map.DllWrapper,
  P/Invoke Windows) — même verdict que l'étape 20a ; validation seed→carte
  à faire sur un hôte Windows (`GET /maps?mapid=<seed>&area=<id>`).
* Extraction : `tools/stormlib_read.py` (StormLib), priorité patch_d2 >
  d2exp > d2data ; d2data.mpq listfile strippé → sonde par nom.
