# Audit de parité — domaine MOTEUR/CORE (OpenDiablo2 → diablo-2-wasm/engine-rs)

- **Date** : 2026-10-22
- **Référence Go** : OpenDiablo2 (repo cloné dans `/home/agent/workspace/opendiablo2`, archivé par ses auteurs) — réimplémentation Go de Diablo 2.
- **Port audité** : `/home/agent/workspace/diablo-2-wasm/engine-rs` (crates `d2game`, `d2wasm`, `d2tool` + supports `d2mpq`, `d2dc6`, `d2dcc`, `d2rules`, `d2missiles`) et démo navigateur `/home/agent/workspace/diablo-2-wasm/demo/`.
- **Périmètre du domaine** : `d2core/d2asset`, `d2core/d2map` (engine, entities, mapgen, renderer, stamp), `d2common/d2geom`, `d2common/d2path`, `d2common/d2math`, `d2common/d2cache`, `d2core/d2render`, `d2core/d2screen`, `d2core/d2input`, `d2core/d2ui`, `d2core/d2gui`, `d2core/d2audio`, `d2core/d2config`, `d2core/d2term`.
- **Hors périmètre** (autres domaines) : `d2records` (tables txt → couvert par d2rules, autre audit), `d2hero`/`d2stats`/`d2inventory` (gameplay), `d2networking` (N-A : port navigateur single-player), `d2script` (terminal Lua, N-A), `d2game/d2gamescreen` + `d2player` (écrans/HUD gameplay, autre audit).

**Modèle d'architecture du port** (à garder en tête pour lire les statuts) : le port est un moteur *data-centric WASM* — les assets sont extraits OFFLINE des MPQ et servis par le manifest `assets/manifest.json` ; le cœur Rust (`GameCore`) ne fait aucune IO fichier (blobs `Vec<u8>` fournis par `provide()`), tourne à l'identique en natif (`cargo test`) et dans le navigateur (wasm-bindgen), et rend dans un framebuffer RGBA blitté en JS. Beaucoup de « manques » sont en réalité des **choix d'architecture** (précomposition, DOM pour l'UI) plutôt que des trous fonctionnels — c'est signalé à chaque fois.

---

## 1. Tableau récapitulatif

Statuts : **FULL** (parité fonctionnelle, preuve test/usage) · **PARTIAL** (le cœur existe, des pans manquent) · **MISSING** (absent, pertinent au scope) · **N-A** (hors scope port navigateur/WASM centré données, justifié).

