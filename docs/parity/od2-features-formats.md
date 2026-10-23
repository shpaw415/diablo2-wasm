# Audit de parité OpenDiablo2 → diablo-2-wasm — Domaine « FORMATS DE FICHIERS »

> Audit du 2026-10-22. Référence : repo Go **OpenDiablo2** (cloné dans `/home/agent/workspace/opendiablo2`,
> archivé par ses auteurs). Port Rust : `/home/agent/workspace/diablo-2-wasm/engine-rs`.
> Périmètre : `d2common/d2fileformats` + `d2common/d2loader` + `d2core/d2records`.
> Specs du port : `evidence/DC6-SPEC.md`, `evidence/DCC-SPEC.md`, `evidence/MAP-SPEC.md`,
> `evidence/MAP-SPEC-BLOODMOOR.md`, `docs/GAME-DATA.md`, `docs/OPENDIABLO2-NOTES.md`.

**Statuts** — `FULL` : équivalent Rust présent et validé sur fichiers réels (simplifications délibérées documentées) ;
`PARTIAL` : capacité présente mais clairement plus étroite ; `MISSING` : rien côté Rust ;
`N-A` : hors scope du port navigateur WASM centré données (justifié au cas par cas).

---

## 1. Tableau récapitulatif

| # | Feature | Refs Go (OpenDiablo2) | Équivalent Rust (engine-rs) | Statut | Notes |
|---|---------|----------------------|------------------------------|--------|-------|
| 1 | MPQ — ouverture archive v0/v1, tables hash+block | `d2fileformats/d2mpq/mpq.go`, `mpq_header.go` | `d2mpq/src/archive.rs::Archive::open` (l.102) | **FULL** | Read-only, tout en mémoire ; wrapper user-data `MPQ\x1b` géré ; hi-offset v3+ enregistré. Validé SHA-256 (`d2mpq/tests/verify_game_files.rs`) |
| 2 | MPQ — crypto (hash 5 types, decrypt/encrypt) | `d2mpq/crypto.go` | `d2mpq/src/crypto.rs` (`hash_string` l.58, `decrypt_bytes` l.84, `encrypt_bytes` l.109) | **FULL** | Clé de hash = hash("(hash table)",3) / "(block table)",3 — identique |
| 3 | MPQ — lecture secteurs, clé fichier (FIX_KEY, key index), single unit | `d2mpq/mpq_stream.go` (l.46-215) | `archive.rs::file_key` (l.265), `read` (l.326) | **FULL** | Sonde linéaire insensible à la casse (`/`==`\`), slots free/deleted gérés (`find` l.219, `has` l.257) |
| 4 | MPQ — décompression PKWARE DCL (implode) | `mpq_stream.go::pkDecompress` (l.309, case 8) | `d2mpq/src/dcl.rs::blast` (l.155) | **FULL** | Port du validé `tools/mpq_mini.py` + `tools/blast_pkware.py`, `evidence/blast.c` |
| 5 | MPQ — décompression zlib | `mpq_stream.go::decompressMulti` case 2 | `archive.rs::zlib_decompress` (l.440, flate2/miniz_oxide) | **FULL** | — |
| 6 | MPQ — décompression bzip2 | `mpq_stream.go` case 0x10 → **erreur "not supported"** | `archive.rs::bz2_decompress` (l.451, bzip2-rs, détection magic `BZh`) | **FULL** | Rust **en avance** sur Go ici (Go n'implémente pas bzip2) |
| 7 | MPQ — huffman / ADPCM wav / lzma / sparse | huffman : `d2data/d2compression/huffman.go` ; ADPCM : `d2data/d2compression/wav.go` (cases 1, 0x40, 0x80, 0x41, 0x81) ; lzma/sparse : erreurs | — (aucun) | **MISSING** | Go a huffman+ADPCM ; lzma/sparse manquent des deux côtés. Hors besoin runtime (les wavs sont extraits offline) |
| 8 | MPQ — masques multi-méthodes (ex. `0x74` des DCC monstres) | switch sur valeur simple (`decompressMulti`) — `0x74` non couvert non plus | `archive.rs::decompress` (l.287) — masque binaire, bits 0x02/0x08, fallback DCL si pas de magic bzip2 | **PARTIAL** | `0x74` (DCC monstres v1.12) non couvert des deux côtés → extraction via StormLib offline (`tools/stormlib_read.py`, GAME-DATA.md §Méthodo) |
| 9 | MPQ — streaming partiel (DataStream seek/read) | `d2mpq/mpq_data_stream.go` (`ReadFileStream`, mpq.go:118) | — (lecture fichier entier en mémoire) | **N-A** | Choix délibéré WASM : les fichiers sont petits, fetch entier + decode in-memory (`d2wasm`) |
| 10 | MPQ — énumération `(listfile)` | `mpq.go::Listfile` (l.146) | — (sondage par hash via `d2tool probe`) | **PARTIAL** | Le `(listfile)` retail est tronqué (102 o, GAME-DATA.md §7) : la sonde par hash couvre mieux le cas réel |
| 11 | DC6 — décodeur complet | `d2dc6/dc6.go`, `dc6_header.go`, `dc6_frame.go`, `dc6_frame_header.go` | `d2dc6/src/lib.rs::Dc6::decode` (l.225) | **FULL** | 2 layouts de pointeurs (flat/classique) auto-détectés + 2 grammaires RLE (variante 1.12 / classique) ; validé digests SHA vs `tools/dc6_mini.py` (`d2dc6/tests/game_files.rs`) |
| 12 | DCC — décodeur complet | `d2dcc/dcc.go`, `dcc_direction.go`, `dcc_direction_frame.go`, `dcc_cell.go`, `dcc_pixel_buffer_entry.go` | `d2dcc/src/dcc.rs`, `direction.rs`, `frame.rs`, `cell.rs`, `pixel_buffer.rs` | **FULL** | Port byte-faithful (signature 0x74, 1/4/32 directions, pixel buffer 4 valeurs/cellule) ; bins `dccdump`, `dcc_render` |
| 13 | DCC/COF — table Dir64→COF (64→1/4/8/16/32 directions) | `d2dcc/dcc_dir_lookup.go` ; `d2cof/cof_dir_lookup.go` | `d2dcc/src/dir_lookup.rs::dir64_to_dcc` (l.42) | **FULL** | Une table mutualisée Rust vs deux quasi-identiques Go |
| 14 | COF — parseur (couches, priority[dir][frame], speed) | `d2cof/cof.go`, `cof_layer.go`, `helpers.go` | `d2dcc/src/cof.rs::Cof::parse` (l.108), `draw_order` (l.194) | **FULL** | Port explicite de cof.go/cof_layer.go ; + `dcc_stems` (l.207) pour générer les chemins DCC par couche |
| 15 | DT1 — décodeur | `d2dt1/dt1.go` (LoadDT1 l.48 : skip 260 octets, v7.6), `tile.go` (Direction, RoofHeight, MaterialFlags), `subtile.go` (BlockWalk/LOS/Jump/PlayerWalk/Light), `gfx_decode.go`, `material.go` | `d2game/src/dt1.rs::decode_dt1` (l.83), `decode_wall_block` (l.65) | **PARTIAL** | Layout **v1.12 réel** (count@268, table@252, pas de signature — dt1.rs:4-17) vs layout classique Go ; sols raw 32×8 + murs line-RLE portés ; **manques** : orientation, roofHeight, MaterialFlags, SubTileFlags décodés, blocs isométriques |
| 16 | DS1 — décodeur | `d2ds1/ds1.go` (loadHeader/loadFileList/loadObjects/loadSubstitutions/loadNPCs/loadNpcPaths), `ds1_version.go` (v3→v18), `layer.go`, `object.go`, `substitution_group.go` | `d2game/src/ds1.rs::decode_ds1` (l.59) | **PARTIAL** | v18 uniquement (TownN1.ds1), taille réelle = stockée+1, 6 couches fixes, file list `.tg1` ; **manques** : autres versions, act, objets, groupes de substitution, chemins NPC (section PATH) |
| 17 | PAL (.dat) — palette 256×BGR | `d2dat/dat.go` (Load), `dat_color.go`, `dat_palette.go` | `d2game/src/pal.rs::MapPalette::load` (l.23, gamma 0.55 map) + `d2dcc/src/render.rs::Palette::load` (l.28, swap BGR) + `d2game/src/sprite.rs::sprite_palette` (l.101) | **FULL** | 767→768 padding géré ; deux chemins couleur documentés (MAP-SPEC.md §7 : pal.dat R/B inversés, PL2 base = RGB direct) |
| 18 | PL2 — palette + transforms (light, blends, hues, text colors) | `d2pl2/pl2.go` (struct complet : 32 light + 16 inv + blends α/add/mult + 111 hues + 13 text colors…), `pl2_palette.go`, `pl2_palette_transform.go`, `pl2_color.go`, `pl2_color_24bits.go` | — en Rust ; **base palette seulement**, extraite offline (`tools/map_render2.py::load_pl2_palette` → `assets/map/pal_act1_pl2.dat`) | **PARTIAL** | Le runtime consomme un .dat 768 o pré-extrait (@0 du PL2) ; aucune transform (éclairage, blend, variantes) |
| 19 | AnimData.d2 (frames/speed/events par token) | `d2animdata/animdata.go` (256 blocs, 67 rec/bloc, 144 events, speed÷256×25fps), `block.go`, `record.go`, `events.go`, `hash.go` | — | **MISSING** | Identifié comme **le** manque bloquant (OPENDIABLO2-NOTES §1.3 et action prioritaire 2) : FramesPerDirection/Speed viennent d'animdata.d2, pas du COF |
| 20 | Font (format glyphe D2) | `d2font/font.go` (signature `Woo!\x01`, 14 o/glyphe), `d2fontglyph/font_glyph.go` | — | **MISSING** | UI MVP = HUD canvas sans fontes D2 |
| 21 | STRING/TBL (dictionnaire de chaînes) | `d2tbl/text_dictionary.go` (hash entries actives/index/hash/positions) | — (noms pré-injectés dans `assets/manifest.json`) | **MISSING** | Les libellés fr-FR (ex. « Camp des Rousses ») sont figés dans le manifest |
| 22 | TXT excel (TSV) — lecteur générique | `d2txt/data_dictionary.go` (csv tabulé + lookup colonnes), `doc.go` | — en Rust ; valeurs extractées offline vers `assets/manifest.json` + `d2missiles/data/missiles_real.json` (`tools/build_manifest.py`) ; TXT bruts conservés dans `assets/` | **PARTIAL** | Résultat équivalent pour les ~20 tables extraites, mais pas de lecteur générique runtime |
| 23 | Loader — multi-source (MPQ prioritaire + FS), cache, tokens langue | `d2loader/loader.go` (providers l.28-29, cache 512 Mo), `d2loader/mpq/{source,asset}.go`, `d2loader/filesystem/*`, `asset/asset.go` | — (JS fetch des assets extraits + `assets/manifest.json` ; accès MPQ direct : `d2tool`, bindings `d2wasm/src/lib.rs`) | **PARTIAL** | Architecture différente délibérée (pipeline d'extraction offline, OPENDIABLO2-NOTES §4.3) ; priorité patch_d2 > exp > data/char gérée au niveau de l'extraction, pas runtime |
| 24 | Loader — typage d'asset par extension + tokens langue | `d2loader/asset/types/asset_types.go`, `source_types.go` (Ext2AssetType, language/charset) | — (chemins figés dans le manifest) | **N-A** | Pas de résolution dynamique de type à faire |
| 25 | d2records — RecordManager (framework : binding loaders par chemin, Load, structs typés) | `d2records/record_manager.go` (init l.176-266 : **76 loaders**), `record_loader.go` | — (aucun framework ; `d2rules/src/data.rs` lit `assets/manifest.json`, `d2missiles/src/table.rs` lit un JSON embarqué) | **MISSING** | Remplacé par pipeline manifest JSON ; voir familles ci-dessous pour les données |
| 26 | Records — niveaux (LvlTypes, LvlPrest, LvlWarp, Levels, LvlDetails, LvlMaze, LvlSub, AutoMap) | `level_types_loader.go`, `level_presets_loader.go`, `level_warp_loader.go`, `level_details_loader.go`, `level_maze_loader.go`, `level_substitutions_loader.go`, `automap_loader.go` | `assets/LvlTypes.txt`, `assets/LvlPrest.txt`, `assets/levels_exp.txt` extraits ; runtime : manifest `level` (ds1/dt1/palette/sizeTiles/levelsId) | **PARTIAL** | 1 région (Camp des Rousses) ; pas de warps, maze, substitutions, automap |
| 27 | Records — objets décor (ObjTypes, ObjDetails, ObjMode, ObjGroup + objectLookups codés en dur) | `object_types_loader.go`, `object_details_loader.go`, `object_mode_loader.go`, `object_groups_loader.go`, `object_lookup_record.go`, `object_lookup_record_data.go` (`initObjectRecords`) | — | **MISSING** | Aucun objet décor (coffres, portals décoratifs…) |
| 28 | Records — items (Weapons, Armor, Misc, Books, Belts, Colors, LowQuality, ItemTypes, Unique, Sets, SetItems, Gems, Runes, ItemRatio, ItemStatCost, AutoMagic, MagicPrefix/Suffix, RarePrefix/Suffix, QualityItems, StorePage, Gamble, Cube×3) | 23 loaders `item_*_loader.go`, `armor_type_loader.go`, `books_loader.go`, `belts_loader.go`, `color_loader.go`, `gems_loader.go`, `runeword_loader.go`, `set_loader.go`, `set_item_loader.go`, `cubemain_loader.go`, `cube_modifier_loader.go`, `cube_type_loader.go`, `item_affix_loader.go`, `rare_*_loader.go`, etc. | — en runtime ; **constantes MVP** : hax/buc/hp1 (`d2rules/src/data.rs::HeroTpl`), hp1/mp1/gld (world.rs) ; tables extraites pour référence : `assets/patch_weapons/armor/misc/gems/runes/setitems/uniqueitems/cubemain.txt` | **PARTIAL** | ~6 items codés en dur vs 23 parseurs Go ; pas d'affixes, pas de cube, pas d'uniques |
| 29 | Records — monstres (MonStats, MonStats2, MonPreset, MonProp, MonType, MonMode, SuperUniques, UniqueModifiers, Equipment, UniqueAppellation, MonLvl, MonSound→35, MonSequence, MonPlacement, MonAI, UniquePrefix/Suffix, NPC) | 17 loaders `monster_*_loader.go`, `monsters_loader.go`, `npc_loader.go`, `monster_levels_loader.go` | `d2rules/src/data.rs::MonTpl` (6 monstres via manifest) + facteurs MonLvl niv 1 codés en dur (HP×7/100, DM×2/100, XP×30/100, AC 6, TH 8 — world.rs/lib.rs) ; `assets/patch_monstats.txt`, `assets/MonLvl.txt`, `assets/monsounds.txt` extraits | **PARTIAL** | Colonnes portées : Level/minHP/maxHP/A1MinD/A1MaxD/Exp/Velocity ; pas de monstats2/équipement/IA-records/super-uniques |
| 30 | Records — skills (Skills, SkillCalc, SkillDesc, MissileCalc) | `skill_details_loader.go`, `calculations_loader.go`, `skill_description_loader.go`, `missile_calc_loader.go` | dégâts/éléments des skills porteurs codés dans la table missiles (`d2missiles/src/table.rs`, doc l.8-15) ; `assets/patch_skills.txt` extrait | **PARTIAL** | ~6 valeurs canoniques (firebolt, icebolt…) vs parseur complet des 4 tables |
| 31 | Records — missiles (missiles.txt) | `missiles_loader.go`, `missiles_record.go` (mouvement/collision/dégâts paliers/sous-missiles/HitShift) | `d2missiles/src/table.rs::parse_table_err` (l.23) — **10 définitions** extraites de `patch_missiles.txt` v1.12 réelle (684 lignes) : Id/Vel/Range/HitClass/EType/CelFile/AnimLen | **PARTIAL** | Rapport dédié : `projet/rapport_d2missiles` ; pas de HitShift, Calc, sous-missiles (moteur d2missiles couvre le vol droit MVP) |
| 32 | Records — charstats & classes (CharStats, PlrClass, PlrMode, PetType, BodyLocations, Composite, ArmorType→tokens, WeaponClass, HitClass, CompCode) | `charstats_loader.go`, `player_class_loader.go`, `player_mode_loader.go`, `pet_type_loader.go`, `body_locations_loader.go`, `composite_type_loader.go`, `armor_type_loader.go`, `weapon_class_loader.go`, `hit_class_loader.go`, `component_codes_loader.go` | `d2rules/src/data.rs::parse_hero_code` (l.91, BA/SOR/PA via manifest) ; tokens composites = enum `CompositeType` (`d2dcc/src/cof.rs`, 15 tokens HD…S8) | **PARTIAL** | 3 héros vs 7 ; tokens composite dans le COF (équivalent armor_type/composite records) |
| 33 | Records — expérience | `experience_loader.go` | `d2rules/src/world.rs::XP_TABLE` (l.59) — 10 niveaux codés en dur depuis `patch_experience.txt` col. Barbarian | **PARTIAL** | 10/99 niveaux ; MaxLvl/ExpRatio non portés |
| 34 | Records — TreasureClass(Ex) | `treasure_class_loader.go`, `treasure_class_ex_loader.go` | table réelle extraite (`assets/TreasureClassEx.txt`) ; **drop runtime = constantes MVP** 60 % rien / 25 % or / 15 % potion (`d2rules/src/world.rs` l.80-84, `roll_drop` l.1115) | **PARTIAL** | Les données sont là, la logique n'est pas branchée (prévu phase 3, world.rs:80) |
| 35 | Records — sons (Sounds, SoundEnvirons) | `sound_details_loader.go`, `sound_environment_loader.go` | mapping sfx dans `assets/manifest.json` (13 wavs : die→fallen_die.wav, uiClick…) | **PARTIAL** | Pas de sounds.txt, pas d'ambiances par région |
| 36 | Records — divers (DifficultyLevels, States, Overlays, Shrines, ElemType, Events, Inventory) | `difficultylevels_loader.go`, `states_loader.go`, `overlays_loader.go`, `shrine_loader.go`, `elemtype_loader.go`, `events_loader.go`, `inventory_loader.go` | — | **MISSING** | Aucune portée ; hors besoins du MVP (une difficulté, pas d'états/overlays/shrines) |

---

## 2. Sections détaillées par format

### 2.1 MPQ (`d2common/d2fileformats/d2mpq` + `d2common/d2loader`)

**Go.** `mpq.go` : `New`/`FromFile` (ouverture fichier, ignore-case du chemin d'archive l.182), `ReadFile` (l.96, `strings.ToLower(fileName)` l.102 — la hash table MPQ est insensible à la casse), `ReadFileStream` (l.118), `ReadTextFile` (l.135), `Listfile` (l.146), `Contains` (l.172). `mpq_header.go` : header 0x20. `mpq_hash.go`/`mpq_block.go`/`mpq_file_record.go` : entrées hash `<IIHHI` + blocks `<IIII`. `crypto.go` : table de crypte, `hash_string(s, type)` 0..2 + 3 = table key, decrypt. `mpq_stream.go` : secteurs, table d'offsets déchiffrée (`key-1`), `FIX_KEY`, single unit, `decompressMulti` (l.227) — **switch sur valeur** : zlib (2) ✅, pkware (8) ✅ via `pkDecompress` (l.309), wav ADPCM (0x40/0x80) ✅ + huffman (huffman.go) ✅, bzip2 (0x10) ❌, lzma (0x12) ❌, sparse (0x22/0x30) ❌. `mpq_data_stream.go` : wrapper io.ReadSeeker.

**Rust.** `d2mpq/src/archive.rs` : `Archive::open` (l.102) — header `MPQ\x1a` + wrapper user-data `MPQ\x1b` (offset réel à +8), v0/v1 (hi-offset v3+ enregistré, l.135), tables déchiffrées (`load_tables` l.162), `find` (l.219) sonde linéaire insensible à la casse avec terminaison sur slot free (locale=platform=0xFFFF) et skip des deleted (0xFFFFFFFF), `file_key` (l.265 — FIX_KEY `(key+offset)^fsize` + key-index), `read` (l.326) — single unit (l.333) + secteurs, `decompress` (l.287) — **masque binaire** IMPLODE=DCL pur, COMPRESS=mask byte avec bits 0x02 zlib / 0x08 bzip2-si-magic-`BZh`-sinon-DCL. `crypto.rs` : `hash_string`, `decrypt_bytes`, `encrypt_bytes`. `dcl.rs::blast` (l.155) : explode PKWARE complet. `d2wasm/src/lib.rs` : bindings wasm-bindgen (probe/lecture depuis le navigateur). `d2tool` : CLI `probe <mpq> <chemin>` (exit 0 = hit).

**Preuves.** `d2mpq/tests/verify_game_files.rs` : identité octet-pour-octet (SHA-256) contre les assets extraits par le pipeline Python validé `tools/mpq_mini.py`. `docs/GAME-DATA.md` §7 : 21 sprites/COF SHA-256 identiques à l'archive.

**Écarts.**
- Rust lit tout en mémoire (WASM), Go streame par secteurs — N-A pour ce port (voir N-A #9).
- Rust couvre bzip2 (Go non) ; Rust/Go couvrent tous deux DCL+zlib ; huffman/ADPCM absents du port (Go les a) — #7 MISSING.
- Masque composé `0x74` des DCC monstres v1.12 non décodable des deux côtés → `tools/stormlib_read.py` (StormLib) pour l'extraction offline — #8 PARTIAL.
- Pas d'énumération `(listfile)` : sondage par hash (`d2tool probe`), ce qui est la seule voie viable (listfile retail tronqué à 102 o, GAME-DATA.md §7) — #10 PARTIAL.
- Priorité multi-archives (patch_d2 > d2exp > d2data/d2char) gérée par l'ordre de sondage/extraction offline, pas par un chargeur runtime comme `d2loader` Go — #23 PARTIAL.

### 2.2 DC6 (`d2dc6`)

**Go.** `dc6.go` (Load + Decode24), `dc6_header.go` (version, flags, encoding, dirs, frames/dir), `dc6_frame.go`, `dc6_frame_header.go` (flip/w/h/offsets/allocSize/nextBlock/dataLen), RLE via `d2datautils` stream reader.

**Rust.** `d2dc6/src/lib.rs` : `Dc6::decode` (l.225) ; `frame_pointers` (l.87) auto-détecte **layout A flat** (pointeurs de frames directs @24 — tous les fichiers 1.12 digital) et **layout B classique** (indirection directions) ; `valid_frame_header` (l.69) plausibilise (w/h ≤ 2048, flip ≤ 1) ; `rle_rows` (l.138) implémente les **deux grammaires** : variante 1.12 (`encoding==0` mais data RLE, polarité inversée : `≤0x7F` littéral, `0x80` EOL, `0x81 n` littéral n, `≥0x82` transparence) et classique (`encoding==1`, avec rewind `0x80 n`) ; scanlines bottom-up, runs bornés à la ligne, padding 4 octets jamais décodé (stop après `dataLen`/`height`) ; `indices_to_rgba` (l.255) — index 0 transparent, palette courte padée.

**Preuves.** `evidence/DC6-SPEC.md` (layout 1.12 + pièges) ; `d2dc6/tests/game_files.rs` — digests SHA-256 des frames de `assets/skillicon.dc6` générés par `tools/dc6_mini.py` le 2026-10-22 ; bindings navigateur `d2wasm/src/lib.rs:52`.

**Écart.** Go n'a pas le cas « layout flat » explicite (il suit les pointeurs du header) — le port Rust est **plus tolérant** et validé sur les deux.

### 2.3 DCC + COF (`d2dcc` + `d2cof`)

**Go.** `dcc.go` (signature 0x74, version, 1/4/32 directions, offsets ×8 en bits), `dcc_direction.go` (parse bit-level : boxes, pixelMask/rawPixelCodes/pixelCode streams 20 bits, cellules 4×4), `dcc_direction_frame.go`, `dcc_cell.go`, `dcc_pixel_buffer_entry.go` (4 valeurs/cellule partagées inter-frames), `dcc_dir_lookup.go` (Dir64ToDcc). `d2cof/cof.go` (header 28 o, couches 9 o, priority[dir][frame]), `cof_layer.go`, `cof_dir_lookup.go`, `helpers.go`.

**Rust.** `d2dcc/src/dcc.rs` (l.27), `direction.rs::parse` (l.79) + `fill_pixel_buffer` (l.249) avec `CRAZY_BIT_TABLE`/`PIXEL_MASK_LOOKUP` identiques, `frame.rs` (recalculate_cells), `cell.rs`, `pixel_buffer.rs` (PixelBufferEntry = port direct), `bitmuncher.rs` (port de `d2datautils/bitmuncher.go`), `dir_lookup.rs::dir64_to_dcc` (l.42), `cof.rs::Cof::parse` (l.108, 25+3 o header, couches 9 o : type/shadow/selectable/transparent/drawEffect/weaponClass 4 o, `animation_frames`, `priority[direction][frame]`), `draw_order` (l.194), + `dcc_stems` (l.207) qui génère les noms de DCC par couche (tokens `LIT` découverts par hash-probing, `evidence/DCC-SPEC.md`). Composition runtime : `d2game/src/sprite.rs::SpriteSet::compose` (l.57) — union des boxes, composite dans l'ordre COF, palette indices→RGBA (`d2dcc/src/render.rs::frames_union/composite_indices/indices_to_rgba`).

**Preuves.** `evidence/DCC-SPEC.md` (§2 piège BGR, §chemins chars/monstres) ; bins `dccdump`/`dcc_render` (mode compose = origine de sprite.rs) ; le composite `assets/dcc/composite.png` validé.

**Écart.** Aucun fonctionnel : le port déclare être un port byte-faithful de cof.go/dcc.go (lib.rs:4). La partie intégration COF↔animdata.d2 (frames/speed) est absente — voir #19.

### 2.4 DT1 (`d2dt1` vs `d2game/src/dt1.rs`)

**Go.** `dt1.go::LoadDT1` : v7.6 exigé, skip 260 octets inconnus, `numberOfTiles` @260, `bodyPosition` @264, tuiles lues à `bodyPosition` (header 96 o : Direction, RoofHeight, MaterialFlags, flags variés, 6×SubTileFlags, blocks 20 o : x, y, 2×unknown, horizontal/vertical, unknowns, len, offset). `tile.go` : Direction (0 floor / 1-4 walls / others roofs), RoofHeight, MaterialFlags. `subtile.go` : BlockWalk/BlockLOS/BlockJump/BlockPlayerWalk/Unknown1/BlockLight/Unknown2/Unknown3 + `Combine`. `gfx_decode.go` : decode block selon orientation (isométrique line-RLE vs raw). `material.go` : bits matériau.

**Rust.** `d2game/src/dt1.rs` : layout **v1.12 réel validé** (doc l.4-17, port de `tools/map_proto2.py` qui a produit le rendu validé) : pas de signature `C7 AB 0B 00`, `(7,6)` @0, `numberOfTiles` **@268**, table de tuiles **@252** stride 96 ; `nsub==25` ⇒ floor (raw 32×8), sinon wall (line-RLE `decode_wall_block` l.65 avec la sécurité `t&0x1F`) ; blocs : a/b/flags/len/off ; erreurs propres (`TooShort/BadVersion/Truncated`).

**Preuves.** `evidence/MAP-SPEC.md` (layout + piège « murs ori-0 = sols »), `assets/map/level-render.png` validé, tests unitaires wall-RLE (dt1.rs:125).

**Écarts (→ PARTIAL).**
- Pas d'orientation/roofHeight/MaterialFlags décodés ; `flags` de sous-blocs lus bruts et déclarés **non fiables** dans cette release (policy collision = kind, `compose.rs` doc).
- Pas de SubTileFlags par sous-tile (BlockWalk/LOS…) : Go les structure, Rust les ignore — la collision subtile précise (OPENDIABLO2-NOTES §2.2) n'est pas possible avec le port actuel.
- Pas de décodage des blocs isométriques encodés (Go `gfx_decode.go`) ni du champ vertical ; les 8 DT1 Act-1-Town testés n'en ont pas besoin.
- Écart de layout : Go suppose le header classique public (count @260) ; le port Rust suit le layout réel v1.12 (count @268, table @252) — les fichiers de cette release ne passeraient probablement pas le parseur Go sans adaptation.

### 2.5 DS1 (`d2ds1` vs `d2game/src/ds1.rs`)

**Go.** `ds1.go` : `Unmarshal` → loadHeader (versions v3→v18 via `ds1_version.go` : unknown1/2 bytes, act, substitution type, walls/floors counts, file list, objects), `loadFileList`, schéma de couches par version (`getLayerSchema` l.342 — wall/floor/orientation/substitution/substitutionTile/shadow), `loadObjects` (l.230 : type/id/x/y/flags + paths NPC `loadNPCs` l.393), `loadSubstitutions` (l.283). `layer.go`, `object.go`, `substitution_group.go`, `tile.go`.

**Rust.** `d2game/src/ds1.rs::decode_ds1` (l.59) : **v18 uniquement** (TownN1.ds1), stored size+1 (piège v18), `fileCount` @20 + chemins `.tg1` NUL-terminés, meta (2,1) sautée, **6 couches fixes** `w*h` int32 (le schéma réel de ce fichier), unpack cellule `cell_fields` (p1/p2/p3/orient, bit 0x80 = caché), `cell_tile_index` = p1 + (p2&0x0F). Section PATH documentée mais non parsée (doc l.7).

**Preuves.** `evidence/MAP-SPEC.md`, `assets/map/TownN1.ds1` + les 8 DT1 correspondants un à un (GAME-DATA.md §6), tests cell-packing (ds1.rs:97).

**Écarts (→ PARTIAL).** autres versions (v3-v16), act, nombre variable de couches, objets ( placements monstres/objets !), groupes de substitution, chemins NPC. Bloquant pour les régions préfabriquées (stamps) et les warps (OPENDIABLO2-NOTES §2.3).

### 2.6 PAL / DAT + PL2 (`d2dat`, `d2pl2` vs `d2game/src/pal.rs`, `d2dcc/src/render.rs`)

**Go.** `d2dat/dat.go::Load` : 256 triples BGR→`DATColor{b,g,r}` ; `dat_palette.go` : implémente `d2interface.Palette`. `d2pl2/pl2.go` : struct complet — base palette, 32 light-level transforms, 16 inv-color, selected-unit shift, α/add/mult blends (256×), 111 hue variations, R/G/B tones, 13 text colors 24 bits + shifts, etc. (unpack `restruct`).

**Rust.** Deux chemins documentés (`pal.rs` doc) : **map** = `pal_act1.dat` 767 o padé + gamma 0.55 (`MapPalette::load` l.23, choix lisibilité validé du prototype) ; **sprites** = base PL2 pré-extrait `pal_act1_pl2.dat` (RGB direct, **sans** swap depuis l'intégration PL2 — l'ancien pal.dat avait R/B inversés, « blue barbarian bug », sprite.rs:94-100) via `render.rs::Palette::load(raw, bgr=false)` et `sprite_palette` (l.101).

**Preuves.** `evidence/MAP-SPEC.md` §7 (pal.dat R/B inversés), `tools/map_render2.py::load_pl2_palette` (extraction @0 du PL2 → 768 o), tests pal.rs.

**Écarts (→ PARTIAL pour PL2).** Aucune transform PL2 côté Rust (éclairage par niveau de lumière, invcolor sélection, blends α/add/mult, teintes, couleurs de texte UI, palshift.dat pour les variantes de monstres — Go non plus pour palshift, cf. OPENDIABLO2-NOTES §4.4). Le .dat de palette simple est équivalent (FULL #17).

### 2.7 AnimData.d2 (`d2animdata`) — **MISSING**

**Go.** `animdata.go` : 256 blocs × ≤67 records, hash interne, `GetRecord`/`GetRecordNames` (dernier record homonyme gagnant), `record.go` (frames/direction, speed ÷256 en 25ᵉ de fps), `events.go` (frames trigger SFX : 144 events), `block.go`, `hash.go`.

**Rust.** Rien. Les frames/speed viennent du COF (`Cof::frames_per_direction/speed`) et des constantes du manifest — OPENDIABLO2-NOTES §1.3 rappelle que **le COF ne donne pas le nombre de frames** : c'est le gap n°1 déjà identifié (action prioritaire 2 des notes).

### 2.8 Font (`d2font`) — **MISSING**

**Go.** `font.go` : signature `Woo!\x01`, header 12 o, glyphes 14 o (width/unknowns), table d'index unicode, feuille = animation DC6 ; `d2fontglyph/font_glyph.go` (width, frame, indices).

**Rust.** Rien ; le HUD d2game dessine texte/formes canvas sans fontes D2.

### 2.9 STRING/TBL (`d2tbl`) — **MISSING**

**Go.** `text_dictionary.go` : entries hash `<active u8, index u16, hash u32, index-string u32, name-string u32>`, dictionnaire clé→valeur.

**Rust.** Rien. Les libellés nécessaires (nom de niveau, items) sont figés dans `assets/manifest.json` (« Camp des Rousses »). Ok pour le MVP mono-région, bloquant pour tout texte dynamique fr-FR.

### 2.10 TXT excel (`d2txt`) — **PARTIAL (pipeline offline)**

**Go.** `data_dictionary.go` : csv tabulé, lookup colonnes, accès typé (string/number/bool/ints…) — la base de **tous** les records.

**Rust.** Aucun parseur runtime. Les valeurs gameplay sont extraite offline et figées dans `assets/manifest.json` (JSON strict : `hero`/`heroes{BA,SOR,PA}`/`monsters[FA,ZM,SI,SK,FS,YE]`/`level`/`sfx`/`levels`) et `d2missiles/data/missiles_real.json` (include_str!), chaque valeur étant tracée colonne par colonne dans `docs/GAME-DATA.md`. Les TXT originaux restent dans `assets/` comme référence (~20 tables). `tools/build_manifest.py` lit les COF locaux et injecte modes/couches ; `tools/probe_game_data.py` sonde/extrait.

**Conséquence.** Pas de charge dynamique de nouvelles tables sans régénérer le manifest ; en contrepartie zéro parseur TXT en WASM (poids/exécution), et chaque valeur consommée est vérifiable (GAME-DATA.md).

### 2.11 d2records (les 76 loaders Go) — familles

Le `RecordManager` Go (`record_manager.go::init` l.176-266) relie 76 fichiers txt à des loaders typés + `initObjectRecords` (lookups d'objets codés en dur dans `object_lookup_record_data.go`). Côté Rust, il n'y a **pas de framework équivalent** ; l'état par famille est dans le tableau (#26-#36). Points marquants :

- **Items** : 23 loaders Go vs ~6 items codés en dur (hax, buc, hp1, mp1, gld, tsc/isc cités). Les tables réelles sont extraites mais non consommées.
- **Monstres** : 6 monstres via manifest (Level/minHP/maxHP/A1MinD/A1MaxD/Exp/Velocity) + scaling MonLvl niv-1 codé en dur (HP×7/100, DM×2/100, XP×30/100, AC 6, TH 8) — les tables `patch_monstats.txt`/`MonLvl.txt`/`monsounds.txt` sont extraites.
- **Missiles** : 10/684 définitions réelles (arrow, javelin, throwaxe, firearrow, icearrow, magicarrow, firebolt, icebolt, chargedbolt…) — voir `projet/rapport_d2missiles`.
- **TreasureClassEx** : table réelle extraite mais le drop runtime reste le tirage MVP 60/25/15 (`world.rs::roll_drop`) — branchement prévu phase 3.
- **Skills/XP** : valeurs canoniques codées en dur (XP_TABLE 10 niveaux, dégâts skills porteurs) au lieu des parseurs.
- **Objets décor** et **divers** (difficulty/states/overlays/shrines/elemtype/events/inventory) : rien.

### 2.12 d2loader — architecture de chargement

**Go.** `loader.go` : providers MPQ + filesystem, sources ordonnées (priorité patch), cache LRU 512 Mo, résolution tokens langue (`languageFontToken`/`tableToken`), `Load(subPath) -> io.ReadSeeker` ; `asset/types` : typage par extension (json/tbl/txt/dat/pl2/cof/dc6/dcc/ds1/dt1/wav/d2).

**Rust.** Pas d'abstraction runtime équivalente : le navigateur fetch les fichiers listés par `assets/manifest.json` et les passe à `GameCore::provide` (`d2game/src/game.rs` doc) ; `d2tool`/`d2wasm` accèdent aux MPQ directement pour le sondage/extraction. C'est la leçon OPENDIABLO2-NOTES §4.3 (extraire/décoder offline plutôt qu'au runtime) — écart **délibéré et documenté**, pas un manque fonctionnel pour le MVP, mais les priorités multi-MPQ et le cache restent à gérer si le port revient à une lecture archive directe.

---

## 3. Liste priorisée des manques

1. **AnimData.d2** (#19) — *bloquant animation exacte* : FramesPerDirection/Speed/events par token `TOKEN+MODE+WC`. Le COF ne suffit pas (OPENDIABLO2-NOTES §1.3, action 2). Port recommandé : `d2animdata` → crate `d2animdata` ou module `d2dcc::animdata`.
2. **Parseur TXT générique + mini RecordManager** (#22, #25) — *débloque tout le data-driven* : remplacer les constantes codées en dur (XP, drops, items, skills, MonLvl) par des records réels, sans JSON intermédiaire à régénérer.
3. **Consommation TreasureClassEx** (#34) — les données sont déjà extraites ; brancher `roll_drop` sur la chaîne réelle (NoDrop/Picks, `Act 1 H2H A`…).
4. **DT1 complet** (#15) — SubTileFlags (BlockWalk/BlockLOS) par sous-tile pour collision/LOS réelles, orientation/material pour les prochaines régions (Blood Moor, `evidence/MAP-SPEC-BLOODMOOR.md`), blocs isométriques encodés.
5. **DS1 multi-versions + objets/substitutions/PATH** (#16) — prérequis stamps, warps et placements d'entités des DS1 (OPENDIABLO2-NOTES §2.3).
6. **PL2 transforms + palshift.dat** (#18) — niveaux de lumière, blends, variantes de couleur champions/uniques (opportunité signalée : personne ne l'a, notes §4.4).
7. **TBL + Font** (#21, #20) — texte fr-FR dynamique et HUD avec fontes D2 (nécessaire dès qu'on sort du MVP mono-écran).
8. **MPQ : huffman/ADPCM + masques composés (0x74)** (#7, #8) — pour lire les DCC monstres directement en runtime sans StormLib offline.
9. **Records objets décor** (#27) — objectLookups/objtypes/objdetails pour le décor interactif.
10. **Records divers** (#36) — difficultés, states, overlays, shrines : post-MVP.

---

## 4. Comptes finaux

| Statut | Nombre | Features |
|--------|-------:|----------|
| **FULL** | **11** | MPQ ouverture/tables · MPQ crypto · MPQ secteurs+clés · MPQ DCL · MPQ zlib · MPQ bzip2 (en avance sur Go) · DC6 · DCC · Dir64 lookup · COF · PAL/DAT |
| **PARTIAL** | **16** | MPQ listfile · MPQ masques multi-méthodes · DT1 · DS1 · PL2 · TXT excel · Loader multi-source · records niveaux · records items · records monstres · records skills · records missiles · records charstats/classes · records expérience · records TreasureClassEx · records sons |
| **MISSING** | **7** | MPQ huffman/ADPCM/lzma/sparse · AnimData.d2 · Font · STRING/TBL · RecordManager (framework) · records objets décor · records divers (difficulty/states/overlays/shrines/elemtype/events/inventory) |
| **N-A** | **2** | MPQ streaming DataStream (WASM in-memory par design) · typage d'assets/tokens langue (manifest-driven) |
| **Total** | **36** | |
