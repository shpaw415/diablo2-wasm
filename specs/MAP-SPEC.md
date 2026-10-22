# MAP-SPEC — Carte Town acte I : formats constatés & grille de collision

Projet : diablo-2-wasm · Source : release digitale v1.12 (`d2_d2es_2000-01_fr-fr…zip`)
Périmètre : `TownN1.ds1` (Camp des Rousses, levelId 1) + 8 conteneurs DT1 référencés.
Auteur : intégration web — 2026-10-22. Scripts : `tools/map_proto2.py` (rendu),
`tools/map_collision.py` (collision).

---

## 1. Fichiers produits

| Fichier | Contenu |
|---|---|
| `assets/map/collision.json` | `{width, height, rows}` — **`rows[cy][cx] = true ⇒ cellule BLOQUÉE`** (57×41, lignes = cy) |
| `assets/map/collision-heat.png` | heatmap 8 px/cellule : noir = bloqué, blanc = libre |
| `assets/map/collision-overlay.png` | debug : grille teintée (rouge=bloqué, vert=libre) sur `level-render.png` |
| `assets/map/level-render.png` | rendu isométrique validé (phase 1) |

## 2. Conteneur DT1 (release digitale — PAS le format retail classique)

⚠️ **Pas de signature `C7 25 AB 0B`** dans cette release. Entête de 268 o :

```
@0   int32 = 7          preuve floor.dt1 : 07 00 00 00
@4   int32 = 6          06 00 00 00
@268 int32 nbTuiles     table des tuiles @252, stride 96 o (réservé 256 + 40?)
@252+96k   int32 dataOffset, int32 dataLength, int32 nbSousBlocs (+93 o non lus)
```

Blob tuile = `nbSousBlocs × [entête 20 o]` puis les données. **Nature de la tuile :
`nsub == 25` ⇒ SOL, sinon MUR** (heuristique validée sur les 507 tuiles).

Entête sous-bloc (20 o), preuve (tuile 1 de floor.dt1, sous-bloc 0) :

```
40 00 40 00 00 00 04 04 01 00 00 01 00 00 00 00 f4 01 00 00
@0  int16 a        = 64    (x px relatif ancre tuile)
@2  int16 b        = 64    (y px relatif ancre tuile)
@4  4 o indéterminés = 00 00 04 04
@8  uint16 FLAGS   = 0x0001   ← unique source de collision
@10 uint32 longueur = 256
@14 2 o padding
@16 uint32 offset   = 500 (relatif au début du blob)
```

- SOL : 25 sous-blocs **32×8 bruts** (256 o, index 0 = transparent), positions en
  diagonales `a+2b ∈ {64,96,128,160,192}` (losange ~160×72 px — art **2×** du
  80×40 classique). Placement rendu : `sx+a, sy+b`.
- MUR : sous-blocs **RLE-lignes** `(t, o, o octets)` avec `t+o ≤ 32`,
  `(0,0)` = ligne vide, `0x00` finaux = padding. `b` négatif = au-dessus de l'ancre.

## 3. DS1 layout version 18

```
@0  int32 version = 18     preuve : 12 00 00 00
@4  int32 w = 56           réel = w+1 = 57   ⚠️ piège v18
@8  int32 h = 40           réel = h+1 = 41
@12 2× int32 = 0
@20 int32 fileCount = 8    puis 8 chemins ASCIIZ (\d2\data\global\tiles\…tg1)
puis meta (2× int32), puis 6 couches de 57×41 int32 (row-major, cy*w+cx)
```

Cellule int32 → `p1 = v&0xFF` (index de tuile), `p2 = (v>>8)&0xFF` (variante :
index réel = `p1 + (p2 & 0x0F)`), `p3`, `orient = v>>24` (bit 0x80 = cachée).
Couches utilisées ici : **L4 = sol principal, L0 = sol/rues, L1 = arbres (murs),
L2/L5 = variants de sol, L3 = arbres (second passage)**.

### Ordre des DT1 (critique)
Les index p1 sont **cumulatifs dans l'ordre des chemins du DS1** :
`treegroups(0-20), stonewall(21-72), floor(73-216), warp(217-297),
river(298-342), town_objects(343-433), fence(434-464), out_objects(465-506)`.
Mapping `.tg1` → fichier : dernier segment du chemin ; `objects` ⇒
`town_objects.dt1` si le dossier parent est `town`, sinon `out_objects.dt1`.

⚠️ `floor.dt1` == `town_floor.dt1` (idem fence/stonewall/river/treegroups :
md5 identiques, simples doublons de nommage). **Cette carte ne référence
réellement que floor.dt1 (L4/L0/L2/L5) et treegroups.dt1 (L1/L3)** — aucune
cellule river/warp/fence/stonewall référencée (vérifié par scan exhaustif).

## 4. Flags de sous-tuiles — sémantique constatée

Distribution mesurée (10 136 sous-blocs) :