| # | Feature | Refs Go | Équivalent Rust / démo | Statut | Notes |
|---|---------|---------|------------------------|--------|-------|
| 1 | Accès assets (MPQ + filesystem loader) | `d2common/d2loader`, `d2common/d2fileformats/d2mpq`, `d2core/d2asset/asset_manager.go:90-129` (`LoadAsset`/`LoadFile`/`FileExists`) | `d2mpq` (`archive.rs`, `crypto.rs`, `dcl.rs` — validé 7/7 SHA-256), `WasmMpq` (`d2wasm/src/lib.rs:21`), `GameCore::provide` (`d2game/src/game.rs:179`) | **FULL** | Port modèle inversé : MPQ lus en natif (`d2tool`), blobs pré-extraits fetchés en JS. Go = lectures synchrones bloquantes ; ici zéro IO dans le noyau. |
| 2 | Cache d'assets budgeté (LRU) | `d2common/d2cache/cache.go:33-138` (`CreateCache`/`Insert`/`Retrieve`, budgets), `asset_manager.go:40-48` | Absent — `BTreeMap` tout-résident (`game.rs:96`) | **MISSING** | Inutile au stade actuel (cartes entières pré-composées, assets pré-extraits) ; à revoir si grandes cartes/streaming. Voir §d2cache. |
| 3 | Animations runtime (advance/loop/sub-loop/speed/backward/clone) | `d2core/d2asset/animation.go:67,74,345-405`, `dc6_animation.go`, `dcc_animation.go` | `SpriteSet::compose` 1 frame (`d2game/src/sprite.rs`), avancement `anim_frame` dans `d2rules::World::tick` (`d2rules/src/world.rs:471`) | **PARTIAL** | Play modes restreints (NU/WL/RN/A1/DD via `AnimState`) ; pas de sub-loop, play backward, `SetPlayLength`, `Clone`, `SetColorMod`, `DrawEffect` (transparences PCT). |
| 4 | Composite COF multi-couches (ordre COF, WC par couche, équipement) | `d2core/d2asset/composite.go:114,131,257` (`SetMode`/`Equip`/`createMode`) | `sprite.rs` (compose dans l'ordre `cof.draw_order(dir,frame)` via `d2dcc::render::composite_indices`), modes par manifest (`game.rs:443 build_entity_sets`) | **PARTIAL** | Le point dur Go (§1.4 de OPENDIABLO2-NOTES.md — WC par couche lu dans le COF) est correct : `sprite.rs` compose dans l'ordre du COF. Manque : équipement dynamique (`Equip`, armure lit/med/hvy, codes item pour RH/LH/SH), `animdata.d2` (frames/speed officiels), `palshift.dat`. |
| 5 | Palettes + transforms + langue | `asset_manager.go:131-149` (LoadLanguage), `LoadPalette`/`LoadTransform`, `d2pl2` | `d2game/src/pal.rs` (`MapPalette` PAL→LUT), palette units via `d2dcc::render::Palette` | **PARTIAL** | PAL terrain + units OK ; pas de PL2 (transforms), pas de détection de langue (fr-FR fixé à l'extraction). |
| 6 | Fonts + string tables (TBL) | `asset_manager.go` (`LoadFont`, `LoadStringTable`), `d2font`, `d2tbl` | Absent — textes HUD/menus en DOM avec polices navigateur | **MISSING** | Rendu texte in-engine (DC6 fonts + patchstring.tbl fr-FR) absent ; aujourd'hui acceptable (DOM), bloquant pour du texte « dans le monde » (labels d'items, dégâts flottants). |
| 7 | Map engine tiles (DS1+DT1, accès tile/subtile) | `d2mapengine/engine.go:74-111,203,211` (`ResetMap`/`AddDS1`/`addDT1`/`SubTileAt`/`TileAt`), `map_tile.go:18-31` | `d2game/src/ds1.rs`, `dt1.rs` (décodage v1.12), `compose.rs:37-76` (`ComposedMap` : framebuffer index + grille de marche 5×5 sub-tuiles), `game.rs:260 load_level` | **PARTIAL** | Décodage + composition complets et testés, mais **granularité collision simplifiée** : `compose.rs:15-18` (politique documentée : sol marchable / mur bloque la cellule / flag 0x2000), au lieu des flags DT1 PAR SOUS-TUILE de Go (`map_tile.go:18 GetSubTileFlags`). Deviation justifiée (flags v1.12 non discriminants, REPORT.md §3) mais moins fin que Go. |
| 8 | Génération de carte (region/preset → tiles) | `engine.go:320 GenerateMap`, `d2core/d2records` (lvltypes/lvlprest) | Absent — niveaux = DS1 fixes déclarés au manifest (`game.rs:235 resolve_level_def`) | **MISSING** | Go génère depuis les records `LevelType`/`LevelPreset` ; le port charge 2 DS1 extraits (town, Blood Moor) + warps. Élargir = parser lvlprest/lvltypes ou extraire plus de DS1. |
| 9 | Variation aléatoire de tuiles seedée | `map_tile.go:85 getRandomTile` (seed → même carte) | Absente — `compose.rs` déterministe (première tuile candidate) | **MISSING** | La répétitivité visuelle des sols/murs est visible ; petit port à fort gain (RNG seedé déjà présent : `game.rs:1004`, `d2rules/src/rng.rs`). |
| 10 | Pathfinding | `d2mapengine/pathfind.go:12-23` (`PathFind` = raycast LOS, renvoie UN point) | `d2game/src/astar.rs:45` (A* 8 directions, coûts 10/14, anti corner-cut) + `d2rules/src/path.rs:30,94` (astar + snap_near) | **FULL** | **Dépasse Go** : OpenDiablo2 n'a pas d'A* (limitation connue, leur issue #803) ; le port a un vrai A* sub-tile testé (5 tests `astar.rs`, tests `path.rs`). |
| 11 | Entités de carte (player/NPC/object/item/missile/overlay) | `d2mapentity/` : `factory.go:65-265`, `map_entity.go:77 Step`, `npc.go:19-77`, `missile.go:34`, `item.go`, `cast_overlay.go`, `object.go` | `d2rules/src/world.rs` (hero/monsters/items/missiles, `tick:471`, IA aggro/poursuite A* `:101-144`), `d2missiles` (système complet `lib.rs:185-321`), rendu `game.rs:850 blit_entity` | **PARTIAL** | Héros + monstres (avec IA — Go n'en a PAS) + items + missiles OK. Manquent : NPC avec paths DS1, objects (coffres/portes…), cast overlays, 64 directions (ici facing 0-7 → 15 dirs COF via `dir64_to_dcc`, suffisant MVP). |
| 12 | Stamps (DS1 préfabriqués posés à offset) | `d2mapstamp/stamp.go`, `factory.go`, `engine.go:161 PlaceStamp` | Absent | **MISSING** | L'unité de génération de Go (poser un DS1 + ses entités dans une carte hôte) n'a pas d'équivalent ; prérequis pour générer des mondes étendus (cf. #8). |
| 13 | Mapgen procédural (Act 1 overworld) | `d2mapgen/act1_overworld.go:38-273`, `d2wilderness/wilderness_tile_types.go` | Absent | **MISSING** | Le port est sur DS1 fixes ; pas de génération wilderness. |
| 14 | Renderer bas niveau (surfaces, états, filtres) | `d2render/ebiten/ebiten_renderer.go:72-176`, `ebiten_surface.go:72-291` (push translation/skew/scale/effect/filter/color/brightness/saturation), `filter_helper.go` | Framebuffer RGBA CPU : `game.rs:751 render` (blit LUT palette + blits sprites), `png_mini.rs` (export PNG natif), `putImageData` JS (`demo/game.js`) | **PARTIAL** | Rendu 2D logiciel fonctionnel (testé en E2E navigateur) ; pas d'abstraction Surface ni de filtres GPU (PCTTrans, brightness/saturation), pas de draw text (cf. #6). Choix d'architecture : WASM ne pilote pas le GPU ici. |
| 15 | Camera + viewport (3 espaces world/ortho/screen) | `d2maprenderer/viewport.go:38-215` (6 conversions, `IsTileVisible:119`, align toLeft/toRight `:196-215`), `camera.go:12-18` | `compose.rs:58-65,78-118` (`sub_to_screen`/`screen_to_sub`, espace world sub-tile ↔ screen), `game.rs:741 follow_camera` (clamp aux bornes), `game.rs:530 screen_to_world` | **PARTIAL** | 2 espaces (world sub-tile / screen) au lieu de 3 (l'ortho est implicite dans la précomposition) ; camera follow-only (pas de pan libre ni cible externe), pas d'alignement gauche/droite. |
| 16 | Passes de rendu + tri entités/murs + culling | `renderer.go:165 Render`, passes 1-4 `:231-358` (floors/walls, entités derrière/devant murs, roofs), tri par tuile+subtile, culling ±3 tuiles | Tri y global simple : ombres puis entités triées par screen-y (`game.rs:778-813`) ; blit viewport direct = culling gratuit | **PARTIAL** | Le port ne masque jamais une entité derrière un mur (pas de passes 2/3) — différences visibles dans les intérieurs ; le tri O(n log n) global suffit aux petites cartes. |
| 17 | Cache d'images de tuiles (composer une fois) | `tile_cache.go`, `renderer.go:145,684-689` | `compose.rs` : carte entière pré-composée UNE fois au `finalize` (`game.rs:195`), frames sprites pré-composées au boot | **FULL** | Même but (ne jamais recomposer par frame), approche plus radicale : framebuffer entier + LUT palette. Coût RAM index (1 o/px) ; invalidation palette inutile (palette fixe par niveau). |
| 18 | d2path (struct Path + actions DS1) | `d2common/d2path/path.go` (9 lignes : `Position`+`Action`), consommé par `npc.go` | Pas d'équivalent direct ; chemins runtime = `Vec<(f32,f32)>` suivis par `advance_along_path` (`d2rules/src/world.rs:233`) | **N-A** | Le paquet Go est un type trivial au service des paths de patrouille NPC (absents du scope MVP navigateur) ; le besoin réel (suivi de chemins) est couvert et testé côté d2rules. |
| 19 | d2geom (Point/Rectangle/Size) | `d2common/d2geom/point.go,rectangle.go:22 IsInRect,size.go` | Géométrie inline : `find_nearby_walkable` (spirale, `compose.rs:84`), `monster_near` (rayon, `game.rs:514`), test warp circulaire (`game.rs:718-720`), `Rect` interne `d2dcc/src/render.rs:57` | **PARTIAL** | Tous les usages réels de Go sont couverts inline ; pas de types réutilisables Point/Rect/Size (dette de factorisation, pas de trou fonctionnel). |
| 20 | d2math (Clamp/Lerp/epsilon, RangedNumber, Vector, Position tile/world, DirectionTo 64) | `d2common/d2math/math.go:18-172`, `ranged_number.go`, `d2vector/vector.go` (345 l.), `position.go:56-83` | Math inline f32 (`hypot`, `clamp`, `abs_diff`), `facing_from` 8 dirs (`d2rules/src/world.rs:202`), RNG seedé dédié (`d2rules/src/rng.rs`, `game.rs:1004`) | **PARTIAL** | Le nécessaire est inline et testé ; pas de lib vecteur (Angle/Reflect/Rotate… inutilisés au MVP), pas de RangedNumber (plages de dégâts gérées par `d2missiles::DamageRange`). |
| 21 | Boucle de jeu (update/draw cadencés) | `d2render/ebiten/ebiten_renderer.go:109 Run`, `d2app/app.go` (Advance/Render à 48-62 fps) | rAF navigateur → `D2Game.tick(dt,input)` + `render()` (`demo/game.js:693-704`, `d2game/src/wasm.rs:145-156`) | **FULL** | La boucle existe et est E2E-vérifiée (captures `demo/game-e2e.png`, `game-run-e2e.png`) ; le dt vient du rAF, events JSON. `cargo test` natif : 109 passed / 1 ignored / 0 failed. |
| 22 | Screen manager + écran de chargement | `d2screen/screen_manager.go:27-110` (état courant + `SetNextScreen`), `loading_state.go`/`loading_update.go` (progression) | Pages navigateur `menu.html` → `game.html` (navigation), fetch assets avant `finalize` | **PARTIAL** | Deux écrans existent (menu avec logo en flammes animé + sélection de classe, jeu) ; pas de machine à états dans le moteur ni d'écran de chargement avec ratio (le navigateur affiche son propre chargement). |
| 23 | Input (handlers priorisés, mods, curseur) | `d2input/input_manager.go:219-259` (`BindHandlerWithPriority`/`UnbindHandler`/`propagate`), `handler_event.go`, `d2input/ebiten/ebiten_input.go` | JS : click → `clickToInput` (`game.js:478,504`), keydown (`game.js:522`) → JSON `PlayerInput` (move/attack/pickup/setRun/useItem) parsé côté Rust (`d2game/src/wasm.rs:220 parse_input`) | **PARTIAL** | Click-to-move + attaques + raccourcis (I, Maj course, save/load) fonctionnels ; pas de système de handlers priorisés par écran, pas de keymap rebindable, pas de mods (ctrl/alt). |
| 24 | UI widgets (bouton, checkbox, label, scrollbar, textbox, tooltip, frame…) | `d2ui/` : `button.go`, `checkbox.go`, `label.go`, `scrollbar.go`, `textbox.go`, `tooltip.go`, `frame.go`, `ui_manager.go`, `color_tokens.go` | HUD/inventaire dessinés sur le canvas (`game.js:208-300` : globes D2, panneau 10×4), boutons DOM du menu (assets DC6→PNG extraits), bannières de zone, bouton son | **PARTIAL** | Le rôle (UI de jeu) est tenu par DOM+canvas JS ; pas de widget engine en Rust, pas de tooltips surfacés, pas de frames DC6 dynamiques. |
| 25 | GUI layout engine (flow vertical/horizontal, spacers) | `d2gui/layout.go`, `layout_entry.go`, `gui_manager.go`, `box.go`, `spacer.go` | Layout CSS/flexbox du navigateur | **N-A** | Le navigateur fournit un layout bien plus complet ; recoder un layout engine en Rust/WASM n'apporte rien au port. |
| 26 | Audio (SFX, panning, environnements sonores) | `d2audio/sound_engine.go:71,201` (`SetPan`, `PlaySoundID`), `sound_environment.go:32 SetEnv` (song + ambiances day/night par région), `ebiten/ebiten_audio_provider.go` | WebAudio JS : décodage WAV du manifest, anti-spam 70 ms, volume/vitesse par event, toggle (`game.js:152-205,420-462`) | **PARTIAL** | SFX d'événements OK (attaque, hit, mort par monstre, pickup, UI) ; **pas de panning positionnel**, pas de musique/ambiance par région (SoundEnvironment), pas de volume par distance. Le navigateur est le bon backend (desktop = pas de son natif requis). |
| 27 | Config persistante (fichier, defaults) | `d2config/d2config.go:25 Save`, `defaults.go`, `default_directories.go` (chemins MPQ, tick speed, langue) | `assets/manifest.json` (config de données : assets, niveaux, héros, sfx) + `localStorage` (héros choisi, save/load partie `game.rs:670-691 save_world/load_world`) | **PARTIAL** | La config données est le manifest ; les réglages runtime passent en localStorage ; pas de fichier de config structuré (inutile en WASM — pas de FS). |
| 28 | Console développeur (terminal) | `d2term/terminal.go:83-407` (`Bind`, `Execute`, categories log, easing), `terminal_logger.go`, commandes branchées (`renderer.go:118,133` debug vis) | Absent — DevTools navigateur + endpoints debug JSON (`monster_codes`, `compose_stats`, `sprite_notes`, `switch_notes`) | **MISSING** | Pas d'équivalent de `map debug vis`/`entity debug vis` ni de commandes runtime. Basse priorité (DevTools + probes E2E couvrent le debug courant). |

### Compte des statuts (28 lignes de features)

| Statut | Compte | Lignes |
|--------|--------|--------|
| **FULL** | **4** | #1 accès assets, #10 pathfinding, #17 cache/composition tuiles, #21 boucle de jeu |
| **PARTIAL** | **15** | #3, #4, #5, #7, #11, #14, #15, #16, #19, #20, #22, #23, #24, #26, #27 |
| **MISSING** | **7** | #2 cache LRU, #6 fonts/TBL, #8 génération, #9 variation tuiles, #12 stamps, #13 mapgen, #28 terminal |
| **N-A** | **2** | #18 d2path (type trivial, usage hors scope), #25 d2gui (layout navigateur) |

---

## 2. Sections détaillées par sous-système

### 2.1 d2asset — gestion d'assets, animations, composite

**Go.** `d2core/d2asset/` (1 749 lignes) :
- `asset_manager.go:64-80` : `AssetManager` = loader (`d2loader` MPQ/FS) + caches budgetés (`asset_manager.go:40-48` : `animationBudget` 128 Mo, dt1/ds1/cof/dcc…) + `Records *d2records.RecordManager` + tables `d2tbl` + langues (`:131-149`).
- `animation.go` (410 l.) : runtime d'animation générique — `Advance(elapsed):74` (accumulation fractionnaire), `SetSubLoop:67`, `PlayForward/PlayBackward/Pause:345-357`, `SetPlayLoop/SetPlaySpeed/SetPlayLength:363-377`, `SetColorMod:385`, `SetEffect:395` (DrawEffect = modes de transparence PCT), `SetShadow:400`, `Clone:405`, rendu d'ombre `:129`, rendu sectionné `:224`.
- `composite.go` (385 l.) : `Composite` = entité animée multi-couches — `SetMode:114`, `Equip:131` (armes/armure par couche), `createMode:257` : lit le COF et génère les chemins DCC **par couche avec le weapon-class du COF** (le savoir-faire clé, cf. OPENDIABLO2-NOTES.md §1.4), vitesse depuis `animdata.d2`.
- `dc6_animation.go:1-199`, `dcc_animation.go:1-199` : implémentations DC6/DCC derrière l'interface.

**Rust.**
- Accès : `d2mpq` (lecteur MPQ v0/v1 complet, DCL inflate, crypto — validé 7/7 SHA-256 contre le pipeline Python de référence) + `d2wasm::WasmMpq` (`d2wasm/src/lib.rs:21-44`) pour ouvrir un MPQ **dans** le navigateur. En démo, les assets sont pré-extraits et servis par manifest ; `GameCore::provide` (`game.rs:179-192`) les indexe (case-insensitive + basename).
- Animations : `d2game/src/sprite.rs` — `SpriteSet` (COF + couches DCC par token 2 lettres), `compose()` assemble un frame RGBA dans l'ordre `cof.draw_order(dir, frame)` (premier = fond), direction résolue par `dir64_to_dcc(facing*4, num_directions)` — exactement le principe Go §1.6. L'avancement temporel (`anim_frame` par `AnimState` NU/WL/RN/A1/DD) vit dans `d2rules::World::tick`. Modes absents/illisibles = skip gracieux + note (`sprite_notes_json`, `game.rs:106-108,627`).
- Palettes : `pal.rs` (PAL terrain → LUT RGBA), `d2dcc::render::Palette::load` (units, fix BGR).

**Écart vs Go.** Pas de runtime d'animation générique (play backward, sub-loop, playLength arbitraire, clone) ; pas d'équipement dynamique (`Equip` — les variantes d'armure `lit/med/hvy` et codes item pour RH/LH/SH ne sont pas résolus ; le manifest pointe directement les DCC) ; pas de `animdata.d2` (frames/speed viennent des DCC + constantes) ; pas de DrawEffect/palshift ; pas de fonts/TBL (ligne #6). **Statut global : PARTIAL.**

### 2.2 d2map/d2mapengine — moteur de carte

**Go.** `engine.go` : tableau plat `tiles []MapTile` (w×h tuiles, 5×5 sub-tuiles), `entities map[string]MapEntity`, seed RNG, `ResetMap:74` (levelType → DT1 files), `AddDS1:111`/`addDT1:87`, `TileAt:211`/`SubTileAt:203`, `GetTiles:246` + `getRandomTile` (`map_tile.go:85`, seedé → carte reproductible), `PrepareTile` (`map_tile.go:31`), `PlaceStamp:161`, `GenerateMap:320` (regionType + levelPreset + fileIndex via records), `Advance:291` (tick des entités), positions de départ/centre `:267-284`. `pathfind.go:12` : `PathFind` = **raycast LOS** (`checkLos:23`) qui renvoie au mieux un point d'arrêt — pas de A*.

**Rust.** Décodage `ds1.rs`/`dt1.rs` (quirks v1.12 validés contre `tools/map_proto2.py`), `compose.rs` : composition iso 80×40/16×8 en framebuffer d'indices palette (`ComposeMap.idx`) + **grille de marche 5×5 sub-tuiles** exportée au contrat `d2rules::World::new` (polarité inversée à l'entrée, `game.rs:14-16`). Niveaux : `game.rs:235 resolve_level_def` (bloc `levels` du manifest, alias historique `level` = town), `load_level:260`, `switch_level:346` (transfert héros stats/inventaire/xp, spawn snapé marchable), zones warp avec cooldown anti-rebond (`game.rs:28-30,715-737`), spawns monstres par niveau avec distances de sécurité (`game.rs:361-419`).

**Écart vs Go.** (a) Génération par records lvltypes/lvlprest absente (#8) — DS1 fixes ; (b) pas de variation seedée de tuiles (#9) ; (c) collision par sous-tuile simplifiée (politique documentée `compose.rs:15-18`, les flags DT1 v1.12 étant non discriminants) vs flags par sub-tuile de Go ; (d) pathfinding : **le port dépasse Go** (A* réel testé vs raycast). **Statut global : PARTIAL.**

### 2.3 d2map/d2mapentity — entités

**Go.** Hiérarchie `mapEntity` (position/velocity/target/path, `Step:77`, `SetPath:49`) ← `AnimatedEntity` (`rotate:62` 64 dirs, `Advance:72`) ← `Player` (`factory.go:65`), `NPC` (`:183` — paths de patrouille DS1 avec actions, `npc.go:69-77`, aucune IA), `Object` (`:265`, modes o1/o2), `Missile` (`:127`, `SetRadians:34`), `Item` (`:155`, flippy + label), `CastOverlay` (`:229`). Sélection/highlight d'entités (`Highlight`, `Selectable`, `Label`).

**Rust.** `d2rules/src/world.rs` : `Entity { x, y: f32, facing: u8 0-7, anim: AnimState, anim_frame }`, héros + monstres + items + missiles, `tick:471` avec **IA aggro/poursuite par A* avec re-path périodique** (`:101-144` — Go n'a aucune IA de monstre), mort/cadavres (`AnimState::DD`, `monster_alive:499`), items au sol (or, potions), save/load JSON du monde (`save.rs`). Missiles : crate `d2missiles` dédiée (`MissileSystem` : définitions JSON depuis missiles.txt pré-digéré, `spawn_weapon:263`, `tick:310`, collisions murs `tick_walls:321`, RNG seedé). Rendu : `game.rs:850 blit_entity` (couches COF), `monster_near:504` (routage clic attaque/déplacement), draw order y-sort (`game.rs:778-813`).

**Écart vs Go.** NPC de patrouille DS1, objects interactifs, cast overlays, flippy DC6 des items (les items sont des marqueurs dessinés, `item_marker`), labels/survol d'entités, 64 directions. En face : IA monstres + missiles + save/load que Go n'a pas (ou pas fini — leur missile `Advance` incomplet, issue #819). **Statut global : PARTIAL.**

### 2.4 d2map/d2mapgen + d2mapstamp — génération

**Go.** `d2mapgen/map_generator.go` + `act1_overworld.go:38-273` : génère l'overworld Act 1 (town + wilderness E/S/O posés comme stamps de tuiles wilderness). `d2mapstamp/` : `Stamp` = DS1 préfabriqué (tuiles + entités) posable à offset (`engine.go:161 PlaceStamp`) — l'unité de génération de niveau.

**Rust.** Absent. Les niveaux viennent de DS1 extraits entiers (town, Blood Moor) reliés par warps manifest. C'est le plus gros écart de capacité « monde » du domaine — mais aussi le moins nécessaire pour un port navigateur centré données au stade MVP (chaque DS1 de Blizzard remplace une génération procédurale). **Statuts : MISSING (les deux).**

### 2.5 d2map/d2maprenderer — rendu de carte

**Go.** `renderer.go:165 Render` : culling par rect visible (marge ±3 tuiles pour murs hauts), **4 passes** `:231-358` (1 floors/shadows/walls basses, 2 entités derrière murs, 3 murs hauts + entités devant, 4 roofs), tri entités par bucket (tuile, subtile) — pas de z-sort global ; `tile_cache.go` + `getImageCacheRecord:684` : chaque tuile (style,sequence,type,randomIndex) composée en image une fois, invalidée si palette change ; palette **par acte** (`loadPaletteForAct:627`) ; debug vis tuiles/entités (`:118-133,452-548`) ; camera libre (`camera.go`) ; `viewport.go` : 3 espaces (world/ortho/screen), 6 conversions (`:61-110`), `IsTileVisible:119`, alignements gauche/droite (`:196-215`).

**Rust.** `compose.rs` : la carte entière est composée **une fois** au `finalize` en framebuffer d'indices (≈ cache de tuiles poussé à l'extrême — ligne #17 FULL) ; par frame, `game.rs:751 render` = blit viewport (LUT palette) + ombres + sprites triés par y (`:778-813`). Conversions world sub-tile ↔ screen (`compose.rs:58-65,78-118`), camera follow centrée clampée (`game.rs:741-748`), `screen_to_world:530` pour les clics, stats de composition exposées (`compose_stats:563`).

**Écart vs Go.** Pas d'occlusion par murs (passes 2/3 — une entité derrière un mur reste visible), pas de roofs, camera follow-only, pas d'alignement viewport alternatif, pas de debug vis intégré. Compensé par : précomposition (CPU par frame ~constant), palette par niveau (équivalent par-acte). **Statuts : #14 PARTIAL, #15 PARTIAL, #16 PARTIAL, #17 FULL.**

### 2.6 d2common/d2geom / d2path / d2math / d2cache

- **d2geom** (`point.go` 13 l., `rectangle.go` 24 l., `size.go` 6 l.) : types triviaux ; tous les usages réels du port sont inline et testés (`find_nearby_walkable`, `monster_near`, warp circulaire, `Rect` de `d2dcc/render.rs`). Pas de types réutilisables. **PARTIAL.**
- **d2path** (`path.go` : `struct { Position, Action }`, 9 lignes, consommé uniquement par les NPC paths DS1) : hors scope MVP navigateur ; le suivi de chemins runtime existe dans `d2rules` (`advance_along_path`, facing mis à jour). **N-A.**
- **d2math** (`math.go` : Clamp/Lerp/epsilon/angles ; `ranged_number.go` ; `d2vector` 426 l. + `position.go` avec `World()/Tile()/RenderOffset()/DirectionTo()` 64 dirs) : le port fait l'essentiel inline (`facing_from` 8 dirs `d2rules/world.rs:202`, distances/lerp f32), RNG seedés dédiés (`d2rules/rng.rs`, `game.rs:1004`, `d2missiles/rng.rs`). Pas de lib vecteur complète, pas de RangedNumber (plages gérées dans `d2missiles`). **PARTIAL.**
- **d2cache** (`cache.go` : LRU à budget avec `GetWeight`, eviction, verbose) : rien d'équivalent — tout est résident (`BTreeMap` blobs, framebuffer entier, sprites pré-composés). Cohérent avec le modèle WASM (assets extraits offline, cartes modestes) ; deviendra nécessaire pour grandes cartes/streaming. **MISSING (priorité basse).**

### 2.7 d2render / d2screen / d2input — platine de présentation

- **d2render** : Go = backend Ebiten (fenêtre, vsync, fullscreen, surfaces à pile d'états, filtres). Port = framebuffer RGBA CPU dans `GameCore` + `putImageData`/canvas côté JS ; `png_mini.rs` exporte les PNG natifs (screenshots de tests). Fonctionnel et E2E-vérifié, mais sans abstraction Surface ni effets (PCTTrans, brightness/saturation — les modes de blend D2 des animations et palshift). **PARTIAL.**
- **d2screen** : Go = manager à état courant + next + loading screen à progression. Port = pages navigateur (`menu.html` avec logo en flammes 30 frames + sélection de classe + continue, `game.html`) — navigation par URL, chargement = fetch JS. Le mécanisme de machine à états in-engine n'existe pas (et n'est pas indispensable en navigateur) ; les transitions d'états gameplay (menu→jeu, avec choix de héros) sont couvertes. **PARTIAL.**
- **d2input** : Go = gestionnaire à handlers priorisés + mods + rebinding. Port = listeners JS (click, keydown) qui fabriquent des `PlayerInput` JSON consommés par `tick` (`wasm.rs:220`) ; inventaire/clic-route (`clickToInput:478`), touche I, Maj = course, boutons son/save/load. Le routage de clic (monstre le plus proche dans un rayon → attaque, sinon déplacement) est côté Rust (`monster_near`). Pas de handlers priorisés multi-écrans ni de keymap configurable. **PARTIAL.**

### 2.8 d2ui / d2gui

- **d2ui** : 17 fichiers Go de widgets (boutons/checkbox/textbox/tooltip/frames DC6…). Port : HUD sur canvas (globes vie/mana dessinés, barre XP, or, zone, bannière de transition), panneau inventaire 10×4 dessiné sur canvas (`game.js:293`), menus en DOM avec les assets DC6 extraits en PNG. Le *rôle* est couvert, pas le *mécanisme* (aucun widget engine en Rust). **PARTIAL.**
- **d2gui** : layout flow maison. Le navigateur (flexbox) fait mieux pour l'usage du port. **N-A.**

### 2.9 d2audio / d2config / d2term

- **d2audio** : Go = `SoundEngine` (SFX par id, panning `SetPan`, stop, reset) + `SoundEnvironment` (musique + ambiances jour/nuit par région, depuis sounds.txt/env.txt) + provider Ebiten. Port = WebAudio (`game.js:152-205`) : WAV du manifest décodés en buffers, déclenchement sur événements (`attack`, `hit`, `die` par code monstre, `pickup`, UI), volume/vitesse variables, anti-spam 70 ms, toggle. Manquent panning positionnel, musique/ambiance par région. **PARTIAL.**
- **d2config** : Go = fichier JSON utilisateur (chemins MPQ, vitesse de tick…). Port : manifest = config de données ; réglages/save en localStorage (choix de héros, save/load du monde via `save_world/load_world` `game.rs:670-691` + boutons démo). Pas de config structurée hors données. **PARTIAL.**
- **d2term** : console déroulante avec commandes bindables (debug vis carte/entités, sons, etc.). Rien d'équivalent ; DevTools + endpoints debug JSON (`compose_stats`, `sprite_notes`, `monster_codes`, `switch_notes`) et probes E2E tiennent lieu. **MISSING.**

### 2.10 État des tests (vérifié ce jour)

`cargo test` (native, engine-rs) : **109 passed / 1 ignored / 0 failed** — dont d2game 28 (pipeline complet manifest→finalize→tick→render, transitions town↔bloodmoor, warp round-trip, transferts héros), d2missiles 42, d2mpq (SHA-256 de référence), d2dc6/d2dcc, d2rules (contrat monde, A*, save). La même base `GameCore` tourne dans le navigateur (E2E Chrome documentée dans `engine-rs/REPORT.md` §6-7, captures `demo/game-e2e.png`, `game-run-e2e.png`, `menu-e2e.png`).

---

## 3. Manques priorisés

**P1 — impact gameplay visible (prochaine itération)**
1. **Occlusion par les murs (passes de rendu)** (#16) — port du principe Go passes 2/3 : deux couches (sol+ombres / murs hauts) ou buckets par tuile ; un mur doit pouvoir masquer le héros. Pré-requis : garder le framebuffer d'indices mais blitter les murs hauts après les entités (réordonner la composition en 2 tampons).
2. **Flags DT1 par sous-tuile** (#7) — réessayer la discrimination des flags (peut-être un masque différent en v1.12 ; l'équipe Go avait les flags complets) pour coller au collision fin au lieu du blocage par cellule.
3. **animdata.d2 + vitesse/frames exactes + events sonores** (#4) — débloque les vitesses d'animation officielles et les triggers SFX de pas/coups (branchés au WebAudio existant).
4. **Variation aléatoire de tuiles seedée** (#9) — port direct de `getRandomTile` ; gros gain visuel pour ~30 lignes (RNG déjà présent).
5. **Items flippy DC6 + labels de survol** (#11/#24) — remplacer le marqueur d'item par le DC6 flippy + label DOM/canvas.

**P2 — capacité moteur (après la boucle gameplay)**
6. **Stamps DS1** (#12) puis **génération/preset** (#8) — pour grandir au-delà des 2 DS1 sans tout extraire ; parsing lvlprest/lvltypes ou extraction ciblée.
7. **DrawEffects + palshift.dat** (#4/#14) — variantes de couleur champions/uniques (personne ne l'a ; les décodeurs palette existent déjà).
8. **Audio : panning positionnel + ambiance par niveau** (#26) — prolongement WebAudio simple (stéréo panner, loop d'ambiance par `levels.key`).
9. **Fonts/TBL in-engine** (#6) — seulement si besoin de texte dans le monde (dégâts flottants, labels) ; sinon DOM reste suffisant.
10. **Cache LRU / streaming** (#2) — à l'ouverture de cartes beaucoup plus grandes uniquement.

**P3 — confort/debug (basse priorité)**
11. Console dev type d2term (#28) — commandes debug vis, spawn, god mode ; les probes E2E JSON couvrent déjà l'essentiel.
12. Types geom/math factorisés (#19/#20) — dette de factorisation, pas de besoin fonctionnel immédiat.
13. Input rebindable + handlers priorisés (#23) — pertinent si plusieurs écrans gameplay se superposent.

**Non planifié (N-A assumé)** : layout engine d2gui (CSS), paths NPC DS1 (d2path), 64 directions (8-15 suffisent, conversion `dir64_to_dcc` déjà correcte), networking, scripting Lua.

---

*Audit statique (lecture de code + tests), aucune écriture de code. Sources Go citées chemin:ligne depuis le clone `/home/agent/workspace/opendiablo2` ; port cité depuis `/home/agent/workspace/diablo-2-wasm/engine-rs` et `../demo`.*
