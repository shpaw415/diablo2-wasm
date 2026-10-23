# finish-monde — Audit « MONDE & CONTENU » : ce qu'il manque pour FINIR LE JEU

> 2026-10-22 · Audit statique, zéro code. Domaine : monde (niveaux, warps, waypoints,
> quêtes, monstres, shrines, NPC de ville) — pour aller de la création du perso à la
> mort de Baal, 5 actes × 3 difficultés, 7 classes, en solo navigateur.
> **Ne recopie pas** les audits de parité od2 (`SYNTHESE.md`, `od2-features-formats.md`,
> `od2-features-moteur.md`, `od2-features-gameplay.md`) : ils comparent à OpenDiablo2 ;
> ici on inventorie ce que LE JEU doit contenir et ce qui manque. Hors-scope flaggé :
> multi-joueurs (N-A assumé), cinématiques Bink, hirelings (domaine limitrophe, flaggé §W16).

Méthodo : les chiffres « données » sont comptés ce jour depuis les MPQ réels
(`data/d2exp.mpq` listfile complet 10 096 noms ; `engine/patch_d2.mpq` tables finales v1.12)
via `tools/stormlib_read.py` (StormLib) — cf. `docs/GAME-DATA.md` §Méthodo. Les chiffres
« port » sont cités fichier:ligne dans `engine-rs/` et `demo/`.

---

## 0. État actuel du monde (une ligne par fait, preuve incluse)

| Fait | Preuve |
|---|---|
| **2 niveaux** jouables : town (56×40) + bloodmoor (72×72) | `assets/manifest.json` `levels` l.1336 (town, `levelsId:1`) et l.1370 (bloodmoor, `levelsId:2`) |
| Blood Moor = DS1 **synthétique composé de 36 presets réels** (le retail n'a AUCUN DS1 complet de ce niveau, DrlgType=3, taille retail 80×80) | `evidence/MAP-SPEC-BLOODMOOR.md:17,43-49` ; `tools/bloodmoor_build.py` ; `assets/map/presets_bloodmoor/` (36 fichiers) |
| **1 paire de warps** town↔bloodmoor (cercle + cooldown 1500 ms + test aller-retour) | `game.rs:36,62-70,759-781`, test `game.rs:1854` |
| **6 gabarits monstres** au manifest (FA ZM SI SK FS YE), **3 réellement spawnés** (FA×3 ZM×2 SI = constante codée en dur), 5-8 par niveau | `manifest.json:592` ; `game.rs:952` `DEFAULT_OUTDOOR_MONSTERS` ; `game.rs:394-452` |
| Scaling monstre = facteurs **MonLvl niveau-1 Normal figés** (AC 6, TH 8, HP×7/100, DM×2/100, XP×30/100) | `d2rules/src/world.rs:66-82` (appliqués `spawn_monster_at` world.rs:463) |
| IA = FSM 4 états (Idle/Chase/Attack/Dying), aggro 6.0, leash 10.0 | `world.rs:36-44,104-114` |
| **ZÉRO** waypoint, quête, NPC, shrine, objet interactif, champion/unique/boss (grep = 0 hits dans engine-rs/ + demo/) ; Andariel→Baal n'apparaissent nulle part | grep ce jour ; `docs/parity/od2-features-gameplay.md:42` (quêtes MISSING) |
| Bannière de zone = `nameDisplay` figé du manifest (« Camp des Rousses », « Blood Moor (Acte I) ») | `demo/game.js:400-425` ; `game.rs:968-973` |

## 1. Inventaire des données v1.12 disponibles (comptées ce jour)

**Déjà extraites dans `assets/`** : `patch_levels.txt` (139 lignes : 135 niveaux + Null),
`LvlPrest.txt` (1 090 presets, tous avec File1 ; 227 marqués Expansion),
`LvlTypes.txt` (37 types), `MonLvl.txt` (110 niveaux de scaling N/H),
`patch_monstats.txt` (735 lignes), `TreasureClassEx.txt` (853 TCs), `monsounds.txt` (141).

**Dans les MPQ, jamais extraites** (lignes = en-tête inclus ; source `engine/patch_d2.mpq`
sauf mention) : `SuperUniques.txt` **68** · `monpreset.txt` **230** · `MonPlace.txt` **38** ·
`monumod.txt` **44** (modificateurs champions/uniques) · `monstats2.txt` **611** (équipement,
hauteur, Utrans) · `monai.txt` **149** · `monseq.txt` **1011** (d2exp) · `monname.txt` **44**
(d2exp) · `npc.txt` **18** · `objects.txt` **575** · `objgroup.txt` **134** · `shrines.txt`
**24** · `LvlWarp.txt` **90** · `LvlMaze.txt` **83** · `LvlSub.txt` **36** · `AutoMap.txt`
**3288** · `gamble.txt` **127** · `hireling.txt` **121** · `SoundEnviron.txt` **51** ·
`pettype.txt` **21** · `Overlay.txt` **295** · `ElemTypes.txt` **14** · `difficultylevels.txt`
**4**. Aucune table de quêtes n'existe dans les MPQ (grep `quests` sur le listfile d2exp =
0 ; la logique de quête est dans D2Game.dll, seul `levels.Quest/QuestFlag` la pointe).

