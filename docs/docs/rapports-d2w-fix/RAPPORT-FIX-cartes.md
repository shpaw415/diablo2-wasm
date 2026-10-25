# RAPPORT-FIX — Cartes : le Camp des Rousses (ville 1) rendu comme une forêt

Arbre : `/home/agent/workspace/d2w-fix-cartes/` — mission d2w-fix-cartes (diablo-2-wasm).
Contraintes respectées : **ni le HUD, ni les personnages, ni `demo/game.js`** (vérifié `node --check`, aucune modification).

---

## 1. Cause racine (prouvée sur les données réelles)

Le moteur décodait `TownN1.ds1` avec un schéma **faux**, hérité du prototype
`tools/map_proto2.py` :

1. **Mauvais schéma de flux.** Un DS1 v18 réel porte, après la liste de
   fichiers, `numWalls=2` puis `numFloors=1` (le proto les appelait
   « meta (2, 1) »). Les 6 flux sont
   `[wall1, orientation1, wall2, orientation2, floor1, shadow1]` —
   le moteur les traitait comme `L0=rues, L1=arbres, L4=sol, L5=warp`.
2. **La « forêt » est un artefact.** `L1` du proto est le flux
   d'*orientation* (codes 1-15). Ses ~380 cellules ont été rendues comme
   des indices absolus de tuiles dans la fenêtre `out_treegroups.dt1`
   → forêt dense dans tout le camp.
3. **Cellules bit-packées**, pas 4 octets (référence OpenDiablo2
   `d2common/d2fileformats/d2ds1/tile.go`) : `prop` bits 0-7, `seq` 8-13,
   `unk1` 14-19, `style` 20-25, `unk2` 26-30, `hidden` bit 31.
4. **Décodeur DT1 désapparié.** Le record standard `k` (table à
   `276 + 96k`, `bodyPosition@272`) porte TOUTE la tuile `k` :
   `dir@+0, type@+20, style@+24, seq@+28, rfi@+32` ET ses blocs
   `+72/+76/+80`. L'ancien lecteur (base 252) prenait les blocs du record
   `k-1` : pixels décalés d'une tuile, tuile 0 fantôme vide, dernière
   tuile du conteneur jamais lue.
5. **Bug `resolve_container`** : `outdoors/objects.tg1` tombait sur
   `town_objects.dt1` (règle du suffixe) — `out_objects.dt1` (tentures,
   props du camp) n'entrait jamais dans le pool.

### Appariement cellule → tuile (la règle du jeu, prouvée 100 % sur TownN1)

- **sol** : candidats `(style, seq)` parmi les tuiles `type == 0` ;
- **mur** : candidats `(type == code d'orientation du flux, style, seq)`
  parmi toutes les tuiles ;
- `prop` = départ de variation (`prop % candidats`), `hidden` = non
  dessinée.
- Mesuré : **sols 2337/2337, murs 346/346 (+43 cachés)**.

### Structure réelle du camp (dans les données depuis toujours)

2337 sols (herbe style 5 = 1366 cellules, chemins, rivières) ; murs :
**tentures (type 15) ×19** au centre x[24-39] y[8-15], **feu/props
(type 12) ×16**, **palissades (types 1/2) ×184 visibles**, **arbres
(type 14) ×118** (bosquet NO + bordure, ≈ 5 % de la carte), porte-warp
(type 10) à la sortie Est. La forêt généralisée n'a jamais existé dans
les données.

## 2. Fichiers modifiés (précis)

