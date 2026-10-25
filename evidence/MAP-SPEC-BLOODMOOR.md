# MAP-SPEC-BLOODMOOR — 2e niveau : Blood Moor (acte I, levelId 2)

Projet : diablo-2-wasm · Source : release digitale v1.12 (`d2_d2es_2000-01_fr-fr…zip`)
Auteur : développeur NIVEAUX SUPPLÉMENTAIRES — 2026-10-22.
Scripts : `tools/bloodmoor_build.py` (composition + écriture DS1),
`tools/bloodmoor_render.py` (rendu + collision), `tools/mpq_list.py` (énumération MPQ
via StormLib), `tools/bm_sheet.py` (planche contact des presets).
Base : `evidence/MAP-SPEC.md` (Town acte I) — formats DT1/DS1 v18, palette PL2,
masque de collision : **inchangés et réutilisés tels quels**.

---

## 1. Fichiers produits

| Fichier | Contenu |
|---|---|
| `assets/map/bloodmoor.ds1` | DS1 v18 **synthétique** 72×72 (124 825 o, md5 `1f3bca17`), chargeable par le même parseur que TownN1 |
| `assets/map/bloodmoor-render.png` | rendu isométrique 5760×3130 (palette PL2 + gamma 0.7, échelle 1/2) |
| `assets/map/bloodmoor-collision.json` | `{width:72, height:72, rows}` — **`true` = BLOQUÉ** (1410 bloquées / 3774 libres) |
| `assets/map/bloodmoor-collision-heat.png` | heatmap noir=bloqué / blanc=libre |
| `assets/map/bloodmoor-collision-overlay.png` | debug teinté sur le rendu |
| `assets/map/bloodmoor_cliff1.dt1` | 477 434 o, 13 tuiles (d2data.mpq) |
| `assets/map/bloodmoor_cliff2.dt1` | 459 509 o, 13 tuiles (d2data.mpq) |
| `assets/map/bloodmoor_corner.dt1` | 139 154 o, 3 tuiles (d2exp.mpq) |
| `assets/map/presets_bloodmoor/` | les 36 presets outdoor réels `ACT1\Outdoors\*.ds1` (148 Ko) |
| `tools/bm_sheet.py` | planche contact de vérification visuelle des presets |

Manifestes : `assets/map/MANIFEST.json` restructuré en `{town:[…], bloodmoor:{…}}`
(aucun code ne le lisait — docs seulement) ; `assets/manifest.json` gagne
`levels:{town:{…}, bloodmoor:{…}}` et **`level` pointe toujours sur le Town**
(compat : `d2game` lit les pointeurs JSON `/level/ds1`, `/level/dt1`, `/level/palette`).

## 2. Ce qu'est la Blood Moor dans les données

`levels.txt` / `patch_levels.txt` ligne `Act 1 - Wilderness 1` :

```
Id=2  Name="Act 1 - Wilderness 1"  LevelName="Blood Moor"  Act=0
SizeX=80 SizeY=80  DrlgType=3  LevelType=2  EntryFile="A1L2"
mon1..3 = zombie1 fallen1 quillrat1   MonDen=520
```

- **DrlgType=3** : niveau à génération ALÉATOIRE — il n'existe **aucun DS1 complet**
  de la Blood Moor dans les MPQ (les colonnes File1/File2 sont vides).
