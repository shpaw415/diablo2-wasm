# Audit de parité OpenDiablo2 → diablo-2-wasm — Domaine GAMEPLAY + RÉSEAU + APP

> Audit du 2026-10-22. Référence : repo Go **OpenDiablo2** (cloné dans `/home/agent/workspace/opendiablo2`,
> réimplémentation Go de Diablo 2, archivée par ses auteurs). Port audité :
> `/home/agent/workspace/diablo-2-wasm/engine-rs` (crates `d2missiles`, `d2rules`, `d2game`)
> et `/home/agent/workspace/diablo-2-wasm/demo/`.
>
> **Statuts** : FULL = équivalent fonctionnel livré (au périmètre MVP port navigateur) ·
> PARTIAL = un sous-ensemble livré, des écarts documentés · MISSING = aucun équivalent ·
> N-A = hors scope du port (navigateur WASM centré données, mono-joueur) — justification fournie.
>
> Audit uniquement : aucune ligne de code portée. Les chemins Go sont relatifs à `opendiablo2/`,
> les chemins Rust à `diablo-2-wasm/`.

---

## 1. Tableau récapitulatif

| # | Feature | Refs Go | Équivalent Rust | Statut | Notes |
|---|---------|---------|-----------------|--------|-------|
| A1 | État de base héros (niveau, XP, vie/mana, force/dex, or) | `d2core/d2hero/hero_stats_state.go` | `engine-rs/crates/d2rules/src/lib.rs:205` (`HeroStats`), `world.rs:347` `init_hero` | PARTIAL | Pas de vit/energy/stamina ni points à répartir |
| A2 | Attributs D2 complets (vit, ene, stamina, statsPoints/skillPoints) | `hero_stats_state.go:9-30` | — | MISSING | `HeroStats` couvre str/dex seulement |
| A3 | Stats initiales par classe depuis charstats.txt | `hero_stats_state.go:33-56` (`CreateHeroStatsState`) | `engine-rs/crates/d2rules/src/data.rs:91` (`parse_hero_code`) | PARTIAL | Valeurs réelles (55 pv, str 30, dex 20…) mais figées dans le manifest, pas les tables txt ; formules `LifePerVit×vit` non portées |
| A4 | Progression par niveau (NextLevelExp, +vie/+mana/niveau) | `hero_stats_state.go:37`, `d2core/d2records/experience_record.go` | `world.rs:59` (`XP_TABLE`), `world.rs:926` `apply_xp` | PARTIAL | Seuils XP réels (col. Barbarian, `assets/patch_experience.txt`) mais plafonnés niv 10 (`MAX_LEVEL` `world.rs:55`) |
| A5 | HeroState persistable (position, or, act, difficulté) + save/load | `d2core/d2hero/hero_state.go`, `hero_state_factory.go:243` (`Save`) | `engine-rs/crates/d2rules/src/save.rs` (JSON v1), `demo/game.js:555` (localStorage) | PARTIAL | Save bit-exact complète du monde (RNG inclus) mais 1 seul slot, pas d'act/difficulté/left-right skill/nom |
| A6 | Compétences (état, points, skills de base par classe) | `d2core/d2hero/hero_skill.go`, `hero_state_factory.go:111-148` | — | MISSING | Aucun skill : attaque mêlée fixe + tir `arrow` |
| A7 | Multi-personnages (liste saves, création, suppression) | `hero_state_factory.go:73-109` (`GetAllHeroStates`) | `demo/menu.js:36` (« Continuer » unique) | MISSING | Un seul slot de sauvegarde localStorage |
| B1 | Système générique Stat/StatValue/StatList | `d2core/d2stats/diablo2stats/stat.go`, `statlist.go`, `stat_value.go` | — (structs fixes `HeroStats`/`ItemDef`) | MISSING | Le port code les stats en dur au lieu d'un système à clés |
| B2 | Descriptions de stats (itemstatcost DescFunc, stringers) | `diablo2stats/stat.go:348+`, `stat_factory.go:81-130` | — | MISSING | — |
| B3 | Agrégation de stat lists (reduce/append équipement) | `diablo2stats/statlist.go:71-116` | — | MISSING | — |
| C1 | Types d'objets armor/weapon/misc (code, nom, taille grille) | `d2core/d2inventory/inventory_item_{armor,weapon,misc}.go` | `d2rules/src/lib.rs:169` (`ItemDef{code,name,kind,amount}`) | PARTIAL | 4 kinds, 1 case par objet, pas de tailles X×Y ni weapon class |
| C2 | Équipement 8 slots + items par défaut par classe | `d2inventory/character_equipment.go`, `inventory_item_factory.go:35-70` | — (stats `startWeapon`/`startShield` brutes, `data.rs:110-113`) | MISSING | Les dégâts/AC de départ sont intégrés aux stats, pas portés comme objets |
| C3 | Grille d'inventaire multi-cases (canFit, Set/Remove) | `d2game/d2player/inventory_grid.go` | `world.rs:88-90` (10×4 plat), `d2game/game.rs:661` | PARTIAL | 40 slots 1 case, empilement d'or géré, pas de fit X×Y |
| D1 | Génération d'items depuis TreasureClassEx | `d2core/d2item/diablo2item/item_factory.go:163-330` | `world.rs:1116` (`roll_drop` MVP 60/25/15) | MISSING | Table de drop MVP 3 issues ; le code documente « TreasureClassEx réel en phase 3 » |
| D2 | Qualités d'items (low/normal/magic/set/rare/unique/crafted) | `item_factory.go:24` (`dropModifier`), `item.go:312-330` | — | MISSING | — |
| D3 | Affixes magiques (filtre ilvl, poids, exclusions) | `item_factory.go:324` (`FindMatchingAffixes`), `item.go:331-386` | — | MISSING | Tables `item_affix_*` Go non portées |
| D4 | Propriétés d'items → stats (Property.eval, fn*) | `item_property.go:118+` | — | MISSING | — |
| D5 | Nom généré, label, description, identification | `item.go:127,610,802,830` | — | MISSING | Items MVP : nom statique de la table |
| E1 | Parseur de formules CalcString (lexer/parser/eval) | `d2common/d2calculation/{calcstring,calculation}.go`, `d2lexer/`, `d2parser/` | — (`world.rs:190-194` formules codées en dur) | MISSING | 2 formules D2 implémentées à la main (toucher `200×TH/(TH+AC)` [5,95], dégâts ×(1+str/100)) |
| F1 | Enums Hero + tokens | `d2common/d2enum/hero.go`, `hero_string.go` | `data.rs:76` (`resolve_hero`), codes manifest `BA/SOR/PA` | PARTIAL | 3 classes jouables vs 7 ; tokens 2/3 lettres documentés (docs/OPENDIABLO2-NOTES.md §1.2) |
| F2 | Modes d'animation joueur/monstre | `d2enum/player_animation_mode.go`, `monster_animation_mode.go` | `d2rules/src/lib.rs:125` (`AnimState{NU,WL,RN,A1,DD}`) | PARTIAL | 5 modes sur 16 ; GH/BL/SC/TH/S1-4/DD-monstre absents |
| F3 | Enums gameplay restantes (difficulté, quêtes, relations, régions…) | `d2enum/difficulty.go`, `quests.go`, `players_relationships.go`, `region_id.go`… | — | MISSING | 1 seul acte, 1 seule difficulté, pas de quêtes |
| G1 | Table missiles.txt (parsing 100+ colonnes) | `d2core/d2records/missiles_record.go`, `missiles_loader.go` | `engine-rs/crates/d2missiles/src/table.rs` (10 déf, colonnes Missile/Id/Vel/Range/HitClass/EType/CelFile/AnimLen) | PARTIAL | Table RÉELLE v1.12 embarquée (684 lignes sources), 10 missiles MVP sur ~400 |
| G2 | Entité missile en vol (vélocité, portée) | `d2core/d2map/d2mapentity/missile.go` | `d2missiles/src/lib.rs:122` (`Missile`), `step_one` sous-pas anti-tunneling | PARTIAL | Ligne droite à vitesse/portée réelles ; pas des fonctions de mouvement ServerMovementFunc (spirales, forks…) |
| G3 | Collision missile (types 0/1/3/6/8, friendly fire, timer) | `missiles_record.go:48-62` (`MissileCollision`) | `lib.rs:321` (`tick_walls`), `world.rs:864` (rayon 0,7) | PARTIAL | Collision unités+murs couverte (≈type 3) ; friendly fire, timers, types murs-seuls absents |
| G4 | Dégâts missiles (SrcDmg, paliers niveau, synergies, HitShift) | `missiles_record.go:63-72` (`MissileDamage`) | `lib.rs:263` (`spawn_weapon`), `world.rs:852` (×force au tir) | PARTIAL | Règle D2 « la flèche transporte les dégâts » respectée ; paliers MinLevDam 1-5, synergies et HitShift absents |
| G5 | Éléments (EType, durée ELen) | `missiles_record.go:73-80` | `d2missiles/src/lib.rs:51` (`DamageType` 6 types) | PARTIAL | Élément taggé (missiles.txt + skills.txt) mais ni durée, ni résistances, ni DoT |
| G6 | Sous-missiles, explosion, overlays, lumière, sons de missile | `missiles_record.go:105-128` | — | MISSING | `SubMissile[3]`, `HitSubMissile[4]`, `ExplosionMissile`, `TravelSound` non portés |
| G7 | Cast de compétence → missile | `d2map/d2mapentity/player.go:228` (`StartCasting`), `d2networking/d2client/game_client.go:281` (`handleCastSkillPacket`) | `world.rs:844` (`hero_ranged_attack` → `spawn_weapon("arrow")`) | PARTIAL | Tir à distance sans anim SC ni timing « cast à 50 % » ; cooldown swing 800 ms MVP |
| H1 | Client local in-proc | `d2networking/d2client/d2localclient/local_client_connection.go` | Appels directs JS↔Rust (`d2game/src/wasm.rs`) | N-A | Architecture mono-joueur navigateur : pas de couche client/serveur à simuler, l'appel direct WASM est l'équivalent fonctionnel |
| H2 | Client TCP distant | `d2client/d2remoteclient/remote_client_connection.go` | — | N-A | Serveur TCP multi-joueurs hors scope navigateur (un équivalent futur serait WebSocket/WebRTC) |
| H3 | Serveur de jeu TCP + serveur dédié | `d2server/game_server.go`, `d2networking/dedicated_server.go` | — | N-A | Idem H2 |
| H4 | Paquets réseau (13 types : GenerateMap, AddPlayer, MovePlayer, PlayerCast, Ping/Pong…) | `d2networking/d2netpacket/packet_*.go` | — | N-A | Dépend de H2/H3 |
| H5 | Connexion UDP serveur | `d2server/d2udpclientconnection/udp_client_connection.go` | — | N-A | Idem H2 |
| I1 | Moteur de scripting JS embarqué (otto) + AddFunction + evalJS | `d2script/engine.go`, `d2app/console_commands.go:61` | — | N-A | Le navigateur hôte EST un moteur JS ; le besoin d'un interpréteur embarqué disparaît (aucune API de modding exposée, mais rien à scripter dans le scope données) |
| J1 | Boucle de jeu (advance/update/render, dt) | `d2app/app.go:270-443` | `demo/game.js:699` (rAF + dt clamp 100 ms), `game.rs:693` (`GameCore::tick`/`render`) | FULL | Équivalent navigateur direct de la boucle ebiten |
| J2 | Boot/lifecycle app + config + arguments | `d2app/app.go:103-269`, `initialization.go` | `wasm.rs:28` (`D2Game::new/withLevel`), `game.js:58-90` (`?hero/?level/?continue`) | PARTIAL | Paramètres URL au lieu d'un fichier de config persisté ; pas d'écran d'erreur d'init |
| J3 | Terminal/commandes console | `d2app/console_commands.go`, `d2core/d2term` | `game.js:18` (`window.__d2` + `__d2.goto`), params `?run=1` | PARTIAL | Sondes E2E/téléport debug oui ; pas de terminal interactif, pas de dumpHeap/vsync/timescale |
| J4 | Capture GIF / frames | `d2app/app.go:452-533`, `capture_state.go` | — | MISSING | Faisable plus tard via `canvas.toDataURL` |
| K1 | Menu principal | `d2game/d2gamescreen/main_menu.go` | `demo/menu.html` + `menu.js` (logo flammes 30 frames, Solo/Continuer/GitHub/Quitter) | FULL | Périmètre mono-joueur ; le sous-menu TCP/IP Go (main_menu.go:637-661) est couvert par le N-A réseau |
| K2 | Sélection de personnage | `d2gamescreen/character_select.go` | `menu.js:36` (« Continuer ») | PARTIAL | Charge l'unique save ; pas de liste, scroll, suppression, confirmation |
| K3 | Création de héros (7 classes animées, sons, hardcore) | `d2gamescreen/select_hero_class.go` | `menu.html` cartes classes + `menu.js:41-54` | PARTIAL | 3 classes (BA/SOR/PA), pas de rendu animé de sélection ni sons de classe ni option hardcore |
| K4 | Écran de jeu | `d2gamescreen/game.go` | `demo/game.html` + `game.js` + `d2game/src/game.rs` | FULL | Map + monde + HUD + events + save/load + transitions, 60 FPS |
| K5 | Test moteur de carte (navigation régions, tile info) | `d2gamescreen/map_engine_testing.go` | `game.js:675` (`__d2.goto`), `?level=`, `composeStats` (`game.rs:563`) | PARTIAL | 2 niveaux navigables + stats de composition ; pas d'inspecteur tuile par tuile |
| K6 | Crédits défilants | `d2gamescreen/credits.go` | — | MISSING | — |
| K7 | Cinématiques (intro Blizzard, vidéos actes) | `d2gamescreen/blizzard_intro.go`, `cinematics.go`, `d2data/d2video` | — | MISSING | — |
| K8 | Bac à sable GUI (dev interne) | `d2gamescreen/gui_testing.go` | — | N-A | Outil de dev interne du repo Go, sans objet dans le port |
| L1 | HUD : globes vie/mana, boutons, minipanel | `d2player/hud.go`, `globeWidget.go`, `mini_panel.go` | `game.js:208-263` (`drawGlobes` canvas), boutons HTML son/save/load | PARTIAL | Globes D2 dessinés sur canvas ; minipanel OOD2 (inventaire/stats/skilltree/quête/groupe) réduit à 2 boutons |
| L2 | Barre d'endurance + ceinture de potions | `hud.go:497-546` (`renderStamina`, `renderPotions`) | — (potions via touche P / clic inventaire, `game.js:539-545`) | MISSING | Stamina inexistant (lié A2) ; pas de belt 4 slots |
| L3 | Barre d'expérience | `hud.go:525` (`renderExperienceBar`) | `game.js:100,265-275` (`XP_THR`, `hud.xpfill`) | FULL | Seuils XP réels col. Barbarian + tooltip absent seulement |
| L4 | Contrôles de jeu (clic déplacer/attaquer, hotkeys) | `d2player/game_controls.go`, `key_map.go` | `game.js:478-549` (router clic `monsterNear`, Maj course, I/E/P) | PARTIAL | Click-to-move + click-attack + course OK ; pas de boutons L/R skill, hotkeys non rebindables |
| L5 | Panneau d'inventaire UI (équipement, tooltips, or) | `d2player/inventory.go`, `equipment_slot.go`, `move_gold_panel.go` | `game.js:293-394` (panneau 10×4 canvas, hover, use) | PARTIAL | Grille + info-bulle survol + usage potion ; pas de slots d'équipement, pas de déplacement d'or, pas de sprites d'items |
| L6 | Arbre de compétences + icônes + sélection L/R | `d2player/skilltree.go`, `skillicon.go`, `skill_select_panel.go`, `skill_row.go` | — (`demo/main.js:32` décode skillicon.dc6 en démo décodage seulement) | MISSING | — |
| L7 | Panneau de stats héros (+ dépense de points) | `d2player/hero_stats_panel.go` | — (stats lues dans le HUD, `game.rs:579`) | MISSING | Lecture seule, pas de panneau ni dépense (dépend A2) |
| L8 | Journal de quêtes | `d2player/quest_log.go` | — | MISSING | — |
| L9 | Panneau de groupe | `d2player/party_panel.go` | — | N-A | UI multi-joueurs (relations/hostilité) — dépend du N-A réseau |
| L10 | Menu Échap / options (son, vidéo, automap) | `d2player/escape_menu.go` | `game.js:550` (toggle son), Escape ferme l'inventaire | PARTIAL | Toggle son oui ; pas de menu options complet ni vidéo/automap |
| L11 | Overlay d'aide + menu de rebind des touches | `d2player/help_overlay.go`, `key_binding_menu.go`, `binding_layout.go` | — | MISSING | Raccourcis affichés dans la barre de statut (`game.js:687`) mais fixes |
| L12 | Annonce de changement de zone | `game_controls.go:830-845` (`SetZoneChangeText`) | `game.js:397-410` (`showLevelBanner` + fondu) | FULL | Bannière + fondu noir à chaque `LevelSwitch` |

