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