- **LevelType=2** → `LvlTypes.txt` Id=2 : 26 DT1 act1 (Town/Floor, Outdoors/*,
  Graveyard…). Nous n'en utilisons que 8 (cf. §4) — suffisants pour une plaine.
- **LvlPrest.txt ne contient AUCUNE ligne LevelId=2** : le DRLG outdoor retail
  n'assemble PAS la Blood Moor depuis LvlPrest ; il remplit une grille depuis
  le tileset (choix codé dans D2Common.dll) et y pose des features.

## 3. Les presets outdoor réels (découverte clé)

`(listfile)` de `d2data.mpq` est un stub de 102 o (Blizzard l'a strippé en 1.12) —
mais **`d2exp.mpq` a un listfile complet** (429 716 o, 10 095 noms, 603 .ds1).
Lecture : `tools/mpq_list.py` (StormLib `SFileFindFirstFile`) ou `stormlib_read.py`.

On y trouve les **chunks outdoor de l'acte I**, exactement les presets que le DRLG
pose dans les wilderness :

```
data\global\tiles\ACT1\Outdoors\bord1..12(.b/.c/.o/.oe).ds1   bordures forestières
data\global\tiles\ACT1\Outdoors\LriverN/S, UriverN/S .ds1     berges de rivière
data\global\tiles\ACT1\Outdoors\bridge1/S/u/l .ds1            ponts
data\global\tiles\ACT1\Outdoors\stnclf6b.ds1                  falaise pierreuse
data\global\tiles\ACT1\Outdoors\waypoint.ds1                  plaine pure (waypoint)
+ townE1/S1/W1, crypt*, caves*, Monastry/facade1, tristram…   (57 .ds1 act1 au total)
```

### Anatomie d'un preset (v18, 9×9 cellules — taille stockée 8, piège +1 du Town)

| tag1 | contenu des couches | exemple |
|---|---|---|
| `(1,1)` | 4 couches de murs **seuls** (overlay) — PAS de sol | bord*o/oe, bridge* |
| `(2,1)` | `[mur1, mur2, mur3, mur4, SOL@4, ombre@5]` | bord2..5/10..12, Lriver*, waypoint, stnclf6b |
| `(3,1)` | `[mur1..mur6, SOL@6, ombre@7]` | bord6..9 (clairières avec chemins) |

Preuves (octets) : `bord11.ds1` = `12 00 00 00 | 08 00 00 00 | 08 00 00 00 | …` (v18,
w=8, h=8) puis 4 chemins `.dt1`, puis tag `02 00 00 00 01 00 00 00`, puis 6×81 int32 ;
`bord6.ds1` même entête avec tag `03 00 00 00` et 8×81 int32 (2 822 o au total).
Le sol (L4) d'un preset tag2 est **toujours plein** (81/81 cellules de tuiles sol).

⚠️ **Piège « tag »** : lire 6 couches sur un fichier tag1==1 déborde (nos premières
erreurs `unpack_from requires a buffer of at least 1791 bytes…` le prouvent) ;
à l'inverse un tag3 lu comme tag2 perd ses 2 couches de murs supplémentaires.

## 4. Composition (version simplifiée du DRLG, alignée OpenDiablo2)

OpenDiablo2 (`docs/OPENDIABLO2-NOTES.md` §2.3) : MapEngine = tableau de tuiles +
**stamps** (DS1 positionnés par `PlaceStamp`), et leur `d2mapgen/act1_overworld` est
« un petit générateur maison qui pose des stamps wilderness autour de la ville »,
avec **un seul RNG seedé**. C'est exactement ce que fait `bloodmoor_build.py` :

1. **Liste DT1 unifiée** (espace d'indices cumulatifs du DS1 écrit — ordre figé) :
   `out_treegroups(21), out_stonewall(52), floor(144), out_stones(41), out_river(45),
   bloodmoor_cliff2(13), bloodmoor_corner(3), bloodmoor_cliff1(13)` = 332 tuiles.
   Chemins embarqués dans le DS1 (style tg1) : `\d2\data\global\tiles\act1\…`.
2. **Remap des indices** : chaque preset a sa propre liste de DT1 → bases cumulatives
   propres. Une cellule preset `p1|p2<<8|…` est re-résolue : fichier = fwd-lookup de
   `idx=p1+(p2&0x0F)` dans les bases du preset, puis `new_p1 = base_unifiée[fichier]
   + (idx − base_preset[fichier]) − (p2&0x0F)`. Le bit d'orientation (`v>>24`) et p3
   voyagent tels quels. 5 175 cellules remappées, **0 hors bornes**.
3. **Canevas 72×72** (8×8 chunks de 9×9), couches tag2 `[mur1..4, sol, ombre]` :
   - sol = pool de cellules d'herbe **réelles** (tous les sols tag2 des presets,
     filtrés sur le fichier floor.dt1) brassées par RNG seedé (`SEED=20261022`) ;
   - anneau de bordure = chunks `bord*` complets posés sur le pourtour (forêt) ;
   - features intérieures : `waypoint` (3,3), `stnclf6b` ×2 (falaises),
     rivière 2×2 `LriverN/UriverN/LriverS/UriverS`, clairières `bord6..9` (chemins) ;
   - dispersion déterministe : 420 cellules d'arbres + 90 rochers (pools de cellules
     réelles de murs) + 50 taches de pierres au sol ; densité d'arbres 0.85/0.30/0.12
     selon la distance au bord (ceinture forestière irrégulière).
4. **Écriture DS1 v18** : `18,w-1,h-1,0,0,fc | chemins ASCIIZ | tag(2,1) | 6×W×H int32`
   (packés **non signés** puis interprétés signés : orient≥0x80 ⇒ bit de poids fort ;
   première version du script a planté sur `struct.error 'i' format` — piège n°3).
   Pas de section PATH (comme les presets ; seul TownN1 en a une, 1 164 o après les
   couches — inutile au rendu).

Vérifications intégrées : round-trip parse→assert (72×72, tag (2,1)), sol 100 % non
vide, indices `p1+(p2&0x0F) < 332` sur les 5 couches tuilées, `oob_remap == 0`.

## 5. Rendu + collision

Même pipeline que le Town (`map_proto2.Renderer`, palette `pal_act1_pl2.dat`,
gamma 0.7) ; ordre de dessin `4,0,2,3,1` (sol, mur1, mur3, mur4, arbres mur2 en
dernier). Rendu : 7 324 cellules dessinées, 0 hors cadre.

Collision : **BLOCK_MASK = 0x1000|0x2000|0x0004** inchangé (§4 de MAP-SPEC),
murs ⇒ bloqué, sol bloqué si ≥50 % de sous-tuiles bloquantes, **anneau extérieur
bloqué** (bord de carte, même convention que le Town). Résultat :
**1410 bloquées / 3774 libres** (73 % ouvert — une plaine), bords N/S/E/O scellés
vérifiés dans le JSON.

## 6. Ce que le moteur devra changer pour charger les 2 niveaux

État : `d2game::GameCore` lit les pointeurs `/level/*` de `assets/manifest.json`
(game.rs:721 `include_str!`, :108-117). Le contrat JSON ajoute `levels` sans toucher
`level`. Pour le multi-niveaux :

1. **Sélection du niveau** : `GameCore::new` gagne un paramètre `level_key`
   (`"town"`/`"bloodmoor"`) ou un `set_level(key)` qui relit les pointeurs
   `/levels/{key}/ds1|dt1|palette` ; `level` reste l'alias de `town`.
2. **`sizeTiles`** : bloodmoor stocke `[71,71]` (v18 : réel = +1 ⇒ 72×72) — le
   parseur Rust d2map applique déjà le +1 (piège du Town), rien à changer.
3. **Ordre des DT1** : l'ordre de la liste `dt1` de bloodmoor est **différent** de
   celui du Town et porteur du sens des indices — charger strictement dans l'ordre
   du manifeste (déjà le cas côté Town).
4. **Transition Town ↔ Blood Moor** : la sortie Est du Town (`warp` tuiles 217-297)
   devrait téléporter vers bloodmoor (et vice-versa) — le point d'entrée Blood Moor
   naturel est le chunk `waypoint` collé en (27,27) (zone chemins du centre).
5. **Monstres** : `patch_levels.txt` Blood Moor = `zombie1, fallen1, quillrat1`
   (déjà tous les trois dans le manifeste `monsters`) — MonDen=520, MonLvl 26/51/1.
6. Plus tard (DRLG complet) : remplacer le DS1 figé par une génération seedée côté
   Rust (port de `bloodmoor_build.py` : ~300 lignes, mêmes remaps).

## 7. Limites connues

- Les chunks tag1==1 (overlays walls-only : bord*o/oe, bridge*) ne sont pas posés
  (pas de sol propre ; les ponts attendent une logique de rivière à échelle fine).
- Les bandes d'arbres des chunks de bordure n'ont pas été réorientées par côté :
  la ceinture forestière est continue « assez » (visuel validé) mais pas pixel-parfaite
  comme le retail (qui choisit bordN/S/E/O via sa table interne D2Common).
- L'ombre (couches shadow) des presets est vide partout (vérifié) : non simulée.
- La collision reste à la résolution cellule (pas sous-tuile), comme le Town.
