# RAPPORT-FIX — d2w-fix-heros (jambes du héros + échelle de jeu)

Arbre : `/home/agent/workspace/d2w-fix-heros/` (copie conforme du maître `diablo-2-wasm/`,
`extracted` en symlink lecture-seule, aucun fichier du maître modifié).
Date : 2026-10-24. Suite `cargo test --workspace` : **0 failed** (≈810 tests, 1 ignoré).

---

## 1. Jambes manquantes — cause racine et correctif

### Cause racine (prouvée)
**La couche LG (jambes) n'existe pas dans l'enum COF** (0=HD … 14=S8 —
`evidence/DCC-SPEC.md §7.3`, confirmé par dump des COF réels `BAWL1HS/BANU1HS/
BARN1HS/BAA11HS/BADDHTH` : 9 couches déclarées, aucune LG). Le compositeur
`SpriteSet::compose` (engine-rs/crates/d2game/src/sprite.rs) n'empilait que les
couches retournées par `cof.draw_order()` → les DCC LG listés dans le manifest
n'étaient **jamais dessinés** : « allure buste », héros sans jambes dans tous
les modes, en jeu comme partout. Le piège était documenté à l'extraction
(DCC-SPEC §7.3 « le compositeur COF ne les empile pas ») mais jamais corrigé
dans le moteur.

Deuxième volet : plusieurs fichiers LG manquaient localement (sondes antérieures
faites sur le mauvais weapon class). Sonde retail refaite **par hash** sur
d2char/d2data/d2exp/patch_d2 : les jambes existent en boutique pour **toutes les
classes et tous les modes** NU/WL/RN/A1, au **wc du corps** (`hth` pour
BA/SO/AI/DZ/NE, `1hs` pour PA, `1ht` pour AM — « sonder TOUJOURS le wc hth »,
DCC-SPEC §7.5).

