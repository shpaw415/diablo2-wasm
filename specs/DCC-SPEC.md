# DCC — Sprites animés Diablo 2 : spéc, rendu et pièges corrigés

Décodage et composition des animations DCC (character/monster sprites) avec la
crate Rust `engine-rs/crates/d2dcc` (port byte-faithful d'OpenDiablo2 `d2dcc`/
`d2cof`). Ce document résume le format, le pipeline de rendu, et — surtout —
les pièges découverts et corrigés lors du rendu du barbare WALK 1HS, avec les
octets de preuve.

**Livrables** (barbare `BAWL1HS`, direction jeu 0 → slot DCC 4, ×3, fond
`#1A1A22`) :

| Fichier | Contenu | sha256 (prefix) |
|---|---|---|
| `assets/dcc/composite.png` | frame 0, 6 couches composées (138×147) | `69cdc006…` |
| `assets/dcc/walk-cycle.png` | frames 0–7 en grille 4×2 (555×309) | `bc219720…` |

CLI : `engine-rs/target/debug/dcc_render`

```text
dcc_render <file.dcc> <out.png> [--pal FILE] [--dir N] [--frame N] [--scale S] [--bg RRGGBB]
dcc_render compose <file.COF> <out.png> [--pal FILE] [--dir N] [--scale S] [--bg RRGGBB]
      (--layer TOK=FILE.dcc ... | --dcc-dir DIR [--stem STEM]) [--frames K]
```

## 1. Rappel du format

### DCC (`dcc.rs`, `direction.rs`, `frame.rs`)

```
[0]  signature 0x74
[1]  version (6 ici)
[2]  nombre de directions (1, 4, 8, 16, 32, 64)
[3]  i32 frames per direction
[7]  i32 tag=1
[11] i32 total_size_coded
[15] i32 × n_dirs : offsets de direction (× 8 → position en BITS dans le fichier)
```

Preuve — `BAWLTR.dcc[0..12] = 74 06 10 08 00 00 00 01 00 00 00 c0`
(sig 0x74, v6, 16 directions, 8 frames/dir, tag 1).

Chaque direction est un flux de bits indépendant (entête à largeurs variables
encodées via `CRAZY_BIT_TABLE`), contenant :

1. les en-têtes de frames : `width`, `height`, `x_offset` (signé), `y_offset`
   (signé), `optional_bytes`, `coded_bytes`, `bottom_up`. La boîte d'une frame
   vaut `left = x_offset`, `top = y_offset - height + 1` ;
2. les tailles des 4 sous-flux (equalCells, pixelMask, encodingType,
   rawPixelCodes) sur 20 bits chacun, présents selon `compression_flags` ;
3. une bitmap de 256 bits listant les entrées de palette utilisées (compactée
   en `palette_entries[256]`) ;
4. les sous-flux, forkés **à la position bit courante** (le fork copie la
   position intra-octet), puis le flux principal saute chacun ;
5. le flux pixel-code/déplacement (displacements 4 bits, `15` = continuer).

Les pixels sont décodés cellule par cellule (grille 4×4 px) dans un
*pixel buffer* partagé par direction (une cellule déjà peinte par une frame
précédente peut être déclarée « égale » via equalCells → copiée). Chaque frame
récupère ses cellules peintes dans son propre buffer `pixel_data`.

Erreurs proprement rejetées : `frame_is_bottom_up == 1`
(`DccError::BottomUpFrame`), `optional_data_bits > 0`
(`OptionalBytesUnsupported`), et taille de sous-flux non consommée exactement
(`BitstreamSizeMismatch`) — chaque frame de chaque direction des 8 fichiers
`assets/dcc/*.dcc` passe ces vérifications.

### COF (`cof.rs`)

```
[0]  n_layers   [1] frames_per_dir   [2] n_dirs   [3..24] unknown(21)   [24] speed
[25..28] unknown(3)
puis n_layers × 9 octets : type, shadow, selectable, transparent, effect, weapon_class[4]
puis frames_per_dir octets ("animation frames")
puis n_dirs × frames × n_layers octets : priorité de dessin PAR FRAME
```

