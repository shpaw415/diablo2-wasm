# impl-drlg — W01 · W04 · W02 phase 1 (+ bonus maze) : lecteur de tables, graphe Vis/Warp, DRLG wilderness seedé

> 2026-10-22 · Chantier XL d2game (`engine-rs/crates/d2game/`) — brief council
> `finish-jeu`/`finish-monde` (items **W01, W04, W02 phase 1**, bonus maze).
> Référence : `docs/parity/finish-monde.md` (W01-W04), `evidence/MAP-SPEC*.md`,
> `docs/GAME-DATA.md`, proto `tools/bloodmoor_build.py`, Go `~/workspace/opendiablo2`.
> **Périmètre respecté** : uniquement `d2game` + `assets/` (2 nouvelles tables
> extraites) + ce rapport. Zéro édition de `d2rules`, zéro commit git.

## Ce qui a été livré

| Item | Livrable | Fichiers |
|---|---|---|
| **W01** | Lecteur runtime des tables de niveaux : parseur TSV générique + `LevelTables` (levels.txt), `LvlTypes`, `LvlPrest`, `LvlMaze`, `LvlWarp` | `src/levels.rs` (nouveau, ~690 l. avec tests) |
| **W04** | Graphe de transitions `Vis0-7`/`Warp0-7` non orienté, dédoublonné, réciproques enregistrées + intégration moteur (résolution par id, voisins exposés) | `src/levels.rs` (`WarpGraph`) + `src/game.rs` |
| **W02 p1** | DRLG wilderness **seedé** en Rust : port fidèle de `bloodmoor_build.py` (Blood Moor 72×72 depuis 23 presets réels embarqués) | `src/drlg.rs` (nouveau, ~1000 l. avec tests) + `game.rs` |
| **Bonus** | Squelette génération maze : plan de rooms seedé (grille LvlMaze, arbre couvrant, preset LvlPrest tiré par room) | `src/drlg.rs` (`plan_maze`/`MazePlan`) |
| Tables | `LvlMaze.txt` (3 463 o) et `LvlWarp.txt` (5 244 o) extraits de `engine/patch_d2.mpq` (`data\global\excel\`) ce cycle via StormLib | `assets/LvlMaze.txt`, `assets/LvlWarp.txt` |

## W01 — lecteur runtime (`levels.rs`)

Parseur TSV générique (en-tête nommé → index de colonnes, lignes expanders
`Expansion` ignorées, entiers lenients) + les 5 tables :

- **`patch_levels.txt`** : 137 records avec Id (Null + 136 niveaux ; l'audit
  disait « 135 + Null » — recompté ce jour : 137, sans doublon d'Id). Champs
  consommés : Id/Name/LevelName/Act/Size/DrlgType/LevelType/Vis0-7/Warp0-7/
  MonDen/mon1-10/Waypoint/Quest/LevelWarp/EntryFile/Portal.
- **`LvlTypes.txt`** : 36 types (37 lignes − expander) ; type 2 (Blood Moor)
  = 26 DT1 déclarés.
- **`LvlPrest.txt`** : 1 092 rows, 1 090 avec `Def` ; **1 088 portent au moins
  un DS1**, 2 sont des marqueurs sans aucun fichier (`File1..6` tous « 0 »),
  6 autres n'ont qu'un `File` secondaire. **Aucun record `LevelId=2`** —
  re-vérifié : la Blood Moor n'est PAS assemblée depuis LvlPrest par le retail
  (conforte `finish-monde.md` §2 W02).
- **`LvlMaze.txt`** (extrait ce cycle) : 82 records ; ex. Cave 2 (level 9) =
  4 rooms, grille 24×24, Merge 500.
- **`LvlWarp.txt`** (extrait ce cycle) : 88 warps ; types 0-3 = arêtes
  wilderness (Cliff/Floor L/R), 4-5 = Cave Up/Down, chaque type référencé
  par les colonnes `Warp*` de levels.txt existe (testé).

Le moteur branche la table quand l'asset `assets/patch_levels.txt` est fourni
(`GameCore::provide(LEVELS_ASSET, …)`) ; illisible ⇒ note + repli manifest
intégral (jamais fatal).

## W04 — graphe Vis/Warp (`levels.rs::WarpGraph`)

Graphe **non orienté**, une arête par paire (from, to), slot d'origine et type
LvlWarp conservés en métadonnées, réciproque enregistrée quand la table ne la
déclare pas (testée sur table synthétique à sens unique).

**Constat structurant (nouveau, prouvé par les tests)** : levels.txt ne contient
PAS l'adjacence entre niveaux outdoor. Les slots `Vis3-6` des wilderness portent
leurs caves/tours (ex. Blood Moor → Den of Evil sur 4 slots, types LvlWarp 0-3
= les 4 arêtes), jamais les wilderness voisins :

- composante {Blood Moor(2), Den of Evil(8)} ; composante {Cold Plains(3),
  Cave 2(9), Cave 2 Treasure(13)} ; Stony Field(4)/Dark Wood(5) → Cave 3(10) ;
  Black Marsh(6) → {Tower(20), Hole(11)} ; Tamoe(7) → Cave 5(12) ;
- le Town (1) n'a **aucun** lien Vis/Warp dans la table ;
- la chaîne 1→2→3→4→5→6→7 (chemin critique de l'acte I) vit dans le layout
  outdoor du DRLG retail (D2Common), pas dans les données — c'est pourquoi le
  port garde la paire town↔bloodmoor portée par le manifest (`warp` du bloc,
  positions/entry incluses) et que le graphe table ne la remplace pas.

Intégration moteur (`game.rs`) :
- `resolve_level_def` accepte les clés par id de table (`"2"`, `"level:2"`)
  → bloc manifest portant ce `levelsId` : les transitions peuvent désormais
  viser un id levels.txt, pas seulement une clé manifest ;
- `level_info_json` ajoute `"neighbors":[{id,name},…]` (append-only) pour le
  HUD/debug ;
- le manifest historique (clés, warps géométriques) reste l'autorité quand la
  table est absente — zéro régression (tests dédiés).

## W02 phase 1 — DRLG wilderness seedé (`drlg.rs`)

Port fidèle de `tools/bloodmoor_build.py` (pipeline §4 de
MAP-SPEC-BLOODMOOR) : pools de cellules réelles remappées vers l'espace DT1
unifié (8 conteneurs, ordre = liste `dt1` du manifest bloodmoor), sol d'herbe
brassé sur le canevas 72×72, anneau forestier (28 chunks `bord*`), features
(waypoint, 2 falaises `stnclf6b`, rivière 2×2, 4 clairières), dispersion
(420 arbres à densité ceinturée 0.85/0.30/0.12, 90 rochers, 50 taches de
pierres), validation (sol 100 % plein, 0 indice hors bornes).

Différences assumées vs proto Python :
- **RNG** : SplitMix64 (`d2rules/src/rng.rs`) au lieu du MT19937 Python —
  l'algorithme de composition est identique, les tirages diffèrent donc de
  `assets/map/bloodmoor.ds1` (figé). Même graine ⇒ même carte, à jamais.
- **Embarqué** : les 23 presets DS1 réels (d2exp.mpq, v1.12) sont
  `include_bytes!` dans `drlg.rs` (~120 Ko) — le DRLG marche natif et WASM
  sans nouvelle requête JS ; les comptes de tuiles viennent des DT1 réels
  décodés (jamais codés en dur : [21,52,144,41,45,13,3,13] = 332, vérifié).
- **En mémoire** : la génération produit un `Ds1` synthétique consommé tel
  quel par `compose::compose_seeded` (rendu + collision 360×360 : un seul
  chemin de code, pas de DS1 intermédiaire écrit).

Lecteur preset dédié (tags 1/2/3 → 4/6/8 couches — `ds1::decode_ds1` lit 6
couches fixes, les clairières `bord6-9` sont tag3). Faits découverts au
passage : `waypoint.ds1` fait 7×7 réel (les autres chunks 9×9) ; la couche
ombre des presets est vide partout (confirmé §7 MAP-SPEC).

**Intégration (`game.rs::load_level`)** : quand les tables sont branchées ET
que le record levels.txt du niveau (`levelsId` du manifest) déclare
`DrlgType=3` ET que `drlg::supports(key)` (phase 1 : Blood Moor), la carte est
GÉNÉRÉE seedée (`drlg::level_seed(game_seed, clé)`, `game_seed` réglable via
`set_game_seed`, 0 par défaut = cartes stables par boot) ; tout échec DRLG ⇒
note + repli DS1 figé. Sans tables : comportement historique intégral.

## Tests (tous verts sur le périmètre du chantier, `cargo test --workspace`)

**d2game : 62 tests verts** — lib 59 (38 d'avant-chantier + **21 nouveaux** :
levels 9, drlg 9, intégration moteur 3) + proof 3. Zéro échec, zéro test
historique modifié.

- **W01** : parse des 5 tables réelles (137/36/1092/82/88 records), champs
  spot (Blood Moor DrlgType 3/mon1-3/MonDen 520, Den of Evil Vis0=2/Warp0=4…),
  rejets propres (garbage, table trop petite).
- **W04** : cohérence sur les **9 niveaux A1 (ids 1-9)** — cibles Vis toutes
  existantes, liens 2↔8 / 3↔9 / 9↔13 symétrisés, graphe simple (pas de
  doublon), composantes attendues, town sans lien, types Warp ⊆ LvlWarp,
  réciproque auto sur table à sens unique.
- **W02** : banque 23 presets v18 (7×7 à 9×9, tags 1/2/3), mapping tg1,
  **déterminisme** (même graine ⇒ mêmes couches ; graine ≠ ⇒ carte ≠ ; graine
  par partie ⊕ clé de niveau), carte saine (72×72, 39 collages, sol plein,
  ombre vide, >5 000 remaps, 0 oob), compose réel (grille 360×360, ratio
  marchable de plaine, entrée warp (137,147) avec tuile marchable à ≤ 10
  sous-tuiles, pixels peints), ceinture plus dense au bord.
- **Bonus maze** : plan déterministe, 4 rooms posées sans doublon, arbre
  couvrant (BFS connexe), ouvertures réciproques N↔S/E↔W, presets dans la
  planche, erreurs propres.
- **Intégration moteur** : boot bloodmoor avec tables ⇒ notes « tables de
  niveaux branchées » + « drlg seedé actif », rendu non vide ; même graine ⇒
  grille de collision identique, graine ≠ ⇒ monde ≠ (et stable) ; fallback
  sans tables = notes absentes + DS1 figé ; **warp aller-retour complet
  town→bloodmoor(générée)→town** avec 5-8 monstres marchables ; résolution
  `"2"`/`"level:2"` + switch par id + voisins Vis/Warp dans levelInfo.
- **Non-régression** : les 143 tests d'avant-chantier restent verts (chemins
  historiques intacts — vérifié par les suites existantes inchangées).

## Ce qui reste du XL (W02/W03 complet)

1. **Assemblage maze** (DrlgType 1) : `plan_maze` donne le plan seedé ; reste
   le choix DS1 par room (LvlPrest Def ↔ LvlMaze), le collage multi-rooms en
   canevas et les DT1 intérieurs (caves acte I = premier candidat naturel).
2. **Adjacence outdoor** : la chaîne 1→2→…→7 n'est dans aucune table — il
   faut l'authorer (grille d'acte façon od2 `act1_overworld.go`) ou la
   reverse depuis D2Common.dll pour brancher le graphe W04 dessus.
3. **Géométrie des warps** : `LvlWarp.txt` est parsé (SelectX/Y, ExitWalk,
   Direction) mais non consommé — les zones warp restent manifest ; le
   branchement passerait par les tuiles layer 5 des DS1 (warp.dt1) posées
   par les presets.
4. **Autres wilderness** : `drlg::supports` est câblé pour bloodmoor ; un
   niveau de plus = banque de presets + tileset unifié + entrée `PlainSpec`
   (la structure isole déjà ces listes).
5. **W03** (PL2 actes 2-5, DT1 manquants) : inchangé — hors périmètre de ce
   cycle.

---
*Preuves : `cargo test -p d2game` 62/62 vert ce jour ; les autres crates du
workspace restent verts (d2animdata 6, game_files 5+3, d2dc6 8+3, d2dcc 4,
d2missiles 12, d2mpq 7+1). État `d2rules` ce jour : chantier PARALLÈLE en
cours (affixes/loot/skills_tree — autres membres, périmètre interdit) avec
21 tests en vol au moment de la mesure ; les 25 tests des modules stables
(world/save/path/rng/skills/data) passent. Tables extraites via
`tools/stormlib_read.py` sur `engine/patch_d2.mpq` ; presets/décodeurs
validés par round-trip compose.*