### Correctif
1. **`engine-rs/crates/d2game/src/sprite.rs`** — `compose()` empile désormais
   LG **en bas de pile** (dessinée en premier, sous torse/bras/tête), aux
   coords de sa propre box DCC ; l'union des boxes l'intègre sans rognure.
   Même géométrie que le rendu de référence validé `tools/heroes_sheets.py`.
   Marqueur `// FIX-HEROS` dans le code. (Boxes vérifiées : BANULG dir0 =
   (-12,-44) 20x55 — les pieds descendent à y=+11, le bas de l'union reste
   l'ancrage « pieds » du blit bottom-center existant.)
2. **6 DCC LG extraits des MPQ réels** (probe hash + `mpq_extract`, repli
   StormLib `tools/stormlib_read.py` pour d2exp « bad sector offset table »,
   piège §7.4) — posés dans `assets/dcc/` **et** `demo/assets/dcc/`
   (mêmes inodes, hardlinks préservés) :
   - `BARNLG.dcc` ← d2char `balglitrnhth.dcc` (55 031 o)
   - `SORNLG.dcc` ← d2char `solglitrnhth.dcc` (38 932 o)
   - `NEWLLG.dcc` ← d2char `nelglitwlhth.dcc` (37 247 o)
   - `AIWLLG.dcc` ← d2exp `ailglitwlhth.dcc` (38 154 o)
   - `AIA1LG.dcc` ← d2exp `ailglita1hth.dcc` (52 809 o)
   - `DZWLLG.dcc` ← d2exp `dzlglitwlhth.dcc` (45 096 o)
3. **Manifest (les DEUX copies, section heroes strictement identique)** :
   `assets/manifest.json` + `demo/assets/manifest.json` — 13 ajouts LG :
   BA modes.NU/RN ; SOR modes.RN ; AS modes.NU/WL/A1 + walk ;
   DZ modes.NU/WL + walk ; NE modes.NU/WL + walk. (PA/AM et les autres
   modes avaient déjà leurs LG en manifest.) DD inchangés (corps entier en
   TR seule, conforme retail). Les modes DD n'ont pas de couche LG fournie
   sauf AS (déjà présent).

### Preuves (docs/)
- `docs/fix-heros-avant.png` — planche proof_sprite_plate AVANT : buste sans jambes.
- `docs/fix-heros-apres.png` — planche APRÈS (test `tests/fix_heros.rs`) :
  NU/WL/RN/A1 × 4 facings, corps entier.
- Mesures (compose dir0 frame2, test `hero_has_legs`, vert) :
  - **NU : 52x47 (buste AVANT) → 52x77 (APRÈS, +64 % de hauteur)** ;
  - WL 50x76 · RN 42x76 · A1 54x87 — ~77-90 px de haut, soit la taille
    retail du barbare sur des diamants de sol de 80 px (réf. validée
    `docs/parity/heroes-sheet-BA.png` = 53x77 à l'échelle 3) ;
  - assertions : avec LG > hauteur buste ; quart bas peint (jambes) ;
    RN (jambes de course) également allongé.

## 2. Échelle de jeu — conclusion : déjà 1:1 retail, aucune valeur à changer

Le symptôme « gameplay trop petit / caméra trop loin » venait du **buste sans
jambes** (héros 47 px au lieu de ~77 px, soit -39 %) et du héros **invisible au
spawn du camp** (forêt dense, voir note d2w-fix-cartes ci-dessous) — pas d'un
facteur de zoom.

Vérification exhaustive de la chaîne (aucun `scale`/`zoom`/`drawImage` de
scaling n'existe dans wasm.rs / game.rs / compose.rs / game.js — la boucle est
un `putImageData` 1:1 d'un framebuffer viewport 800x600, cf. game.js:895-896) :

| Élément | Mesure (données réelles) | Verdict |
|---|---|---|
| Tuile sol DT1 | 25 sous-blocs 32x8 → diamant **160x80 px** (mesuré : `town_floor.dt1` tile 1-6, x [0,160), lignes peintes 32→160→32) | pose native 1:1 |
| Pitch isométrique | `(cx-cy)*80, (cx+cy)*40` (compose.rs CW=80, CH=40) = tiling retail exact des diamants 160x80 (chevauchement de bounding box, diamants jointifs) | conforme |
| Sous-tuiles | `(wx-wy)*16, (wx+wy)*8` px (compose.rs sub_to_screen) = 32x16 px iso retail ÷ 2 axes — identique au repère D2 | conforme |
| Sprites héros/monstres | DCC décodés et blittés pixel natif (blit_sprite, aucun scaling) | 1:1 |
| Viewport | canvas 800x600 (game.html), `setViewport(800,600)` | résolution de référence D2 |

Le « 256x128 » cité dans le brief correspond à la largeur des tuiles murales
DT1 (256 px, murs/arbres) ; les diamants de sol retail sont 160x80 — les deux
sont déjà rendus à leur taille native. **Échelle avant/après : inchangée
(1:1) ; taille perçue du héros : 47 px → 77 px (+64 %) grâce au fix jambes.**

### Preuves gameplay (docs/)
- `docs/fix-zoom-avant.png` — viewport AVANT (buste, héros masqué au spawn forêt).
- `docs/fix-zoom-apres.png` — Blood Moor APRÈS : héros entier visible au centre,
  proportions retail, 60 fps.
- `docs/fix-hud-town-apres.png` — camp APRÈS : HUD intact (voir §3).

## 3. HUD — aucun conflit (pour d2w-fix-ui)

**`demo/game.js` n'a PAS été touché** (0 ligne `// FIX-ZOOM :`). Les globes
sont dessinés par game.js à coords canvas fixes (`HP_GLOBE {66, 538}`,
`MP_GLOBE {canvas.width-66, 538}`) — la taille du canvas (800x600) et le rendu
moteur 1:1 sont inchangés, donc **rien ne bouge** : vérifié visuellement sur
`docs/fix-hud-town-apres.png` (globes et textes en place). Aucun risque de
conflit sur game.js.

## 4. Fichiers modifiés (liste exhaustive, arbre d2w-fix-heros)

| Fichier | Changement |
|---|---|
| `engine-rs/crates/d2game/src/sprite.rs` | fix LG en bas de pile dans `compose()` (bloc `// FIX-HEROS`, +11 lignes) |
| `assets/manifest.json` | +13 chemins LG (heroes, cf. §1.3) |
| `demo/assets/manifest.json` | idem (copie identique de la section heroes) |
| `assets/dcc/{BARNLG,SORNLG,NEWLLG,AIWLLG,AIA1LG,DZWLLG}.dcc` | extraits du retail (nouveaux) |
| `demo/assets/dcc/*.dcc` | les mêmes (hardlinks) |
| `engine-rs/crates/d2game/tests/fix_heros.rs` | nouveau test preuve (planche + assertions) |
| `docs/fix-heros-avant.png`, `docs/fix-heros-apres.png`, `docs/fix-zoom-avant.png`, `docs/fix-zoom-apres.png`, `docs/fix-hud-town-apres.png` | preuves |
| `demo/game-pkg/d2game.{js,wasm}` + `.d.ts` | pkg reconstruit (wasm-bindgen) ; `game.js` importe toujours `./game-pkg/d2game.js` (vérifié lignes 710/739) |

## 5. Tests cargo (totaux)

- `cargo test --workspace` : **0 failed** — 492 (d2game lib) + 173 (d2rules) +
  38 + 18 + 16 + 12 + 11 + … ≈ **810 passed**, 1 ignored, **0 failed**.
- Nouveau test `hero_has_legs` (tests/fix_heros.rs) : vert (assertions
  hauteur + pixels jambes + planche écrite).
- pkg : `cargo build -p d2game --release --target wasm32-unknown-unknown` +
  `wasm-bindgen --target web --out-dir ../demo/game-pkg --out-name d2game` — OK.

## 6. Observations pour les coéquipiers (hors de mon sujet, rien touché)

- **d2w-fix-cartes** : au spawn du Camp des Rousses (héros ~142,102), le héros
  est totalement masqué par la canopée (tuiles arbres hautes dessinées en
  overlay à chaque cellule, forêt très dense). En Lande sanglante le héros est
  parfaitement visible. La conversion « camp en village » (éclaircies/chemins)
  règlera la visibilité au spawn.
- **d2w-fix-ui** : aucun changement côté HUD/game.js de mon côté (cf. §3).
- Note extraction : les wc à sonder pour les couches corps (LG inclus) sont
  `hth` (BA/SO/AI/DZ/NE), `1hs` (PA), `1ht` (AM) — DCC-SPEC §7.5 confirmé sur
  les 7 classes × NU/WL/RN/A1.