Preuve — `BAWL1HS.COF[0..34] = 09 08 10 14 00 f0 fd 7f c1 ff ff ff 24 00 00 00
96 ff ff ff 0a 00 00 00 a8 00 00 00 01 01 01 00 00 68` → 9 couches, 8 frames,
16 directions, speed 168. Les octets de priorité de la frame 0 (après
`25+3+9×9+8 = 117`) sont `05 03 02 01 04 08 00 09 07` = **RH, RA, LA, TR, LH,
S2, HD, S3, S1** — premier = fond. L'ordre change frame par frame : toujours
lire `cof.draw_order(dir, frame)`, pas un ordre global.

`BAWL1HS.COF` déclare 9 couches ; le MPQ retail n'expose que 6 fichiers DCC
pour cette anim (HD, TR, LA, RA, S1, S2) — LH, S3, RH sont absents du retail
(le compositeur les saute avec un `note:`). C'est normal : plusieurs tokens
COF n'ont pas de sprite pour tous les modes/armes.

### Chemins des fichiers : motif LIT

`data\global\chars\<classe>\<couche>\<classe><couche>LIT<mode><arme>.dcc`
(côté monstre : `data\global\monsters\<code>\…`). Le segment `LIT` est une
constante du chemin, PAS un token de couche — `BAWL1HSHD.dcc` n'existe pas,
c'est `BA\HD\BAHDLITWL1HS.dcc`-style. Les 6 DCC du barbare ont été sondés par
hash MPQ selon ce motif (voir `cof.rs::dcc_stems`).

## 2. Piège n°1 — la palette est BGR (preuves par octets)

`data\global\palette\act1\pal.dat` stocke chaque entrée en **[bleu, vert,
rouge]**, pas [R,G,B]. Lire les triplets en RGB rend un démon rouge en bleu.

Preuve — entrée 70, très utilisée par la peau du Fallen (histogramme frame 0
de `FAWLTR.dcc` : indices 33–88 = rampe du corps) :

```
pal.dat[210..213] = 18 28 80
  lu RGB : (24, 40, 128)  → bleu  ✗
  lu BGR : (128, 40, 24)  → brun-rouge ✓ (le Fallen est rouge)
```

Rampe complète du corps (indices 33→85, stockés) : le 3e octet domine
(8→176) en croissant, les deux autres restent bas — en RGB ce serait un
démon bleu, en BGR un démon rouge. Vérification visuelle : le même frame
rendu en BGR donne un Fallen rouge/cornu, en RGB une créature bleutée
(`dcc_render` + échange R↔B).

Rendu final correct (BGR, fond clair pour lisibilité) : le Fallen est rouge,
lance et cornes nettes.

**Conséquence** : `render::Palette::load(raw, /*bgr=*/true)` ; index 0 =
transparent (alpha 0, jamais peint par `composite_indices`).

### 2b. Piège annexé — le fichier palette « 767 octets » était CORROMPU

`assets/map/pal_act1.dat` faisait 767 octets et la consigne était « padder à
768 avec 0 ». En réalité ce fichier était une **extraction corrompue** (voir
piège n°3 pour la cause) : 646 des 767 octets différaient du vrai contenu de
l'archive. Le padder n'aurait pas suffi (entrée 255 = `(255,255,0)` jaune au
lieu de blanc). 

Correction : le lecteur MPQ réparé ré-extrait le vrai `pal.dat`
(768 octets, dernier octet `FF`) :

```
assets/pal_act1.dat  == assets/map/pal_act1.dat
sha256 = 92faf0d06039faaf156480893d55f064355b900faadd1e0f7fddad569849c17c
premiers octets 00 00 00 00 00 24 08 18 1c · derniers 58 64 68 ff ff ff
```

Le CLI padde toujours 767→768 par robustesse (`Palette::load`), testé.

## 3. Piège n°2 — `frame.pixel_data` couvre la boîte de DIRECTION, pas celle de la frame

