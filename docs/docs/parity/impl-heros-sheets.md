# HERO-SHEETS — sprite sheets NU des 7 classes + feu de camp (2026-10-23)

Objectif : boucles d'idle (mode NU) des 7 classes en sprite sheets pour
l'écran de sélection animé du menu, plus le feu de camp du menu original.

## Livrables

| Fichier | Contenu |
|---|---|
| `demo/assets/menu/heroes/AM.png` … `SO.png` | 7 sheets RGBA, 1 rangée, frames uniformes, fond transparent |
| `demo/assets/menu/heroes/heroes.json` | index `{CODE: {file, frames, w, h, fps}}` + entrée `fire` |
| `demo/assets/menu/heroes/_preview.html` | page de test : anime les 7 sheets + le feu d'après heroes.json |
| `docs/parity/heroes-sheet-<CODE>.png` | 7 captures de contrôle (damier = preuve de transparence, séparateurs de frames) |
| `docs/parity/heroes-preview-screenshot.png` | capture écran de la page de test (navigateur, serveur HTTP local) |

Outil : `tools/heroes_sheets.py` (régénère tout : sheets + json + captures).

## Index heroes.json (fps officiels AnimData.d2 v1.12)

| CODE | token (COF) | frames | cellule px (w×h) | speed | fps = 25·speed/256 |
|---|---|---|---|---|---|
| AM | AMNU1HT | 8 | 84×270 | 128 | 12.5 |
| AS | AINUHT1 | 8 | 99×225 | 128 | 12.5 |
| BA | BANU1HS | 8 | 159×231 | 96 | 9.375 |
| DZ | DZNU1HS | 6 | 126×228 | 120 | 11.71875 |
| NE | NENU1HS | 8 | 117×216 | 128 | 12.5 |
| PA | PANU1HS | 8 | 108×222 | 128 | 12.5 |
| SO | SONUSTF | 8 | 171×228 | 128 | 12.5 |
| fire | — | 30 | 180×140 | — | 15 (66 ms/frame, cf. menu.js) |

Frames/speed tirés de `engine-rs/crates/d2animdata/tests/golden/animdata_v112.txt`
(dump golden d'AnimData.d2) et recoupés avec l'octet speed des COF : identiques.

## Direction documentée : dir 0 = sud = face caméra

`--dir 0` de dcc_render est une direction **jeu** (0..63) : `dir64_to_dcc(0, 16)`
= **slot DCC 4**. Contrôle visuel (barbare, 16 slots rendus) : le slot 4 est le
seul buste frontal symétrique (face caméra) ; le slot 11 est le dos. C'est la
direction du rendu de face de l'écran de sélection original. Toute la pipeline
existante (mode_grid, sprite.rs `facing*4`) est cohérente avec ce choix.

## Couche LG (jambes) — découverte et correction

LeCOF n'a **pas** d'enum LG (§7.3) : ni `dcc_render compose` ni `sprite.rs`
ne dessinent les jambes — le jeu actuel rend les héros **sans jambes** (le
manifest `modes.NU.layers` n'a jamais de LG). Pour l'écran de sélection
(corps entier comme le menu original), les sheets fusionnent LG :

1. Sondage MPQ : les 7 LG NU existent, mais au weapon class **hth** (corps),
   pas celui du COF — c'est pourquoi la vague d'extraction précédente
   (probe wc=COF) les avait manquées pour BA/AI/DZ/NE :
   - `data\global\chars\ba\lg\balglitnuhth.dcc` (d2char, 38 506 o)
   - `data\global\chars\ai\lg\ailglitnuhth.dcc` (d2exp, 31 021 o)
   - `data\global\chars\dz\lg\dzlglitnuhth.dcc` (d2exp, 25 651 o)
   - `data\global\chars\ne\lg\nelglitnuhth.dcc` (d2char, 28 007 o)
   - AM (`amlglitnu1ht`), PA (`palglitnu1hs`), SO (`solglitnuhth`) déjà extraits.
2. Extraction des 4 manquants (mpq_extract, repli StormLib pour AI/DZ) →
   `assets/dcc/{BA,AI,DZ,NE}NULG.dcc` (dccdump OK ; DZ = 6 frames/dir).
3. Composition : cellule = union des boxes de **direction** (slot 4) des
   couches COF ∪ LG ; LG collée en premier (bas de pile), cellules COF
   dcc_render par-dessus, à coordonnées de box DCC (scale 3). Un assert
   vérifie que la cellule COF de dcc_render = union des boxes COF ×3.

Cohérence « même pipeline que sprite.rs » : ordre des couches COF = draw
order du COF (première = fond), palette act1 corrigée (BGR), index 0 =
transparent. LG est simplement ajoutée en bas de pile (position retail).

## Feu de camp — sonde MPQ (documentée)

Sonde par hash (`mpq_mini.block_info`) sur d2char/d2data/d2exp/patch_d2,
candidats `data\global\ui\FrontEnd\*` :

- **TROUVÉ** : `fire.dc6` (d2data, 163 229 o) — le feu de camp du menu
  original. Déjà extrait le 2026-10-22 → `demo/assets/menu/fire.dc6` et
  rendu en **`demo/assets/menu/fire-sheet.png`** (30 cellules 180×140,
  base y=132, palette loading PL2 — voir `demo/assets/menu/README.md`).
  L'entrée `fire` de heroes.json référence cette sheet existante (menu.js
  l'anime déjà à 66 ms/frame) — pas de doublon `fire.png` créé.
- Absents (16 candidats sondés) : firesurround, firematch, fireplate,
  bigfire, fire2, campfire, menu, menumember, selectscreen, selectscreenEXP,
  herosel, gameselectscreen. Présents aussi : titlescreen.dc6, diablo2.dc6
  (déjà utilisés), gameselectscreenEXP.dc6 (d2exp, fond écran sélection EXP).

## Qualité / tests

- 7 sheets RGBA transparentes, cellules uniformes (union des boxes sur toute
  la boucle → aucune coupure d'une frame à l'autre), 672–1368 × 216–270 px.
- Validation auto : json parse, dims = frames×w × h, mode RGBA, contenu
  opaque > 1000 px par frame ; `tools/verify_dcc.py` : 0 manquant, 0 erreur,
  missingLayers exacts 112/112 (le manifest du jeu n'est pas modifié).
- Test de rendu : `_preview.html` servi par HTTP local, capture navigateur
  `docs/parity/heroes-preview-screenshot.png` — les 7 héros + le feu animés.

## Intégration (pour menu-member)

```js
const idx = await (await fetch('assets/menu/heroes/heroes.json')).json();
// idx.AS = {file:'AS.png', frames:8, w:99, h:225, fps:12.5}
// frame f : drawImage(sheet, f*w, 0, w, h, ...) toutes les 1000/fps ms
// idx.fire.file est relatif au dossier heroes/ (../fire-sheet.png)
```

Animer avec `setInterval(…, 1000/fps)` ; fond transparent natif. Les
chemins `file` sont relatifs au dossier `heroes/`.

## Limites connues

- Couches COF manquantes au retail (déjà l'état du jeu, verify_dcc OK) :
  AM LH+S3, AI S3, DZ LH+S2+S3, NE LH+S3 (osals/quivers optionnels).
- fps BA (9.375) et DZ (11.71875) sont officiels mais non entiers — garder
  la précision float (ne pas arrondir à 9/12, le rythme D2 s'en ressent).
- Scale 3 (proche du rendu pipeline existant) ; le consommateur peut
  downscaler lissé, l'inverse perderait de la définition.