**Sprites/tuiles** : listfile d2exp = **603 DS1** (ACT1 57 · ACT2 5 · ACT3 15 · ACT4 46 ·
expansion 480) et **62 DT1**. Note structurante : les niveaux maze (DrlgType 1) n'ont pas
besoin de DS1 (grilles LvlMaze + DT1), d'où les petits comptes ACT2/ACT3.

**Ce que `patch_levels.txt` contient déjà par niveau** (header 140 colonnes vérifié) :
`DrlgType` (répartition 70 maze / 35 preset / 31 wilderness, ligne Null incluse),
`LevelType` (→ LvlTypes), `Vis0-7` + `Warp0-7` (graphe de transitions), `MonLvl1/2/3`
(ex. Blood Moor : 1/26/51), `MonDen` (+N/H, 124 niveaux >0), `MonUMin/MonUMax` (+N/H),
`NumMon`, **`mon1..mon10` (126 niveaux remplis)**, `nmon1..10`, **`umon1..umon10` (125
niveaux remplis)**, `cmon1-4`, `Waypoint` (**39 entrées flaggées, ids 1..38, doublon 17
sur « Act 5 - Pandemonium 2 »** ; Blood Moor elle-même = 255, son waypoint vient du preset
chunk `waypoint.ds1`), `LevelName` (nom EN, le fr-FR est dans les TBL), `Quest`
(2 niveaux : « Act 1 - Cave 1 » = Den of Evil, « Act 2 - Arcane »), `ObjGrp0-7/ObjPrb0-7`,
`SizeX/Y (+N/H)`.

**Par acte (patch_levels, colonne Act)** : Acte I = 39 niveaux, 8 waypoints (ids 1-8) ;
Acte II = 35 niveaux, 9 waypoints (9-17) ; Acte III = 28 niveaux, 8 waypoints (18-26) ;
Acte IV = 6 niveaux, 3 waypoints (27-29) ; Acte V = 28 niveaux, 10 waypoints (30-38).
Soit **135 niveaux et 38 waypoints** pour finir le jeu (× 3 difficultés de scaling).

**Chaîne boss** : Andariel, Duriel, Mephisto, Diablo, Baal ne sont PAS des super-uniques —
ce sont des records `monstats.txt` (`andariel`/`uberandariel`, `duriel`, `mephisto`+
`mephistospirit`, `diablo`+`diabloclone`, famille Baal : `baalthrone`, `baalcrab`,
`baaltaunt`, `baalhighpriest`, `baalcrabstairs`, `baaltentacle1`…) instanciés par la
logique de quête. Les sous-boss sont dans `SuperUniques.txt` (67 records, vérifié :
Blood Raven→`bloodraven` est monstats ; Radament, The Smith, Corpsefire, Bishibosh,
Rakanishu, Coldcrow, Bonebreak, Treehead WoodFist, Griswold, Boneash, The Summoner,
Nihlathak Boss, Ancient Barbarian 1-3, « Baal Subject 1-5 » = vagues du trône…).
Izual (`izual`), Hephasto (`hephasto`), Blood Raven (`bloodraven`) = monstats dédiés.