| Flags | Sol (6175) | Murs (3961) | Interprétation |
|---|---|---|---|
| `0x0001` | 6053 (98 %) | 121 | **marchable** (herbe, routes, petits props) |
| `0x1001` | 50 | 3752 (95 %) | **bloquant** (murs, haies, troncs) |
| `0x2005` | 72 | 88 | **bloquant** (rochers/buissons denses, bords) |
| `0x2001` | 0 | — | bloquant (variante) |

**Choix fondé sur les données** : `BLOCK_MASK = 0x1000 | 0x2000 | 0x0004`.
La lecture retail classique « bit 0 = block walk » est **impossible ici** :
98 % des sous-blocs de sol (routes et cours comprises) porteraient 0x0001 et
toute la carte serait murée. Les bits 12/13 non documentés chez Phrozen Keep
portent le blocage dans cette release ; 141/143 tuiles de sol de floor.dt1 sont
uniformes à 0x0001, les 2 mixtes portent 0x2005. Les tuiles « sol » de
treegroups.dt1 sont uniformes à 0x1001 (forêt dense bloquante ✓).

## 5. Algorithme de collision (`tools/map_collision.py`)

Par cellule (cx, cy), parcours des couches dans l'ordre `4, 0, 2, 5, 1` :

1. cellule vide (`p1==0`) ou cachée (`orient & 0x80`) → ignorée ;
2. tuile **mur** (nsub≠25) référencée → **bloquée** ;
3. tuile **sol** → `floor_ok = true` ; on cumule les sous-blocs et ceux qui
   satisfont `flags & BLOCK_MASK` ;
4. état final : `bloquée = mur présent ∨ pas de sol ∨ (sous-blocs bloquants ≥ 50 %)`.

Défaut = bloqué (bord de carte, hors ville). Résultat : **381 bloquées /
1956 libres** sur 2337 cellules. Overlay visuel : clôtures/haies, bâtiments et
bordures forestières rouges ; prairies, routes et cours vertes — conforme au
rendu validé.

## 6. Contrat pour d2game / d2rules

- `collision.json` : `width=57, height=41`, `rows[cy][cx]` — **`true` = bloqué**.
- Résolution : 1 cellule = 1 losange de tuile (art 2× ≈ 160×72 px ; pas iso 80×40).
- A* : voisins 4- ou 8-connexité au choix (8 recommandé, les diagonales entre
  deux bloquées sont rares mais existent le long des haies — filtre optionnel).
- Positionnement écran (repère de `level-render.png`, échelle 1/2) :
  `centre_px_x = (cx - cy) * 40 + 1680`, `centre_px_y = (cx + cy) * 20 + 228`.
  Inverse : `cx = ((x-1680)/40 + (y-228)/20) / 2`, `cy = ((y-228)/20 - (x-1680)/40) / 2`.
- Sauvegarde : régénérer avec `python3 tools/map_collision.py` (déterministe).

## 7. Palette — ⚠️ `pal.dat` de cette release a R et B inversés

Découverte clé (pipeline de rendu) :

- `data\global\palette\act1\pal.dat` (768 o, 99,6 % ≡0 mod 4 = 6 bits VGA ×4,
  pad `ff ff ff` en queue) rend le monde **bleu ardoise** : à l'index 54 elle
  vaut `(8,16,88)` alors que la base du PL2 vaut `(89,16,8)` — **c'est la même
  palette avec les canaux R et B permutés** (vérifié sur toute la table :
  `pal.dat[i] = swapRB(PL2base[i])` à ±1 près).
- La vraie palette d'affichage de l'acte I est **`pal.pl2` @0 : 256 entrées
  RGBA 6 bits (alpha=0)** — tons terre/verts act1 corrects. Le PL2 (443 175 o)
  contient en outre 3 palettes RGBA @370412 (shifts sombres type nuit) et une
  zone centrale de tables (non décodées, pas nécessaires au MVP).
- Preuves : `tools/map_render2.py` ; tuile d'herbe `floor.dt1` #129 = terre
  brune + bande d'herbe verte avec le PL2, bruit ardoise avec pal.dat ;
  skillicons DC6 rendus « plausibles » avec pal.dat car sans rouge saturé
  (le swap y est invisible) — ne pas s'y fier.

**Décision** : `assets/map/pal_act1_pl2.dat` = palette corrigée (768 o RGB 8 bits,
`v×255/252`) — LA palette à utiliser partout (tuiles ET sprites). Le PNG
`level-render-v2.png` ajoute un lift gamma 0,7 (équivalent rampe vidéo D2) ;
le .dat reste brut/authentique. `manifest.json` → `level.palette` pointe
désormais sur `pal_act1_pl2.dat`.

## 8. Limites connues

- Résolution cellule (pas sous-tile) : suffisante pour l'A* MVP ; le sous-tile
  5×5 par cellule (diagonales `a+2b`) reste disponible dans les DT1 si besoin.
- Le seuil ≥50 % lisse les tuiles mixtes (2 tuiles concernées, non référencées).
- Flags 0x0004 seul jamais observé seul ; si d'autres cartes l'utilisent,
  re-vérifier le masque avant réutilisation telle quelle.
