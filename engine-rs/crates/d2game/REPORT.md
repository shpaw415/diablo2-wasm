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