---

## 2. Le tableau — Id | Manque | Nécessaire pourquoi | État actuel/preuve | Taille | Dépendances | Phase

Tailles : **S** ≤ 1 j · **M** 1-3 j · **L** 3-10 j · **XL** > 10 j (ordre de grandeur
personne-jour, données+moteur). Phases : **P1** socle data-driven · **P2** Acte I complet ·
**P3** Actes II-III · **P4** Actes IV-V + Baal · **P5** ×3 difficultés + polish.

| Id | Manque | Nécessaire pourquoi | État actuel / preuve | Taille | Dépendances | Phase |
|---|---|---|---|---|---|---|
| **W01** | Lecteur runtime des tables de niveaux (levels/LvlTypes/LvlPrest/LvlMaze/LvlWarp/LvlSub) — mini `RecordManager` + parseur TXT générique | Sans lui chaque niveau = un bloc JSON à la main dans le manifest ; 135 niveaux × 3 diffs impossibles à entretenir | Tables extraites mais **consommées par personne** ; manifest figé sur 2 clés (`manifest.json:1336-1414`) ; « pas de parseur TXT runtime » (`od2-features-formats.md` #22/#25) | L | — | P1 |
| **W02** | DRLG des 3 types : maze (DrlgType 1, LvlMaze 82 lignes), preset/stamps (DrlgType 2, LvlPrest 1 090), wilderness (DrlgType 3) | 70 niveaux maze + 35 preset + 31 wilderness à générer ; c'est LE gros bloc moteur du domaine | wilderness : proto prouvé en Python (Blood Moor, `tools/bloodmoor_build.py`, MAP-SPEC-BLOODMOOR §4) mais DS1 **figé**, pas seedé en Rust ; maze/preset : **rien** ; Go n'a que `d2mapgen/act1_overworld.go` (town+wilderness A1) | XL | W01 | P1 (port proto) → P3-P4 (par acte) |
| **W03** | Bibliothèque DS1/DT1/palettes multi-actes : extraction ciblée (603 DS1 / 62 DT1 disponibles) + PL2 actes 2-5 + gamma/teintes | Rendre les actes II-V aux bonnes couleurs (PL2 = 1 seule palette acte I aujourd'hui : `assets/map/pal_act1_pl2.dat`) | Extraction prouvée (Town 8 DT1, Blood Moor 3 cliffs ; `d2tool` + StormLib) ; rien pour A2-A5 | L | W02 | P3-P4 |
| **W04** | Graphe de transitions complet : `Vis0-7`/`Warp0-7`/`LevelWarp` + `LvlWarp.txt` (89 records : SelectX/Y, ExitWalk, Direction) | Chaque niveau a 1-8 voisins ; le port ne sait faire qu'UNE paire codée en dur | `WarpDef{to,at,r,entry}` par niveau au manifest (`game.rs:62-70`) ; LvlWarp jamais lu ; Go le charge sans le consommer (`level_warp_loader.go`) | M | W01 | P1 |
| **W05** | **Waypoints** : objet WP (rendu, activation, liste persistée), UI de voyage (panneau), wire vers W04 | 38 waypoints = colonne vertébrale de la progression ; grep `waypoint` = 0 hit code (le preset `waypoint.ds1` est collé en décor, `bloodmoor_build.py:58,250`) | **Aucun système** ; Go non plus (objet animé seulement, `object_init.go:39-46` « opened unconditionally ») ; 39 niveaux flaggés dans patch_levels | L | W04, W14, W19 | P2+ (par acte) |
| **W06** | Placements de monstres data-driven : planches `mon1-10`/`nmon`/`umon` de patch_levels (126/125 niveaux remplis), `MonPreset.txt` (229), `MonPlace.txt` (37), densité `MonDen`/`NumMon`, **packs MinGrp/MaxGrp** (monstats:26-27) | Remplacer `DEFAULT_OUTDOOR_MONSTERS = ["FA","FA","FA","ZM","ZM","SI"]` codé en dur (`game.rs:952`) ; le retail Blood Moor = `zombie1,fallen1,quillrat1`, MonDen=520 | 6 tentatives de spawn par niveau à positions aléatoires (`game.rs:394-452`) ; packs/groupes inexistants | L | W01, W07 | P1 (moteur) / P2-P4 (données) |
| **W07** | MonStats complet : 735 records vs 6 ; colonnes `Level(N)/(H)`, HP(N)/(H), TC 1-3 (difficultés), `AI`/`aip1-8` ±(N/H), `rangedspawn`, `spawnx/y/mode`, `placespawn`, `isSpawn`, `MinGrp/MaxGrp`, monstats2 (610) pour équipement/hauteur/Utrans | Tout le contenu monstre du jeu repose sur cette table ; le port n'en lit que 7 champs et ignore `ac`/`th` du manifest | `MonTpl` = level/hpMin/hp/dmgMin/dmgMax/exp/speed/treasureClass (`d2rules/src/data.rs:39-62,128-135`) ; « pas de monstats2/équipement/IA-records/super-uniques » (`od2-features-formats.md` #29) | M (parseur) / L (intég.) | W01 | P1 |
| **W08** | Scaling 3 difficultés : MonLvl.txt lignes 2-110 (N=1 figé aujourd'hui), `MonDen(N)/(H)`, `MonUMin/Max(N)/(H)`, tailles `SizeX(N)/(H)`, `difficultylevels.txt` (3 records) | Nightmare (MonLvl2) et Hell (MonLvl3) = le « ×3 difficultés » de la définition du jeu | Facteurs niveau-1 Normal **constantes** (`world.rs:66-82` : AC 6, TH 8, ×7/100, ×2/100, ×30/100) ; MonLvl.txt extrait mais seule la ligne 1 est consommée | S-M | W07 | P5 |
| **W09** | Champions & uniques : `monumod.txt` (43 records, flag champion, cpick/upick), boosts HP/dégâts, auras, teintes `Utrans`/palshift, densité par niveau `MonUMin/Max` | Les champions (bleus) et uniques (dorés) sont dans chaque zone du jeu ; 0 code (grep champion/unique = 0 hit) ; Go les charge sans les consommer (`monster_unique_modifiers_loader.go`, const `monumod_const_index.go` jamais référencées) | **Rien** des deux côtés ; TC champion (« Act 1 Champ A »…) déjà dans TreasureClassEx extrait | L | W06, W07 | P2 (A1) → P3-P5 |
| **W10** | Super-uniques : `SuperUniques.txt` (67 records : Class→monstats, Mod1-3, MinGrp/MaxGrp, AutoPos, TC/(N)/(H), Utrans) + spawn aux positions `umon1-10` des niveaux | Les 60+ boss nommés (Bishibosh→Nihlathak, Ancients, vagues Baal) ; table jamais extraite dans `assets/` | **Aucun** super-unique ; Go : record parsé jamais instancié (`monster_super_unique_loader.go`, prouvé par l'agent Go) | M-L | W06, W07, W09 (mods) | P2 (A1) → P3-P4 |
| **W11** | **Boss de quête Andariel→Baal** : records monstats dédiés (andariel, duriel, mephisto+mephistospirit, diablo, famille baal ×6+, bloodraven, radament, izual, hephasto, nihlathak+boss), spawns scriptés, arènes (Baalt?), mort de Baal = fin du jeu | La définition même de « finir » : de la création du perso à la mort de Baal. Ces boss n'existent **nulle part** dans le port (grep andariel/baal = 0 hit) ; ils ne sont PAS des SuperUniques (voir §1) | Sprites DCC extraits seulement pour 6 monstres A1 (FA ZM SI SK FS YE) ; boss = données + sprites + scripts | XL (avec W13) | W13, W07, W03 (sprites) | P2 (Andariel) → P3 (Duriel, Mephisto) → P4 (Diablo, Baal) |
| **W12** | Résurrections / invocations / comportements spéciaux : shaman ressuscite les Fallen, Blood Raven invoque des zombies, vagues « Baal Subject 1-5 » du trône, `monai.txt` (148) + `aip1-8`, flee des Fallen | Sans eux les packs ne ressemblent pas au jeu et Baal est untuable en l'état ; IA actuelle = contact seulement | FSM 4 états sans skills (`world.rs:104-114`) ; FS (FallenShaman) présent au manifest mais **jamais spawné** et sans logique de résurrection | L | W07, W06 | P2-P4 |
| **W13** | **Quêtes** : moteur d'états (q1-qN par acte, flags, journal), triggers (zones/objets/monstres), récompenses (XP/objets/accès — ex. Den of Evil → Akara soigne, Arcane → Duriel), ~6 quêtes/acte (A4 : 3) | La boucle PvE « actable » demandée ; **aucune table txt dans les MPQ** (prouvé §1) → contenu à author (wiki/DLL RE) + moteur | **Zéro** code/donnée (grep quest = 0) ; Go n'a qu'une UI à statuts codés en dur (`d2game/d2player/quest_log.go:82-110` « should come from save file ») ; `patch_levels.Quest` pointe 2 niveaux (Cave 1, Arcane) | XL | W05, W11, W14, W18 | P2 (A1) → P3-P4 |
| **W14** | Objets interactifs : `objects.txt` (574), `objtype`, `objgroup.txt` (133 : groupes Shrines/Wells/décor posés par ObjGrp0-7), modes o1/o2, `OpenWarp` (portails), coffres (→ TC), portes | Les DS1/presets référencent ces objets (lookup Go codé en dur ~7 900 lignes : waypoints l.895, portails l.1067, NPC l.11-20…) ; sans eux les villes sont vides et pas de TP | **Zéro** entité objet : les DT1 `objects` sont des tuiles statiques (`compose.rs:371`) ; Go : `object_details_loader.go` parse mais LookupObject ne sert qu'aux stamps item | L | W01, W02 | P2 (ville A1) → P3-P4 |
| **W15** | Shrines & puits : `shrines.txt` (23 records : Effect, Code, Arg0/1, durée frames, reset minutes, rareté) + effets de jeu (+dmg, résistance, mana recharge…) | « Shrines/misc » du domaine ; présence aléatoire attendue dans chaque wilderness (via objgroup) | **Zéro** ; effets non codés ; table jamais extraite | M | W14 | P2-P4 |
| **W16** | **NPC de ville + rôles** : roster 5 actes (Akara/Kashya/Charsi/Warriv/Gheed/Cain → Lut Gholein, Kurast, Forteresse, Harrogath), sprites DCC, section PATH des DS1 (patrouilles), interaction/dialogue (TBL fr), vendeur (gamble 126, StorePage) — **hirelings flaggé hors-scope P1-P4** (hireling.txt 120 lignes disponibles si décidé) | La ville est l'hub de chaque acte : soigner, acheter, réparer, dialogues de quête ; aujourd'hui la ville est une zone sûre **vide** | `manifest.json:1361` town `monsters: []` ; grep npc/vendor = 0 hit ; parseur DS1 **ne lit pas PATH** (`ds1.rs:7`) ; Go : NPC = patrouille sans IA (`d2mapentity/npc.go`), vendors jamais consommés | L | W14, W18 (textes), W13 (dialogues quête) | P2 (A1) → P3-P4 |
| **W17** | AutoMap : `AutoMap.txt` (3 287 records : Style, Start/EndSequence, Cel1-4) + overlay isométrique | Le jeu retail est injouable sans carte (mazes A2/A3) ; présenté en dernier car non bloquant pour la boucle | **Zéro** ; Go le charge (`automap_loader.go`) sans rendu | M | W02 | P5 |
| **W18** | Textes fr-FR : TBL (string tables) + noms de niveaux (`LevelName`), noms monstres (`monname.txt` 43), libellés quêtes/NPC/objets | Bannière de zone, journal de quêtes, dialogues, labels ; aujourd'hui `nameDisplay` est figé au manifest | TBL = MISSING (`od2-features-formats.md` #21) ; bannière = manifest (`game.js:400-425`) | M | — (W13/W16 en dépendent) | P2 |
| **W19** | Sauvegarde du monde : waypoints activés, quêtes, difficulté, niveaux visités, état des objets | Waypoints/quêtes non persistés = pas de boucle de jeu entre sessions | save JSON v2 = héros + monde courant seulement (`d2rules/src/save.rs`, world entier sérialisé) ; .d2s réel = chantier ouvert (`PLAN.md:14`) | M | — (W05/W13 y écrivent) | P2 |
| **W20** | Ambiance/musique par région : `SoundEnviron.txt` (50), musiques par acte, panning | L'identité sonore des 5 actes (jungle A3, lave A4…) ; déjà signalé PARTIAL côté moteur (`od2-features-moteur.md` #26) | 13 sfx globaux, zéro ambiance/musique | S | — | P5 |

### Cross-domaine (rappel, PAS détaillé ici)
Héros 7 classes, items/affixes, skills/missiles, UI inventaire → voir
`od2-features-gameplay.md` (gameplay) et `od2-features-formats.md` (formats). Le domaine
MONDE y puise : sprites monstres (décodage DCC OK, il faut EXTRAIRE ceux des A2-A5),
TC drops (branchés en phase 2 : `impl-loot.md`).

---

## 3. Definition of done du domaine MONDE & CONTENU

Le domaine est FINI quand, dans le navigateur, solo :

1. **Traversée complète** : depuis la création du perso, on traverse les 5 actes via leurs
   niveaux générés (chaque niveau obligatoire de la chaîne existe, seedé/reseedable) et
   reliés par warps fonctionnels — sans éditer le manifest à la main (W01-W04).
2. **38 waypoints** : chaque waypoint du jeu est présent dans son niveau, activable au clic,
   voyage instantané via l'UI, persisté entre sessions (W05, W19).
3. **Contenu monstre fidèle** : chaque niveau spawn sa planche `mon1-10` à la densité
   `MonDen`, en packs MinGrp/MaxGrp ; champions (bleu) et uniques (doré, teinte) avec leurs
   mods monumod ; super-uniques nommés aux positions `umon` (W06-W10).
4. **Boucle de quêtes** : chaque acte a ses quêtes activables (journal, états, récompenses)
   et son boss final tuable — Andariel, Duriel (avec l'orbe/tuna), Mephisto, Diablo,
   **Baal** (trône + vagues) — la mort de Baal termine le jeu à l'écran (W11, W13).
5. **Villes vivantes** : NPC présents avec rôles minimaux (soin/vente/réparation/waypoint/
   dialogue de quête) dans les 5 villes (W14, W16).
6. **×3 difficultés** : Normal/Nightmare/Hell rejouent les 5 actes avec MonLvl2/3,
   MonDen(N/H), tailles N/H, MonU(N/H) (W08) ; drops TC(N)/(H) déjà dans la table extraite.
7. **Hors-scope assumé (flaggé)** : multi-joueurs, cinématiques Bink, hirelings (données
   dispo si décidé), vaches secrètes, automap P5, ambiance P5.

## 4. Top 5 ordonné (chemin critique vers « Baal mort »)

1. **W01 + W04** — Lecteur runtime des tables de niveaux + graphe Vis/Warp générique.
   Débloque TOUT le reste ; taille L ; les données sont déjà extraites.
2. **W06 + W07 + W08** — Contenu monstre data-driven (planches par niveau, 735 MonStats,
   MonLvl 3 difficultés, packs). Transforme le spawn codé en dur en vrai contenu ; M-L.
3. **W02** — DRLG maze/preset/wilderness (porter `bloodmoor_build.py` en Rust seedé +
   LvlMaze + stamps LvlPrest). Le plus gros bloc moteur (XL) — à attaquer par acte, en
   même temps que W03.
4. **W13 + W11** — Moteur de quêtes + chaîne boss Andariel→Baal. C'est la définition du
   « finir le jeu » ; aucun contenu n'existe (pas de table) : authoring + scripts (XL).
5. **W05 + W19** — Waypoints (39 niveaux flaggés, UI, activation) + leur persistance.
   La colonne vertébrale de la progression entre chaque session de jeu (L).

*Audit statique, aucune écriture de code. Sources : comptes MPQ ce jour
(`tools/stormlib_read.py` sur `engine/patch_d2.mpq`/`data/d2exp.mpq`), `assets/*.txt`,
`engine-rs/crates/*` (chemins:lignes), `demo/`, audits `docs/parity/od2-*.md`, référence Go
`/home/agent/workspace/opendiablo2` (chemins:lignes cités en §2).*