**Compte : FULL 5 · PARTIAL 23 · MISSING 22 · N-A 8 (58 features)**

---

## 2. Section détaillée avec preuves

### 2.1 d2core/d2hero — état du héros, skills, attributs

**A1/A2/A3/A4 — Stats.** Le Go sérialise un état complet :
`d2core/d2hero/hero_stats_state.go:9-30` — `HeroStatsState{Level, Experience, Strength, Energy,
Dexterity, Vitality, StatsPoints, SkillPoints, Health, MaxHealth, Mana, MaxMana, Stamina,
MaxStamina, NextLevelExp}`, généré depuis charstats.txt par `CreateHeroStatsState`
(`hero_stats_state.go:33-56` : `MaxHealth = InitVit × LifePerVit`, `MaxMana = InitEne × ManaPerEne`).
Le port Rust concentre l'équivalent dans `engine-rs/crates/d2rules/src/lib.rs:205` (`HeroStats` :
level, xp, xp_to_next, life/max_life, mana/max_mana, strength, dexterity, defense, to_hit, gold,
walk_speed, potion_heal) rempli par `world.rs:347` `init_hero` depuis le gabarit `data.rs:91`
`parse_hero_code` (valeurs réelles barbare niv 1 : 55 pv, 15 mana, str 30, dex 20, ToHitFactor 20).
La montée de niveau `world.rs:926` `apply_xp` ajoute `life_per_level`/`mana_per_level`
(`data.rs:108-109`, colonnes LifePerLevel/ManaPerLevel) et la table `XP_TABLE` (`world.rs:59-60`)
est la vraie colonne Barbarian de `assets/patch_experience.txt` — mais plafonnée au niveau 10
(`world.rs:55` `MAX_LEVEL`). Écarts : vitalité/énergie séparées, stamina, points à répartir
(statsPoints/skillPoints), formules ×PerVit/×PerEne — absents.