| Fichier | Contenu |
|---|---|
| `engine-rs/crates/d2game/src/ds1.rs` | `Ds1Layout::{Legacy, Real(RealLayers)}` ; `RealCell`/`decode_real_cell` (bit-packing) ; `Ds1::from_ds1_path` ; accesseurs `legacy_layers()`/`real_layers()` ; tests. |
| `engine-rs/crates/d2game/src/ds1_path.rs` | **Inchangé** — son `decode_ds1_paths` (validé or vs 8 échantillons v1.12) est la porte du schéma réel. |
| `engine-rs/crates/d2game/src/dt1.rs` | `Dt1Tile` + champs `dir/kind/style/seq/rfi` ; `decode_dt1` (legacy, inchangé sémantiquement) et **`decode_dt1_paired`** (tuile k = record k, méta+blocs appariés) ; helper `decode_tile_at` ; doc ; tests. |
| `engine-rs/crates/d2game/src/compose.rs` | `TileMatcher` (candidats par métadonnées) ; `compose_seeded` branché Legacy/Real ; `compose_real` (rendu + grille de marche : sols ouvrants + flags 0x2000, murs bloquants, cachés ignorés, ombre jamais dessinée) ; `compose_legacy` = ancien chemin intact ; tests + artefacts avant/après. |
| `engine-rs/crates/d2game/src/game.rs` | `decode_frozen_ds1` → `(Ds1, paired)` (EOF exact = schéma réel) ; `build_frozen_tiles` consomme les tuiles paired + `dt1_meta_aligned_paired` si paired ; `resolve_container` règle dossier « objects » (commentaires `// FIX-CARTES`). |
| `engine-rs/crates/d2game/src/automap.rs` | Résolveur par layout (`visible_tile_index_legacy`/`_real`) ; `dt1_meta_aligned_paired` ; tests mis à jour. |
| `engine-rs/crates/d2game/src/drlg.rs`, `drlg_a45.rs`, `drlg_maze.rs` | Construction des Ds1 synthétiques passée à `Ds1Layout::Legacy` (aucun changement de comportement) ; tests adaptés. Les presets a45 (townsiege/fortress/nihl) **restent** au chemin legacy : leurs banques ne portent pas toutes les tuiles référencées (fortress/nihl : ~20-70 % non résolus, prouvé) — conversion = travail futur. |
| `docs/fix-cartes-avant.png`, `docs/fix-cartes-apres.png` | Artefacts (rendu pleine carte moteur, palette PL2 + gamma 0,7, échelle 1/4). |

## 3. Tests

- `cargo test --workspace` : **832 passés / 0 failed** (dont le playthrough
  e2e complet actes 1-5).
- **Nouveau test mission** `compose::tests::town1_real_data_renders_the_fortified_camp`
  (données réelles TownN1.ds1 + 8 DT1) : 0 cellule non résolue ; 2337 sols +
  346 murs dessinés ; ≥ 19 tentes, ≥ 16 feux/props, ≥ 180 palissades ;
  arbres bornés 100-130 (fini la forêt généralisée) ; ratio marchable
  0.55-0.90 ; entrée warp (247, 107) marchable à proximité.
- `render_before_after_pngs` : produit les artefacts du rapport.
- Automap : le town passe en mode **« fallback »** documenté (couverture
  honnête ~26 % : l'herbe style 5, les arbres et les tentes n'ont pas de
  records « 1 Town » — le 73 % historique était mesuré sur les tuiles
  fausses) ; bloodmoor reste « automap » (non-régression testée).

## 4. Avant / après

- `docs/fix-cartes-avant.png` : ancien décodage — le camp couvert par la
  fausse forêt (flux d'orientation rendu comme tuiles d'arbres).
- `docs/fix-cartes-apres.png` : schéma réel — **camp fortifié** :
  palissade de rondins, bosquet NO, tentes, feu/props, chemins de terre,
  forêt en bordure, water/moat à l'Est près de la sortie.
- Le pkg démo a été reconstruit (`demo/game-pkg`, wasm-bindgen) — la page
  `demo/game.html` consomme le nouveau moteur sans changement JS.

## 5. Points d'intégration (pour le merge dans l'arbre maître)

1. Reprendre les fichiers modifiés ci-dessus (§2) — `demo/` et `game.js`
   ne changent PAS, seul `demo/game-pkg/*` est recompilé.
2. `manifest.json` inchangé (les 8 DT1 du town y étaient déjà tous listés ;
   c'est le résolveur qui se trompait).
3. Les sauvegardes existantes (grille 285×205) restent compatibles : mêmes
   dimensions de grille ; quelques cellules passent de bloqué→libre
   (anciens « arbres » fictifs) — les NPC/spawns du camp sont re-snappés
   sur tuile marchable par le moteur.
4. **Travail futur suggéré** (hors périmètre) : convertir les presets a45
   (townsiege/fortress/nihl) au schéma réel en élargissant leurs banques ;
   exploiter la section OBJETS du DS1 (`decode_ds1_paths` la lit déjà) pour
   placer les objets animés du camp (feu de camp animé, wagon de Warriv).
5. Automap : si l'on veut rendre l'herbe du camp en mini-carte, ajouter un
   record par défaut (cel générique) quand aucun record AutoMap ne couvre
   une tuile de sol — décision produit, non implémenté.
