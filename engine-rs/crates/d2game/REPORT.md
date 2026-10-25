# d2game — rapport (rédigé a posteriori depuis le handoff du membre moteur)

## Ce que fait la crate
Moteur de rendu WASM : compositor de la carte réelle (TownN1.ds1 + 14 DT1 → framebuffer RGBA),
sprites DCC/COF (via d2dcc) superposés par y, caméra centrée héros, A* 8 directions,
façade wasm-bindgen 0.2.129 (`D2Game.new/provide/setViewport/finalize/tick/render/
screenToWorld/heroScreen/collisionGrid`). Preuves natives : engine-rs/target/d2game-proof/
(map-compose.png identique à l'oracle, collision-heat.png, sprite-plate.png, viewport-render.png).

## Pièges DT1/DS1 rencontrés
- Layout DS1 v18 du Town ; tg1 ≠ LvlType (ne pas confondre)
- Murs : coordonnées y négatives dans les layers
- DT1 : orientation 0 = sol — les « murs ori-0 » des DT1 de murs sont des sols

## Politique collision (release 1.12)
Flags de sous-tuiles non discriminants dans cette release → règle retenue : sols marchables,
murs bloquants, bit 0x2000 = sous-région bloquée. Ratio marchable mesuré : 0.84.

## Gotcha wasm-bindgen
Les noms snake_case Rust sont conservés côté JS sans `js_name` explicite — le driver JS doit
appeler `mapPixelSize()` etc. (camelCase du .d.ts), pas `map_pixel_size()`.

## Contrat d2rules
Stub remplacé par crates/d2rules (contrat identique : World::new/tick/hero/monsters/items/
hero_stats/save/load, PlayerInput, GameEvent). Caveats : MonsterDied.index = position dans
monsters() au moment de l'event ; ItemDef littéraux avec ..Default::default() (champ additif amount).

## 9d — AutoMap / monde vivant (automap.rs, 2026-10-24)
Résolution `AutoMap.txt` v1.12 (3 287 lignes, 3 286 records utiles — 1 expander
« Expansion » ignorée) embarquée (`assets/AutoMap.txt`), groupée par
`(acte, nom LvlTypes court)`. Endpoints : `GameCore::automap_json()` natif,
`game.automap()` wasm — consommé par `demo/automap.js` (mini-carte coin,
fog of war disque r=9, toggle Tab + bouton Carte du minipanel).

### Pièges mesurés (preuves dans les tests)
- **Méta DT1** : table réelle à `276 + 96k` (bodyPosition@272) avec
  `Type@+20`, `Style@+24`, `Sequence@+28` ; le port lit `252 + 96k` (tuile
  réelle k−1, tuile 0 fantôme vide) → `dt1_meta_aligned` rend n méta alignées
  sur la table du port (même tuile peinte ⇒ même style/seq).
- **Acte du groupe AutoMap** : la colonne `Act` de patch_levels.txt vaut 0
  pour les niveaux génériques (ids 1/2) — l'acte réel est dans le NOM LvlTypes
  (« Act 1 - Town ») → `split_act_prefix` (repli colonne Act).
- **Matching** : `(Style, plage Sequence)` — premier record ORDRE FICHIER ;
  `Start < 0` = joker (seul Style) ; `End < 0` = pas de borne (seq ≥ start,
  rangée réelle `wbr 0 5 -1`). TileName non matché (approximation documentée ;
  les murs style 0 peu-seq peuvent hériter d'un cel de sol — cosmétique pour
  l'overlay, cf. alternative orientation→TileName d2mods #468).
- **Cels déterministes** : première valeur `Cel1..4 ≥ 0` (le jeu réel tire au
  hasard — Go automap_record.go:43-45 ; le port exige le déterminisme testé).

### Fallback documenté
Sans tables levels / `levelsId` / groupe AutoMap : `mode:"fallback"`, `cel:-1`
partout — la démo dessine le polygone marchable (cellules `walk` seules). Les
`cel` indexent les frames de `MaxiMap.dc6`, sprite absent du port : le JSON
expose les indices, jamais de pixels.

### Format JSON (`automap()`)
`{"level","name","mode":"automap|fallback","lvlType","cellW","cellH","sub":5,
"cells":[[x,y,flags,cel],…]}` — flags bit0=walk, bit1=wall, bit2=special
(L2/L5) ; une entrée par cellule VISIBLE (mêmes gardes que le composeur).

## 20a — DRLG wilderness retail structuralement fidèle (drlg_exact.rs, 2026-10-25)

### Livré
`crates/d2game/src/drlg_exact.rs` (nouveau) + 1 ligne `pub mod drlg_exact;` dans
lib.rs — 11 tests, workspace 1026 verts / 0 rouges / 2 ignorés (baseline 1015/0/1).

### Sources de l'algorithme (relues et citées, contenu = données)
- **D2MOO** (MIT, décompilation restructurée 1.10f) — https://github.com/ThePhrozenKeep/D2MOO :
  `source/D2Common/src/Drlg/DrlgOutdoors.cpp` (grille SizeX/8, SpawnOutdoorLevelPresetEx
  round-robin `Files`, SpawnOutdoorLevelPreset Fisher-Yates 2 tirages/pas, fill acte I
  Dt1Mask `0x44103`), `.../Drlg/DrlgOutWild.cpp` (séquence acte I complète : bordures
  secondaires LvlSub, rivière colonne gw-2 + LUT fichiers 12 entrées, caves de falaise,
  Den of Evil en quadrant RNG, transitions ville par bits 0x80..0x400, waypoints 3..6,
  5 sanctuaires, spéciaux Blood Moor), `.../Drlg/DrlgOutPlace.cpp` (bordures par LUT
  `nBorderIndices`/`levelPrestBorder`, coins `sub_6FD80C10`), `.../Drlg/DrlgTileSub.cpp`
  (AddSecondaryBorder/TestReplace/ReplaceSubPreset, BordType 0/1/2), `.../Drlg/DrlgPreset.cpp`
  (schéma DS1 retail + groupes de substitution v≥12), `source/D2Common/include/D2Seed.h`
  + `src/D2Seed.cpp` (**RNG retail exact** : `seed = high + 0x6AC690C5 * low` mod 2⁶⁴,
  init low=graine/high=666, `RollLimitedRandomNumber` modulo-ou-masque-pow2).
- **Données réelles v1.12** : `assets/{patch_levels,LvlPrest,LvlTypes}.txt` (embarqués) ;
  `evidence/lvlSub.extracted.txt` + `evidence/sub_ds1/Border{Cliffs,Middle,Corner,Border}.ds1`
  extraits de `d2data.mpq` (StormLib) ; 65 presets acte I dans `evidence/exact_ds1/`
  pour la banque de test de marchabilité. Golden structurel 3 graines :
  `evidence/drlg20a-golden.json`.

### Ce qui est fidèle au retail (validé par tests contre données réelles)
1. **RNG** : identique bit à bit (référence indépendante Python de D2Seed.h —
   graines 1 et 42 : 8 rolls + 2 séries limited(64)/limited(10) exactes). La
   constante communautaire 0x343FD ne concerne PAS 1.10f ; c'est bien 0x6AC690C5.
2. **Grille** : `SizeX/8 × SizeY/8` = 10×10 pour la Blood Moor (80×80), grilles
   0/1/2 aux sémantiques retail (Def, flags vis/sanctuaire/waypoint, info packed).
3. **Ordre d'appels acte I** : bordures LUT → LvlSub CLIFFS → (rivière → caves →
   DOE quadrant) → LvlSub MIDDLE → CORNER → transitions ville → LvlSub BORDER →
   waypoints (3..6) → 5 sanctuaires → spéciaux. Blood Moor : pas de waypoint ✓.
4. **Bordures** : LUT directionnelles retail — O=Bord1/S=Bord3/E=Bord4/N=Bord2
   (arêtes) + coins LUT (Bord11/12 ici), cells marquées b00 invalides au spawn.
5. **Fisher-Yates retail** : une passe, DEUX tirages par pas, branche pow2
   (64 cases intérieures → `& 63`) reproduite.
6. **Sous-placements LvlSub réels** : groupes de substitution décodés dans les
   4 DS1 de bordure du MPQ (15/9/8/20 groupes), BordType 0/1/2, variantes
   `field_14`, encodage wall (byte1-1+WILD_BORDER_1, exempt 62) — posés aux
   mêmes étapes que le retail (30-36 presets LvlSub par graine).
7. **Spéciaux Blood Moor** : étang (46) toujours, cottages (47) ×1-2 dont le
   second conditionnel `!(roll&3)`, stone fills 29/30, DOE (52) en quadrant RNG
   x∈{3,4} y∈{3,6}, 5 sanctuaires à flags cycliques 0x1000..0x8000.
8. **Dt1Mask** : garde data-driven Dt1Mask ⊆ tileset LvlTypes (bit n-1 = File n)
   sur tout preset posé.

### Ce qui n'est PAS fidèle (écarts assumés, chiffrés)
1. **Polygone de niveau** : le retail suit le polygone Vis du graphe d'acte
   (Blood Moor = L-shape autour du camp) ; le port utilise le rectangle 10×10
   autonome → les bordures LUT couvrent 40 cellules d'anneau au lieu de suivre
   les segments réels ; `SetOutGridLinkFlags` est no-op (aucun bLvlLink).
2. **Fill intérieur symbolique** : le retail construit chaque cellule non-preset
   en room de tuiles depuis `Dt1Mask 0x44103` (`CreateOutdoorRoomEx`, résolution
   mur/sol DT1) ; le port marque `CellKind::Fill` et approxime ouvert/marchable
   → le rendu DT1 du fill reste à faire (d2mapapi 1.13 le fait via DLL du jeu).
3. **Chemins de terre acte I** (`SpawnAct1DirtPaths` + table byte_6FDCF958) :
   peinture de sol au niveau room — non porté (les tirages RNG s'arrêtent avant ;
   le compteur `rng_rolls` (~5 400/graine) est donc inférieur au retail réel).
4. **Flags autonomes** : `0x10|0x80|0x100` (lien ville + 2 transitions sud)
   posés en dur faute de graphe d'acte ; `generate_exact_with_flags` permet de
   les remplacer par les vrais bits quand le graphe existera.
5. **Collision par cellule** : `walk_grid` colle les DS1 9×9 à pas de 8 avec
   murs bloquants/sol ouvert — pas de sous-tuiles DT1 (composeur existant
   disponible pour le pixel-perfect plus tard).

### Validation (oracle) — pourquoi « golden » et pas d2mapapi
- **d2mapapi** (jcageman, C#) et son port C++ `d2mapapi_mod` (soarqin) sont des
  wrappers des DLL du jeu 1.10-1.13 (d2ptrs/offsets) : **non buildables
  standalone** (pas de Diablo 2 ici), donc l'oracle seed→carte retail n'a pas
  pu tourner. (Le « Rhuvaer » du brief : compte GitHub inexistant — 404 ; les
  dépôts réels sont jcageman/d2mapapi et soarqin/d2mapapi_mod, MIT.)
- **Golden structurel** : `evidence/drlg20a-golden.json` — 3 graines (0,
  12345, 20261025) : 10×10, ~78-84 placements, 5 389-5 501 tirages RNG,
  ratio marchable 0.78-0.80, 5 sanctuaires, sources {"border-lut":36,
  "border-corner":4, "lvlsub":30-36, "special":4-5, "doe-quadrant":1,
  "town-trans":2}. Invariants testés (recette retail) : étang 46 ≥1, cottages
  47 ∈ {1,2}, fills 29/30, DOE 52 quadrant, shrines=5 à 4 directions, PAS de
  waypoint, PAS de rivière hors flags, anneau ⊆ Defs 2..15.
- **Données croisées** : LvlPrest Def/SizeX/SizeY/Files/Dt1Mask et lvlSub
  Type/BordType/GridSize lus des tables v1.12 réelles ; encodage subst des DS1
  vérifié octet à octet (0x3000d81 → byte1=13 → Def 16 CLIFF_BORDER_2).

### Ce qu'il resterait (pour un clone complet)
1. Graphe d'acte (DrlgDrlg/LevelDefBin offsets) → polygone Vis réel + anchors
   Rogue Encampment/Monastère pour SpawnPresetFarAway et les chemins.
2. `DRLGOUTPLACE_CreateOutdoorRoomEx` : construction des tuiles du fill
   (résolution Dt1Mask → DT1, murs de bordure de tuiles) + `DRLGPRESET_BuildArea`
   (posé DS1 → tuiles avec résolution d'orientation) → un vrai canevas DT1.
3. Chemins de terre (table byte_6FDCF958 + sommets).
4. Types LvlSub non-bordure (waypoints/wells/wild objects des types 4+) et
   autres niveaux acte I (StonyField..Tamoe : spéciaux déclinés dans SpawnSpecialPresets).
5. Oracle : faire tourner d2mapapi sur une machine avec le jeu (ou un report
   standalone du DRLG D2Common) pour comparer seed→grille preset par preset.

## 20b — Zones spéciales : Tristram + portail des vaches (drlg_special.rs, 2026-10-25)

### Livré
`crates/d2game/src/drlg_special.rs` (nouveau) + 1 ligne `pub mod drlg_special;` — 12 tests,
workspace **1038 verts / 0 rouges / 2 ignorés** (baseline 20a : 1026/0/2). Le « maze exact »
prévu au même plan est REPORTÉ : couvert côté wilderness par `drlg_exact.rs` (étape 20a,
verdict chiffré §20a ci-dessus).

### 1. Tristram (niveau 38) — preset figé réel, pipeline DS1 existant
- **Extraction MPQ LECTURE SEULE** (StormLib) : le chemin du brief `ACT1\Tristram\tristram.ds1`
  n'existe pas dans la release v1.12 (sonde None sur les 4 MPQ) — la carte réelle est
  `data\global\tiles\act1\tristram\tri_town4.ds1` (d2exp.mpq, listfile) → `assets/map/tristram.ds1`
  (62 475 o, md5_8 `62162f44`) ; son DT1 `data\global\tiles\ACT1\Tristram\town.dt1`
  (d2data.mpq, sonde par hash — listfile strippé) → `assets/map/tristram_town.dt1`
  (1 560 481 o, md5_8 `d5792185`).
- **En-tête réel** : v18, 44×49 (= SizeX/Y+1 du record 38 : 43×48), 7 fichiers embarqués,
  **2 murs + 2 sols = 7 flux**. Distribution réelle : 431 murs (286 palissades 1/2,
  122 arbres 14, 2 feus 12, **1 porte-warp 10 — cellule (34,16), HIDDEN : le portail
  y est DYNAMIQUE dans le retail**), 2 156 sols, ombre vide.
- **Montage** : projection (2,2)→(2,1) sans perte de marchabilité (murs/orients bruts
  conservés ; floor2 n'a aucun blanc à combler — 33 cellules superposées, écart
  documenté) → `assets/map/tristram-engine.ds1` → **pipeline réel** `ds1_real` +
  `compose_real_town` (zéro changement du moteur). Test de fidélité : **0 cellule non
  résolue** sur les (kind,style,seq) du preset contre les 7 DT1 réels ; 2 461 tuiles
  dessinées ; ville en UNE composante connexe de 44 661 sous-tuiles (ratio 0,86).
- **Blocs manifeste** (`assets/manifest.json` + miroir demo, additifs) : `levels.tristram`
  (levelsId 38, 7 DT1, palette acte I, monstres retail SK/GM/FS) + **warp retour** posé
  SUR la cellule porte-warp réelle (sous-tuiles (172,82)) vers la Lande sanglante.
  `?level=tristram` jouable dans la démo (fetch automatique des blocs).
- **Portail rouge des Cairn Stones** : ponts additifs GameCore (`tristram_portal_json`,
  `open_tristram_portal` — proximité de l'ancre (160,160) exigée, refus fr lisible sinon).
  Écart : le repère visuel des pierres n'est pas posé par le DRLG 20a — ancre moteur fixe.

### 2. Portail des vaches (niveau 39, Moo Moo Farm) — chaîne moteur bout-en-bout
- **Items réels vérifiés** : `leg` = Wirt's Leg (`patch_weapons.txt`, code `leg`, type club)
  et `tbk` = Town Portal Book (`patch_misc.txt`, code `tbk`) — le hook cube existe depuis
  l'étape 11a (L4 de `patch_cubemain.txt`, sortie non-item « Cow Portal ») ; le
  « branchement gameplay restant » de `cube_ui.rs` est fait côté moteur :
  `GameCore::open_portal_from_transmute` (consomme la réponse JSON de `cube_transmute`,
  ouvre le niveau 39 ; rejets fr pour toute autre réponse — jamais de switch silencieux).
- **Gabarit HELLBOVINE** ajouté au manifeste (`monsters`, stats retail approximées MonLvl 28,
  note explicite) : `World::parse_monsters` résout le code → les 12 packs spawnent
  (sprites DCC bovine non extraits — entités sans rendu, écart documenté).
- **Canevas généré** (Moo Moo Farm = DrlgType 3 sans DS1 figé dans la release) :
  `moomoofarm_engine_ds1` — 80×80 (record), prairie/pierres/arbres/roche **data-driven**
  des groupes (kind,style,seq) réels des 6 DT1 (kinds clôtures 1/2, arbres 14,
  porte-warp 10), 4 enclos clôturés avec portes, tuile porte-warp du portail rouge à
  l'entrée, RNG retail exact `drlg_exact::D2Seed` (étape 20a). **Déterminisme testé**
  (égalité tranchée par clé minimale — l'ordre d'itération HashMap n'est pas stable),
  test d'or `assets/map/moomoofarm-engine.ds1` (graine 20261025) + bloc
  `levels.moomoofarm` (`?level=moomoofarm` jouable).
- **Test bout-en-bout** : save patchée (leg+tbk slots 0/1, items des tables) →
  `cube_transmute` (L4, items consommés) → `open_portal_from_transmute` → niveau 39
  chargé, ≥ 8 entités `HELLBOVINE`, prairie connexe > 8 000 sous-tuiles, héros sur
  tuile marchable.

### Écarts assumés (annoncés dans le module)
1. floor2 de Tristram (33 cellules de variation) non superposé — 1,5 % des sols, 0 impact collision.
2. Portail de Tristram : cellule HIDDEN réservée (portail dynamique retail) → le warp
   du bloc y est ancré ; l'objet cliquable visuel reste à brancher (overlays/objets).
3. Ancre Cairn Stones fixe (160,160) — le repère des pierres viendra avec les presets 20a.
4. Vaches sans sprite (DCC bovine non extrait) — spawn/IA/combat au contrat.
5. Portail des vaches à sens unique au niveau moteur ; `Pandemonium Portal` (L150)
   non câblé (rejet explicite).
6. Ponts GameCore natifs (pas d'export wasm.rs — fichier hors périmètre 20b) : le
   branchement page (boutons portails) suivra à l'intégration.