**A5 — Persistance.** Go : `d2core/d2hero/hero_state.go` (`HeroState{HeroName, HeroType, Act,
Equipment, Stats, Skills, X, Y, LeftSkill, RightSkill, Gold, Difficulty}`) sauvegardé en JSON
`.od2` par `hero_state_factory.go:243` `Save`, rechargé par `LoadHeroState:187` (avec
ré-hydratation des records de skills `:202-215`). Rust : `engine-rs/crates/d2rules/src/save.rs`
sérialise le monde ENTIER en JSON v1 (grille RLE, RNG `:19`, héros `HeroSave:44`, monstres
`MonSave:65`, items, missiles `:162`) avec validation au chargement (`load_json:168` : bornes,
cohérence anim DD, tpl connus) — bit-exact et testé (`d2rules/src/tests.rs`). Côté page :
`demo/game.js:555-571` (boutons Save/Load → localStorage `d2wasm.save.v1`), « Continuer »
`game.js:63-74` + `detectSaveLevel:79`. Écarts : un seul slot, pas de nom de perso, pas d'act, pas
de difficulté, pas de left/right skill.

**A6 — Skills.** Go : `d2core/d2hero/hero_skill.go` (`HeroSkill` = SkillRecord +
SkillDescriptionRecord + points, sérialisation shallow par ID) et
`hero_state_factory.go:111-148` `CreateHeroSkillsState` (base skills de charstats + tous les
skills de la classe depuis skills.txt + « Attack »). Rust : **aucun équivalent** — le héros a une
attaque mêlée fixe (`PlayerInput::Attack`, `world.rs:620`) et un tir `arrow`
(`PlayerInput::RangedAttack`, `world.rs:655`) ; aucune notion de skill, de points, de mana par sort.
C'est le manque gameplay le plus structurant (les bolts firebolt/icebolt/chargedbolt existent déjà
dans la table `d2missiles` mais rien ne les tire).