`DccDirection::generate_frames` alloue chaque `pixel_data` à
`direction.box_width × direction.box_height` (les cellules sont re-tuilées
sur la grille de la direction, `recalculate_cells_boxed` donne des offsets
relatifs à la boîte de direction). Un compositeur qui indexe `pixel_data`
avec la largeur de la **frame** (`fx + fy * frame.box_width`) lit du bruit.

Règle appliquée (et testée, `render.rs::composite_indices`) :

```
p = frame.pixel_data[x + y * direction.box_width]     // x,y ∈ boîte de direction
canvas[(x + direction.box_left - rect.left) + ...]    // offset via la boîte de direction
```

avec clipping sur le rectangle union des boîtes de frames (`frames_union`).
Les coordonnées de frame (`x_offset`/`y_offset`) partagent le même espace
d'origine entre toutes les couches d'une animation — c'est ce qui rend la
composition COF possible sans calibration supplémentaire (vérifié : bras/
torse/tête s'alignent au pixel près sur le barbare composé).

## 4. Piège n°3 — MPQ : dernier secteur stocké BRUT sans masque

`d2mpq::Archive::read` (et le référentiel Python `tools/mpq_mini.py`, même
bug) traitaient un chunk dont le premier octet vaut 0 comme « masque 0x00 =
données brutes » et émettaient le reste tel quel → `pal.dat` rendu **767
octets au lieu de 768** (`cargo test` échouait sur
`verify_game_files` : `zlib small: length mismatch (got 767, want 768)`).

Preuve par octets (bloc `data\global\palette\act1\pal.dat`,
`flags = 0x80000200` = EXISTS|COMPRESS, non chiffré) :

```
offset=30092207  csize=776  fsize=768
table de secteurs en clair (2 × u32) : 08 00 00 00 · 08 03 00 00  → [8, 776]
secteur unique = bloc[8..776] = 768 octets, commençant par 00 00 00 00 00 24 08 18
```

Règle Storm : un secteur compressé qui n'économiserait pas son octet de
masque est stocké **brut, sans octet de masque**. Détection correcte :
`chunk.len() >= taille_non_compressée_du_secteur` (pour le dernier secteur,
c'est `fsize - n*sector_size`, PAS la taille de secteur 4096). L'ancien test
`is_last && chunk.len() >= sector_size` manquait précisément les fichiers
plus petits qu'un secteur.

Correctif : `engine-rs/crates/d2mpq/src/archive.rs` (`plain_len` par
secteur). Après correctif : les 7 fichiers de jeu de `verify_game_files`
sont byte-identiques (`cargo test` vert).

## 5. Rendu du barbare — recette exacte

```sh
cd /home/agent/workspace/diablo-2-wasm
cargo build -p d2dcc   # dans engine-rs/
# composite (1 frame, 6 couches, ordre COF par frame) :
target/debug/dcc_render compose assets/dcc/BAWL1HS.COF assets/dcc/composite.png \
  --dir 0 --frames 1 --dcc-dir assets/dcc --stem BAWL \
  --scale 3 --bg 1A1A22 --pal assets/pal_act1.dat
# walk-cycle (8 frames, grille 4×2) :
target/debug/dcc_render compose assets/dcc/BAWL1HS.COF assets/dcc/walk-cycle.png \
  --dir 0 --frames 8 --dcc-dir assets/dcc --stem BAWL \
  --scale 3 --bg 1A1A22 --pal assets/pal_act1.dat
```

* `--dir 0` (direction jeu) → slot DCC 4 via `dir64_to_dcc(0, 16)`.
* Les 6 couches sont empilées selon `cof.draw_order(slot, f)` ; chaque frame
  du cycle reste le même personnage (pas de scintillement : les equal-cells
  propagent les cellules inchangées entre frames).
* Debug mono-couche : `dcc_render assets/dcc/FAWLS1.dcc out.png …`
  (Fallen, 1 couche) avant toute composition multi-couches.

## 6. Tests

`cargo test` (workspace `engine-rs/`) : **31 verts d2rules + suites d2dcc/d2mpq/d2dc6/d2tool vertes, 0 échec** —
dont 4 dans `d2dcc::render` : ordre BGR + padding 767→768,
transparence index 0 + priorité des couches, union de boîtes, offsets
boîte-de-direction. Le test MPQ `verify_game_files` (7 fichiers réels,
SHA-256) valide le correctif du secteur brut.

## 7. Sorcière (SO) et Paladin (PA) — extraction et pièges nouveaux

Livrables (vague « deux nouveaux héros », même pipeline que le barbare) :
47 DCC + 8 COF dans `assets/dcc/`, composites `sor-composite.png`
(6 couches, 186×198, ×3) et `pal-composite.png` (7 couches, 120×111, ×3),
`assets/manifest.json` : `heroes` {BA, SOR, PA} + `hero` (compat BA).
Direction jeu 0 → slot DCC 4, fond `#1A1A22`, palette `assets/pal_act1.dat`.

### 7.1 Codes et chemins réels (tous VERIFIÉS par hash MPQ, d2char.mpq)

* Codes classes : **SO** (Sorcière), **PA** (Paladin) — pas « SOR ».
* COF : `data\global\chars\<CLS>\cof\<CLS><MODE><WC>.cof` (sous-dossier
  `cof`, hash insensible à la casse).
* DCC : `data\global\chars\<CLS>\<LAYER>\<CLS><LAYER><COMP><MODE><WC>.dcc`
  où `<COMP>` est le token de composant : **LIT**/MED/HVY (armures,
  TR/LG/LA/RA/S1/S2), casques (HD : LIT=tête nue, CAP, CRN, FHL, GHM, HLM,
  MSK, SKP, BHM), graphies d'arme (RH : SST=short staff, SSD=short sword,
  BST, LST, SCY…). Le segment `LIT` du barbare est donc le composant
  « armure légère », pas une constante.

### 7.2 Piège n°4 — le weapon class est PAR COUCHE dans le COF

Le COF porte un champ `weapon_class` **par couche** (octets
`25+3+9i+5..9`). La sorcière NU/WL STF mélange : bras/mains (LA sauf NU,
RH, LH) en `STF` mais corps (TR, LG, HD, S2, S3) en **HTH**. Un Battery
qui suppose « WC du COF pour toutes les couches » rate TR/LG/HD
(`SOTRLITWLSTF.dcc` n'existe pas ; c'est `SOTRLITWLHTH.dcc`). Lire le WC
de chaque couche dans le COF, puis sonder `<CLS><LAYER><COMP><MODE><WC de
la couche>.dcc`. C'est aussi pour ça que la première sonde « SO WL STF »
ne trouvait que RA/LA.

### 7.3 Piège n°5 — LG (jambes) n'est PAS une couche COF

L'enum COF (0=HD … 14=S8) n'a pas de LG ; aucun COF héros ne déclare de
couche jambes, pourtant `PALGLITWL1HS.dcc` (63 Ko) existe. Les DCC LG
sont extraits et listés dans le manifest (comme pour BA) mais le
compositeur COF ne les empile pas — d'où l'allure « buste » des composites
barbare/sorcière/paladin (identique au `composite.png` validé).

### 7.4 Piège n°6 — `tools/blast_pkware.py` échoue sur des flux DCL valides

`ValueError: bad lit flag` (et erreurs zlib) sur des DCC caractère réels
(ex. `SOHDLITNUHTH.dcc`, `PARHSSDNU1HS.dcc`). Le lecteur Rust réparé
(`d2mpq`, règle du dernier secteur brut) les décompresse — nouveau CLI
**`engine-rs/target/debug/mpq_extract <archive.mpq> <nom\interne> <out>`**
(crates/d2mpq/src/bin/mpq_extract.rs) utilisé pour les 47 fichiers.
Extraction Python : ne plus s'appuyer sur `mpq_mini.read_file` pour les
DCC caractère.

### 7.5 Couverture et couches manquantes (LIT = tenue de base)

| Anim | COF (frames×dirs) | Couches extraites | Manquantes retail |
|---|---|---|---|
| SOR NU | SONUSTF.COF (8×16) | HD TR LG LA RA S2 RH | LH, S3 |
| SOR WL | SOWLSTF.COF (8×16) | HD TR LG LA RA S2 RH | LH, S3 |
| SOR A1 | SOA1STF.COF (18×16) | HD TR LG LA RA S2 RH | LH, S3 |
| SOR DD | SODDHTH.COF (1×16) | TR | LH, S3, HD… (corps entier en TR) |
| PA NU | PANU1HS.COF (8×16) | HD TR LG LA RA S1 S2 RH | LH, S3 |
| PA WL | PAWL1HS.COF (10×16) | HD TR LG LA RA S1 S2 RH | LH, S3 |
| PA A1 | PAA11HS.COF (15×16) | HD TR LG LA RA S1 S2 RH | LH, S3 |
| PA DD | PADDHTH.COF (1×16) | TR | idem SOR |

* DD (mort) = wc **HTH** pour les deux classes, corps entier en une couche
  TR (1 frame).
* RH sorcière = `SORHSST*STF.dcc` (short staff, arme de départ `sst`) ;
  RH paladin = `PARHSSD*1HS.dcc` (short sword, départ `ssd`).
* LH (main gauche/bouclier) et S3 déclarés par les COF mais absents du
  retail pour ces modes — skippés par le compositeur (`note:`), comme
  LH/S3/RH du barbare.
* Noms locaux : `<CLS><MODE><LAYER>.dcc` (ex. `SOWLHD.dcc`) — le stem du
  CLI compose vaut donc `<CLS><MODE>` (SOWL/PAWL/…).

### 7.6 Stats réelles (assets/patch_charstats.txt, colonnes citées)

| | SOR | PA | (BA, mémoire) |
|---|---|---|---|
| life = vit + hpadd | 10+30 = **40** | 25+30 = **55** | 55 |
| mana = energy | 35→**35** | 15→**15** | int=10 (stocké 15, écart noté) |
| stamina | **74** | **89** | 92 |
| str/dex/int/vit | 10/25/35/10 | 25/20/15/25 | 30/20/10/25 |
| walk/run | 6/9 | 6/9 | 6/9 |
| LifePerLevel (÷4) | 4→1 | 8→2 | 8→2 |
| LifePerVitality (÷4) | 8→2 | 12→3 | 16→4 |
| ManaPerLevel (÷4) | 8→2 | 6→1.5 | 4→1 |
| ManaPerMagic (÷4) | 8→2 | 6→1.5 | 4→1 |
| ToHitFactor | -15 | 20 | 20 |
| BlockFactor | 20 | 30 | 25 |
| startWeapon | sst (Short Staff, stf) | ssd (Short Sword, 2-7, 1hs) | hax |

« The following are in fourths » : les colonnes LifePerLevel…ManaPerMagic
sont divisées par 4 à la lecture. `hero` inchangé (compat moteur) ;
`heroes` {BA, SOR, PA} ajouté, BA = copie de `hero`. Test
`hero_selection_default_stays_retrocompatible` corrigé en conséquence :
il construit désormais son manifeste SANS `heroes` explicitement (le
manifeste du dépôt en porte une depuis cette vague).

### 7.7 Recette (reproductible)

```sh
cd /home/agent/workspace/diablo-2-wasm
# composite sorcière (6 couches, ordre COF par frame) :
engine-rs/target/debug/dcc_render compose assets/dcc/SOWLSTF.COF \
  assets/dcc/sor-composite.png --dir 0 --frames 1 --dcc-dir assets/dcc \
  --stem SOWL --scale 3 --bg 1A1A22 --pal assets/pal_act1.dat
# composite paladin :
engine-rs/target/debug/dcc_render compose assets/dcc/PAWL1HS.COF \
  assets/dcc/pal-composite.png --dir 0 --frames 1 --dcc-dir assets/dcc \
  --stem PAWL --scale 3 --bg 1A1A22 --pal assets/pal_act1.dat
# cycles de contrôle (cohérence frame à frame) : --frames 8 (SOR) / 10 (PA)
```