**A7 — Multi-persos.** Go : `hero_state_factory.go:73-109` `GetAllHeroStates` scanne le dossier
de saves, `character_select.go` liste/supprime. Rust : un seul emplacement localStorage
(`menu.js:56`, `game.js:56`).

### 2.2 d2core/d2stats — système de stats

Go implémente un **système à clés** complet : `d2core/d2stats/stat.go` (interface), `stat_value.go`
(valeurs combinables), `diablo2stats/stat.go:45` (`diablo2Stat` piloté par itemstatcost.txt :
DescFunc 1/6/9/11/13/14/15/16/22/24/27/28 — `stat.go:390-601`), `diablo2stats/stat_factory.go`
(stringers « +x % », nom de skill, nom de monstre…), `diablo2stats/statlist.go` (push/pop/clone/
`ReduceStats:71`/`AppendStatList:116`). Le port n'a **aucun** équivalent : `HeroStats` et `ItemDef`
sont des structs figés (`d2rules/src/lib.rs:169,205`). Conséquence en cascade : tout affixe
d'item, tout bonus d'équipement est impossible tant que ce socle n'existe pas (B1→D4).

### 2.3 d2core/d2inventory — inventaire et équipement

Go : `d2inventory/inventory_item.go` (interface), `inventory_item_armor.go`/`_weapon.go`/`_misc.go`
(taille de grille X×Y, code, nom, weapon class), `character_equipment.go` (`CharacterEquipment` :
Head/Torso/Legs/RA/LA/LH/RH/Shield), `inventory_item_factory.go:35-70` `loadHeroObjects` (items de
départ par classe : BA=hax+buc, SOR=sst×2, PA=ssd+buc…). Rust : `d2rules/src/lib.rs:154-190`
(`ItemKind{Potion,Gold,Weapon,Armor}`, `ItemDef{code,name,kind,amount}` — or empilable,
constructeurs `ItemDef::gold`/`ItemDef::potion`) et un inventaire PLAT 10×4 = 40 slots 1 case
(`world.rs:88-90`), rempli par pickup (`world.rs:690-697`, événement `InventoryFull`), consommé
par `UseItem` (`world.rs:702-713`). L'« équipement de départ » existe seulement comme stats brutes
(`startWeapon/dmgMin 3-6`, `startShield/acMin 4-6`, `data.rs:110-113`) utilisées dans les formules
de dégâts/défense (`world.rs:349,822-823`). L'écran : `d2game/d2player/inventory_grid.go`
(`canFit:187`, `Set:209`, `Remove:227`) n'a pas d'équivalent multi-cases.

### 2.4 d2core/d2item — génération d'items, affixes

Go (`d2core/d2item/diablo2item/`) est le générateur d'items le plus abouti du repo :
- `item_factory.go:163` `rollDropModifier` (qualité selon TreasureClass), `:199`
  `rollTreasurePick`, `:226` `ItemsFromTreasureClass`, `:286` `ItemFromTreasure`, `:324`
  `FindMatchingAffixes` (filtre ilvl/type/groupes, poids pondérés), `:372`
  `resolveDynamicTreasureCode` (codes « armo3 » etc.) ;
- `item.go` : `pickMagicAffixes:331`, `pickUniqueRecord:312`, `pickSetRecords:320`,
  `generateAllProperties:408`, `generateName:610`, `Identify:802`, `Label:127`,
  `GetItemDescription:830` ;
- `item_property.go:118` `eval` + les 14 fn* (charges, procs, skills de classe…).

Rust : `world.rs:1116` `roll_drop` — 60 % rien / 25 % or (`lvl×5..×15`) / 15 % potion `hp1`.
La doc du port l'assume : `lib.rs:29-30` « structure prête pour TreasureClassEx réel (phase 3) ».
Statut **MISSING** pour D1-D5 (aucun affixe, qualité, propriété, nom généré, identification).

### 2.5 d2common/d2calculation — parseur de formules

Go : `d2calculation/calcstring.go` (`CalcString`), `calculation.go` (AST `BinaryCalculation`,
`UnaryCalculation`, `TernaryCalculation`, `PropertyReferenceCalculation`, `ConstantCalculation`
avec `Eval()`), `d2lexer/lexer.go` (188 l.), `d2parser/parser.go` (312 l.) + `operations.go`
(opérateurs D2). C'est le moteur des colonnes `*Calc` de missiles.txt/skills.txt. Rust : absent ;
les deux formules nécessaires au MVP sont codées en dur dans `world.rs:190-194` (`hit_chance =
200×TH/(TH+AC)` borné [5,95], §3 de docs/GAME-DATA.md) et `world.rs:822` (dégâts ×(1+str/100)).
Statut MISSING — le besoin n'apparaîtra qu'avec skills/affixes/synergies.

### 2.6 d2common/d2enum — partie données de jeu

- **F1 Hero** : Go `d2enum/hero.go` (7 classes) + `GetToken`/`GetToken3`. Rust : résolution
  `data.rs:76` `resolve_hero` sur `manifest.heroes{BA,SOR,PA}` (3 classes) ; les tokens complets
  (AI, DZ…) sont documentés dans `docs/OPENDIABLO2-NOTES.md` §1.2. PARTIAL.
- **F2 Modes d'animation** : Go `player_animation_mode.go`/`monster_animation_mode.go` (DT, NU, WL,
  RN, GH, TN, TW, A1, A2, BL, SC, TH, KK, S1-S4, DD). Rust : `d2rules/src/lib.rs:125`
  `AnimState{NU,WL,RN,A1,DD}` — 5 sur 16. PARTIAL (le blit `game.rs:851-858` et les sets de
  sprites `game.rs:474-475` sont déjà génériques sur les tags : ajouter un mode = données + enum).
- **F3 Le reste** : `difficulty.go`, `quests.go`, `players_relationships.go`, `region_id.go`,
  `equipped_slot.go`, `item_quality.go`, `hero_stance.go`… sans équivalent (1 acte, 1 difficulté,
  pas de quêtes). MISSING.

### 2.7 Missiles — d2records + d2mapentity + intégration

- **G1 Table** : Go `d2core/d2records/missiles_record.go` parse l'intégralité de missiles.txt
  (calc strings, lumière, animation, collision, dégâts physiques+élémentaires, sous-missiles,
  sons, overlays). Rust : `d2missiles/src/table.rs` embarque la table **réelle** v1.12
  (`assets/patch_missiles.txt`, 684 lignes → `data/missiles_real.json`, 10 définitions MVP) avec
  les colonnes Missile/Id/Vel/Range/HitClass/EType/CelFile/AnimLen, EType complété depuis
  `patch_skills.txt` (en-tête `table.rs:1-18`). PARTIAL : 10 missiles (arrow, bolts de sorcière,
  javelin, throwaxe…) sur ~400 records.
- **G2 Vol** : Go `d2mapentity/missile.go` (`SetRadians` vers une cible à `record.Range`,
  `Advance` = Step). Rust : `d2missiles/src/lib.rs:122` `Missile{x,y,vx,vy,travelled,range}` +
  `tick_walls:321` avec sous-pas uniformes anti-tunneling (`SUBSTEP_TILES 0.5`, `MAX_SUBSTEPS 64`)
  — en pratique plus robuste que le Go (leur issue #819 « Advance missile incomplet »), mais
  limité à la ligne droite : les `ServerMovementFunc` (spirales, forks, accélération `Accel`) ne
  sont pas portés. PARTIAL.
- **G3 Collision** : Go décrit tout dans `MissileCollision` (`missiles_record.go:48-62` :
  types 0/1/3/6/8, friendly fire, timers) mais **n'implémente pas** la résolution (leur moteur
  missiles ~10 %, cf. OPENDIABLO2-NOTES.md §3). Rust : hit-test entités (rayon 0,7 tuile,
  `world.rs:50,884-888`) + murs (`world.rs:890-893`) + `CollideKill` implicite (le missile meurt à
  l'impact, `lib.rs:166`). ≈ type 3 D2. PARTIAL.
- **G4 Dégâts** : règle D2 portée fidèlement — les carriers d'arme (arrow…) transportent les
  dégâts de l'arme résolus au tir ×force (`world.rs:844-860` `hero_ranged_attack` →
  `spawn_weapon`, `lib.rs:263`), les bolts de skill portent leurs dégâts officiels niv 1
  (firebolt 3-8, icebolt 3-7, chargedbolt 2-6, magicarrow +1, `lib.rs:27-32`). Écarts : pas de
  paliers `MinLevDam1-5`, pas de `DmgSymPerCalc`, pas de HitShift (×2^hitshift). PARTIAL.
- **G5 Éléments** : `DamageType{Physical,Fire,Cold,Lightning,Magic,Poison}` (`lib.rs:51`) taggé
  depuis EType missiles+skills ; ni durée (`ELen`), ni résistances, ni DoT. PARTIAL.
- **G6 Sous-missiles/explosion/overlays/lumière/sons** : `SubMissile[3]`, `HitSubMissile[4]`,
  `ExplosionMissile`, `TravelSound/HitSound/ProgSound`, `MissileLight` — MISSING.
- **G7 Cast → missile** : Go `player.go:228` `StartCasting` (anim SC, cast effectif à 50 % de
  l'anim) + `game_client.go:281` `handleCastSkillPacket` (spawn missiles + overlay). Rust :
  `world.rs:844` — face à la cible, cooldown swing 800 ms, spawn immédiat ; pas d'anim SC, pas de
  timing 50 %, un seul missile `arrow` câblé (`world.rs:53` `HERO_MISSILE`). PARTIAL.

### 2.8 d2networking — client/serveur LAN TCP

Go : `d2client/game_client.go` (466 l. : routage des paquets, création entités missiles/NPC côté
client), `d2client/d2localclient` (in-proc), `d2client/d2remoteclient` (TCP, framing JSON
`bytesToJSON:179`), `d2server/game_server.go` (496 l. : listener TCP, `packetManager:183`,
broadcast `sendPacketToClients:200`, `handleClientConnection:341`), `d2networking/dedicated_server.go`
(mode serveur headless), 13 paquets dans `d2netpacket/packet_*.go` (GenerateMap, AddPlayer,
MovePlayer, PlayerCast, ItemSpawn, Ping/Pong, SavePlayer, UpdateServerInfo, ServerFull/Closed,
ConnectionRequest/DisconnectRequest), plus une connexion UDP (`d2udpclientconnection`).

Rust : **rien** — et c'est un choix d'architecture assumé : `docs/OPENDIABLO2-NOTES.md` §2.6
marque `d2networking → — (hors scope MVP, single player)`. Le port est un binaire WASM
mono-joueur où JS appelle Rust directement (`d2game/src/wasm.rs` : `tick`, `render`, `saveGame`…).
**N-A** pour H1-H5 avec la justification : serveur TCP multi-joueurs impossible/hors scope dans un
navigateur centré données ; un équivalent moderne serait WebSocket/WebRTC + state-sync, à décider
en phase multi. Notons au passage que le déterminisme RNG du port (seed sérialisée dans la save,
`save.rs:19`) est le prérequis utile à une future synchro réseau lockstep.

### 2.9 d2script — scripting JS embarqué

Go : `d2script/engine.go` — VM otto (JS), `AddFunction`, `RunScript`, `Eval` gated, branchée au
terminal (`d2app/console_commands.go:61` `evalJS`). Rust : N-A — le navigateur hôte EST le moteur
JS ; il n'y a rien à scripter dans le périmètre données (aucune API de modding exposée, mais le
besoin Go venait du terminal desktop). Si un jour le jeu veut une console, c'est du pur côté page.

### 2.10 d2app — boucle de jeu et lifecycle

- **J1 Boucle** : Go `app.go:270` `Run` → `update:434` → `advance:403` → `render:384` (ebiten).
  Rust : `demo/game.js:699` `frame` (rAF, `dt = min(now-last, 100)`), `game.js:703-713` →
  `game.tick(dt, input)` → events → `heroStats` → `inventory` → `render()` RGBA → `putImageData`,
  à 60 FPS (285×205 tuiles town, 360+ bloodmoor). FULL.
- **J2 Boot/config** : Go `Create:103`, `loadEngine:159`, `parseArguments:195`, `LoadConfig:237`
  (fichier config persisté). Rust : `D2Game::new/withLevel` (`wasm.rs:28-40`) + params URL
  `?hero/?level/?continue/?run` (`game.js:58-90,667`) + barre de chargement progressive des assets
  critiques/optionnels (`game.js:113-150,630-645`). Pas de config persistée (résolution,
  préférences). PARTIAL.
- **J3 Terminal** : Go `console_commands.go` (dumpHeap, evalJS, fullscreen, capture, vsync, fps,
  timescale, quit, gui playground) via d2term. Rust : sondes E2E `window.__d2`
  (`game.js:18-46` : errors, events, level, hero, invCache, monCodes, `goto(level)` `:675`,
  `refreshStatus`) + compte d'erreurs dans le titre d'onglet. Utile mais pas un terminal.
  PARTIAL.
- **J4 Capture GIF** : Go `app.go:452-533` + `capture_state.go`. Rust : absent (les captures E2E
  viennent du driver navigateur, hors moteur). MISSING.

### 2.11 d2game/d2gamescreen — écrans

- **K1 Menu principal** : Go `main_menu.go` (logo flammes DC6, boutons Solo/GitHub/Quit/Credits/
  Cinematics/MapTest, sous-menu TCP/IP `:637-661`). Rust : `demo/menu.html` + `menu.js` — logo en
  flammes 30 frames animées à ~15 fps (`menu.js:16-25`), Solo (choix de classe), Continuer,
  GitHub (`shpaw415/diablo2-wasm`), Quitter, sons de clic. FULL au périmètre mono-joueur (le
  multi Go → N-A réseau).
- **K2 Sélection de perso** : Go `character_select.go` (liste, scroll, suppression avec
  confirmation `:244,484-506`). Rust : simple bouton « Continuer » (grisé sans save,
  `menu.js:56`). PARTIAL.
- **K3 Création de héros** : Go `select_hero_class.go` (7 classes rendues en composite animé,
  descriptions, sons, checkbox hardcore, bouton OK). Rust : 3 cartes de classe (BA/SOR/PA,
  `menu.html`) → `?hero=CODE`. PARTIAL.
- **K4 Écran de jeu** : Go `game.go` (`OnPlayerMove:321`, `OnPlayerSave:339`, `OnPlayerCast:357`,
  debug spawn items/monstres `:369-415`). Rust : `demo/game.html`+`game.js`+`d2game/src/game.rs`
  — carte composée DS1+DT1, monde d2rules, HUD, events→sons/messages, warp multi-niveaux
  (`game.rs:693-739` : `LevelSwitch`/`LevelSwitchFailed`, cooldown anti-rebond 1500 ms, transfert
  héros par save patchée `patch_save_for_level:963`), spawn monstres par niveau avec RNG par
  clé (`LevelRng:1001`, `fnv1a:1020`), town zone sûre. FULL (scope MVP).
- **K5 Map engine test** : Go `map_engine_testing.go` (parcourt 40+ régions, affiche les infos
  tuiles). Rust : `?level=bloodmoor`, `window.__d2.goto`, `composeStats` (drawn/hidden/oob,
  `game.rs:563`) et `monsterCodes` — 2 niveaux seulement, pas d'inspecteur de tuile. PARTIAL.
- **K6/K7 Crédits + cinématiques** : Go `credits.go` (défilement), `blizzard_intro.go`,
  `cinematics.go` (lecteur d2video). Rust : absent. MISSING ×2.
- **K8 GUI testing** (`gui_testing.go`) : bac à sable dev du repo Go. N-A.

### 2.12 d2game/d2player — HUD et UI de jeu

- **L1 Globes/HUD** : Go `hud.go` (barre du bas DC6, tooltips, boutons run/walk) +
  `globeWidget.go` (globes frame-based). Rust : globes vie/mana dessinés à la main sur canvas avec
  dégradé + reflet + cerclage (`game.js:218-263`), texte vie/mana/niveau/or, badge course
  (`game.js:711`). Pas de minipanel (le Go ouvre stats/inventaire/skills/quêtes/groupe) — les
  boutons HTML son/save/load jouent ce rôle pour la partie persistance. PARTIAL.
- **L2 Stamina + belt** : Go `hud.go:497-546`. Rust : ni stamina (A2), ni ceinture 4 slots ; les
  potions passent par l'inventaire (touche P = première potion, `game.js:539-545`, ou clic sur le
  slot). MISSING.
- **L3 Barre d'XP** : Go `hud.go:525`. Rust : barre `hud.xpfill` alimentée par les seuils réels
  `XP_THR` (`game.js:100`) cohérents avec `world.rs:59`. FULL (tooltip d'XP en moins).
- **L4 Contrôles** : Go `game_controls.go` (click-to-move/attack, répéter clic maintenu,
  hotkeys, boutons L/R skill) + `key_map.go`. Rust : routeur de clic
  `monsterNear(wx,wy,2.5) → attack|move` (`game.js:478-484` + `game.rs:514`), anneau de clic
  (`drawClickRing`), Shift = course (persisté ?non — transient), I/E/P/Escape, Maj maintenu.
  Pas de répartition gauche/droite, pas de rebinding. PARTIAL.
- **L5 Inventaire UI** : Go `inventory.go` + `equipment_slot.go` (slots d'équipement depuis
  inventory.txt) + `move_gold_panel.go`. Rust : panneau 10×4 dessiné sur canvas (`drawInventory`
  `game.js:316`), hover + info-bulle (`game.js:378-392`), clic = utiliser/équiper-plus-tard
  (`game.js:508-519` note « équipement à venir (phase 3) »). PARTIAL.
- **L6 Skill tree** : Go `skilltree.go` (3 onglets par classe, icônes, points) +
  `skillicon.go` + `skill_select_panel.go`. Rust : absent en jeu — `demo/main.js:32` décode
  `skillicon.dc6` (24 frames) mais seulement comme démo du décodeur DC6. MISSING.
- **L7 Panneau de stats** : Go `hero_stats_panel.go` (dépense des points +). Rust : stats
  affichées dans le HUD (`hero_stats_json`, `game.rs:579`) mais pas de panneau ni de dépense.
  MISSING.
- **L8 Quêtes** : Go `quest_log.go` (onglets par acte, icônes). Rust : absent. MISSING.
- **L9 Groupe** : Go `party_panel.go` (relations hostilité). N-A (présuppose le multi, N-A réseau).
- **L10 Menu Échap/options** : Go `escape_menu.go` (options son/vidéo/automap/contrôles). Rust :
  toggle son (`game.js:550-554`), Escape ferme l'inventaire. PARTIAL.
- **L11 Aide + rebind touches** : Go `help_overlay.go`, `key_binding_menu.go`, `key_map.go`,
  `binding_layout.go`. Rust : raccourcis listés en statut (`game.js:687`) mais figés. MISSING.
- **L12 Changement de zone** : Go `game_controls.go:830-845`. Rust : bannière animée + fondu noir
  à chaque `LevelSwitch` (`showLevelBanner:397`, `levelFade:404`). FULL.

---

## 3. Manques priorisés

### P0 — Débloque la boucle gameplay D2 (fort impact, socles existants)
1. **Compétences (A6 + L6 + G7)** : état de skills, points, cast → missiles. La table
   `d2missiles` contient DÉJÀ firebolt/icebolt/chargedbolt/magicarrow avec dégâts réels — le
   manque est l'état (`HeroSkill` équivalent) et le câblage `skill → missile` (aujourd'hui
   `HERO_MISSILE="arrow"` codé en dur, `world.rs:53`). Port naturel dans `d2rules` + un panneau
   minimal dans la page.
2. **Panneau de stats + points à répartir (A2 + L7)** : vit/ene séparés, statsPoints — prérequis
   pour que le leveling (déjà livré, `apply_xp`) ait un choix de joueur.
3. **Ceinture de potions (L2)** : 4 slots, touche 1-4 — très visible, données `patch_misc.txt`
   déjà extraites.

### P1 — Boucle loot (valeur forte, gros chantier)
4. **Génération d'items (D1)** : porter `TreasureClassEx` + `item_ratio` (Go
   `item_factory.go:163-330`) — le port a déjà la structure `ItemDef` et l'emplacement documenté
   (« phase 3 »).
5. **Système de stats génériques (B1-B3 + E1)** : `Stat/StatList` + parseur `CalcString` —
   socle obligatoire pour D2-D5 (affixes, uniques, propriétés) et pour les synergies de
   compétences (G4).
6. **Affixes + qualités (D2-D5)** : une fois 5 livré, port direct des tables Go
   `item_affix_*`.

### P2 — Complétude HUD/écrans
7. **Équipement + slots (C2 + L5)** : `CharacterEquipment` 8 slots, AC/dégâts dérivés des objets
   portés.
8. **Multi-sauvegardes + sélection de perso (A7 + K2 + K3)** : index localStorage, liste,
   suppression, 7 classes.
9. **Menu Échap/options + rebind touches (L10 + L11)**.
10. **Stamina (A2/L2)** et modes d'anim manquants (GH touché, BL blocage, SC cast — F2 ; les sets
    de sprites `game.rs:443-502` sont déjà génériques par tag).

### P3 — Cosmique / contenu
11. **Crédits (K6)** — trivial (défilement HTML).
12. **Missiles avancés (G1/G2/G6)** : plus de records de la table réelle, 1-2 fonctions de
    mouvement non-linéaires, sous-missiles/explosion.
13. **Cinématiques (K7)** — dépend d'un pipeline vidéo (hors données).
14. **Quêtes (F3/L8)** — contenu, pas moteur.
15. **Capture GIF (J4)** — `canvas.toDataURL` + encodage côté page.

### Hors scope (réseau/scripting — N-A)
- Client/serveur TCP, paquets, UDP, serveur dédié (H1-H5) : ne prendre sens qu'avec une
  architecture WebSocket/WebRTC dédiée multi-joueurs.
- Scripting embarqué (I1) : le navigateur remplace otto ; sans objet.

---

## 4. Compte final

| Statut | Count |
|--------|-------|
| FULL | **5** |
| PARTIAL | **23** |
| MISSING | **22** |
| N-A | **8** |
| **Total** | **58** |

Lecture d'ensemble : le port couvre **mieux que OpenDiablo2** les morceaux qu'il cible
(simulation déterministe testable, save bit-exact, pathfinding A* réel vs leur raycast,
collision missiles effectivement résolue vs leur moteur ~10 %, multi-niveaux avec warps) ;
OpenDiablo2 garde l'avance sur tout ce qui touche aux **données de règles à grande échelle**
(skills, stats génériques, génération d'items, formules CalcString) et sur les **écrans UI**
(skill tree, quêtes, options) — cohérent avec le positionnement « port navigateur centré
données » et avec l'analyse déjà consignée dans `docs/OPENDIABLO2-NOTES.md` §4-6.
