# Finir le jeu — Audit « SYSTÈMES HÉROS & RÈGLES » (2026-10-22)

> Objectif : **boucle PvE solo complète** — 5 actes × 3 difficultés, 7 classes,
> création du perso → mort de Baal, avec loot complet, skills, personnalisation.
> Ce rapport audite **le jeu qu'on finit** (comportement D2 v1.12), PAS la parité
> OpenDiablo2 — OD2 n'est cité que comme référence de code quand il en a une.
> Zéro code : audit seul. Preuves = fichiers cités (`chemin:ligne`) + sondes MPQ
> (`engine-rs/target/release/d2tool probe`, méthode `shortcut-extrait-mpq-hash-d2`).
>
> **Tailles** (développement solo, données déjà extraites) : S ≤ 1 j · M 2-4 j ·
> L 1-2 sem · XL > 2 sem.
> **Phases** : P1 socles · P2 héros complet & équipement · P3 loot, économie &
> monde vivant · P4 3 difficultés & complétion (« créa → Baal Hell »).
>
> Constat transverse n°1 : **presque toutes les tables de règles v1.12 sont
> DÉJÀ dans `engine/patch_d2.mpq`** (sondes du 2026-10-22, §1) — le manque est
> quasi toujours **le moteur, pas les données**.
> Constat transverse n°2 : `d2rules/src/data.rs` lit un `manifest.json` figé
> (3 héros, 6 monstres) ; tant qu'il n'y a pas de lecture TSV runtime (ou de
> génération manifest élargie), chaque nouveau record = éditer le manifest à la
> main. OD2 résout ça par `d2records` (76 loaders) — formats #22/#25.

---

## 1. État des DONNÉES du domaine (sondes MPQ du 2026-10-22)

Extraites et embarquées dans le port (`assets/`, consommées ou non) :

| Table | Fichier extrait | Records | Consommée par le moteur ? |
|---|---|---:|---|
| CharStats (7 classes) | `assets/patch_charstats.txt` | 7 + header exp | NON — 12 champs recopiés dans le manifest (`data.rs:94`) |
| Skills | `assets/patch_skills.txt` | 210 (30/classe) + Attack/Kick | NON — 3 skills de départ codés (`skills.rs:42`) |
| Missiles | `assets/patch_missiles.txt` | 684 lignes | 10 déf (`d2missiles/src/table.rs`) |
| TreasureClassEx | `assets/TreasureClassEx.txt` | 853 | OUI (`loot.rs:34`) |
| Weapons/Armor/Misc | `patch_weapons/armor/misc.txt` | 307/203/147 | familles d'équivalence loot seulement (`loot.rs:36-40`) |
| Experience | `patch_experience.txt` | 99 niv × 7 classes | 10 niveaux col. Barbarian (`world.rs:59`) |
| MonLvl | `assets/MonLvl.txt` | 111 niv × 3 diff | ligne niv 1 Normal en dur (`world.rs:66-82`) |
| UniqueItems / SetItems / Gems / Runes(Runeword) / CubeMain | `patch_uniqueitems/setitems/gems/runes/cubemain.txt` | 403/128/69/169/150 | NON |
| MonStats / levels / LvlPrest / LvlTypes / monsounds | `patch_monstats.txt` etc. | 734 | 6 monstres via manifest (`data.rs:121`) |

**Présentes dans `engine/patch_d2.mpq` (sonde `d2tool probe` = hit), NON extraites :**

| Table | Records | À quoi elle sert dans ce domaine |
|---|---:|---|
| `magicprefix.txt` / `magicsuffix.txt` | 669 / 747 | affixes magic (ilvl, mods, groupes, transform) |
| `rareprefix.txt` / `raresuffix.txt` | 45 / 154 | noms des rares |
| `ItemRatio.txt` | 5 | tirage de qualité low/normal/magic/rare/set/unique |
| `ItemStatCost.txt` | 358 | ids de stats, DescFunc, coût/vente, save bits — **le dictionnaire du système Stat** |
| `Automagic.txt` | 36 | affixes automatiques (orbes, wands de démarrage…) |
| `qualityitems.txt` / `lowqualityitems.txt` | 7 / 3 | qualités spéciales (d2exp pour low) |
| `skilldesc.txt` | 220 | libellés/descriptions de l'arbre de compétences |
| `hireling.txt` | 119 | **mercenaires A1 Rogue/A2 Desert/A3 Sorcerer/A5 Barb** (HP/Lvl, Dmg/Lvl, skills) |
| `Sets.txt` | 32 | bonus de set COMPLETS (setitems.txt ne couvre que les pièces) |
| `inventory.txt` | 33 | géométrie UI : grille 10×4, slots d'équipement, **stash 6×4/6×8, belt, cube 3×4** |
| `belts.txt` | 15 | nombre de slots de ceinture par item |
| `gamble.txt` | 125 | prix de gamble par base |
| `StorePage.txt` | 3 | pages des magasins |
| `DifficultyLevels.txt` | 3 | **NM/Hell : ResistPenalty −40/−100, DeathExpPenalty 5/10 %, odds gamble, divisors leech** |
| `states.txt` / `overlay.txt` | 184 / 294 | auras, poison, freeze, transform — pilotent états & visuels |
| `events.txt` / `elemtypes.txt` / `pettype.txt` / `compcode` / `hitclass` / `plrmode` / `bodylocs` | — | petites tables d'appui (pets, compos, modes) |
| Sprites `soSkillicon.dc6`, `baSkillicon.dc6`… (`data\global\ui\SPELLS\`, d2data.mpq) | 1/classe | icônes d'arbre par classe (`skillicon.dc6` générique déjà extrait) |

**Vraies absences de données** : `CubeModifier.txt` absent des 4 MPQ (v1.12
gère tout via `cubemain.txt` + `CubeType.txt`, présent) — rien à faire.
Libellés fr-FR dynamiques (noms d'affixes/uniques, descriptions) = tables
STRING/TBL, **non extraites** (formats #21, autre domaine) : les noms d'items
générés en dépendent.

---

## 2. Tableau des manques (Id | Manque | Pourquoi | État/preuve | Taille | Dépendances | Phase)

### A. Socles transverses

| Id | Manque | Pourquoi (réf. D2 v1.12) | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| A1 | **Système Stat/StatList générique** : stats à clés entières (dictionnaire `ItemStatCost`), valeurs, listes, agrégation équipement→héros | Tout bonus d'item/skill/affixe s'exprime en `stat.id += v` ; sans ce socle aucun équipement n'est possible | `HeroStats`/`ItemDef` structs figés `d2rules/src/lib.rs:212,177` ; Go : `d2core/d2stats/stat.go`, `statlist.go` (`ReduceStats:71`) — données : `ItemStatCost.txt` 358 rec. dans patch_d2.mpq (sonde) | L | — | **P1** |
| A2 | **Parseur/évaluateur CalcString** (lexer/parser/eval : `stat('xx')`, `skill('..').lvl`, opérateurs D2) | Pilote toutes les colonnes `*Calc` : synergies, AR, auras, mods d'affixes | 2 formules codées en dur : `world.rs:205` (toucher `200·TH/(TH+AC)` [5,95]), `world.rs:873` (dmg ×(1+str/100)) ; Go : `d2common/d2calculation/` (calcstring.go, calculation.go, d2lexer 188 l., d2parser 312 l.) — port direct | L | A1 | **P1** |
| A3 | **Lecture TSV runtime** (ou pipeline manifest élargi) pour consommer les 20+ tables ci-dessus | Éviter de figer chaque record dans le manifest ; indispensable pour 210 skills × affixes × 734 monstres | `data.rs` = lecteur JSON pointer-par-pointer ; formats #22/#25 (aucun parseur txt runtime) | M | — | P1 |
| A4 | **Extraction du lot de tables §1** (une commande `stormlib_read` par table + golden dumps) | Données nécessaires à C/D/E/F/G/H/K | sondes = hits, extraction outillée (`tools/stormlib_read.py`, `d2tool probe`) | S (M avec golden tests) | — | **P1** |

### B. 7 classes — charstats, sprites, démarrement

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| B1 | **Charstats complet 7 classes** : str/dex/int/vit, stamina, hpadd, LifePerVitality/StaminaPerVitality/ManaPerMagic/StatPerLevel, ManaRegen, ToHitFactor, BlockFactor, StartSkill, Skill 1-10, **items de départ item1-5 avec location/count** | Le démarrement réel par classe (l'Ama démarre avec jav + bouclier + 4 hp1 + 1 tsc + 1 isc, cf. ligne Amazon) | Données COMPLÈTES dans `patch_charstats.txt` (7 classes) ; moteur : `data.rs:94 parse_hero_code` = 12 champs manifest, 3 classes, arme/bouclier en stats brutes (`data.rs:110-113`), pas d'items | M | A1, A4 | **P1-P2** |
| B2 | **Sprites COF/DCC 4 classes manquantes** (AM, NE + exp. AS, DR) **+ modes manquants** (SC cast, BL bloc, GH touché, S1-S4, KK…) | 7 classes jouables ; cast = anim SC avant l'effet | `assets/dcc/` = BA/SO/PA × 4 modes (WL/NU/A1/DD, 39 COF/156 DCC) ; `AnimState` 5/16 (`lib.rs:130`) ; l'empilement COF est déjà générique par tag (`d2game/src/game.rs:443-502`) ; DCC des 4 classes dans d2char.mpq/d2exp.mpq (sondables) | M | A4 | P2 |
| B3 | **Création de héros 7 classes + multi-sauvegardes** (nom, portrait, liste, suppression) | « création du perso » ; un perso = un slot nommé (`.d2s` mental) | `demo/menu.html` 3 cartes → `?hero=CODE` ; 1 slot localStorage `d2wasm.save.v1` (`demo/game.js:555`) ; save v2 porte skills + hero_code (`save.rs:45-48,145-146`) mais pas le nom | M | B2 | P2 |
| B4 | **Démarrement réel** : StartSkill + Skill 1-10 + items item1-5 ÉQUIPÉS (pas des stats) | Chaque classe démarre différemment ; le port doit poser les objets, pas des constantes | `skills.rs:42` 3 classes à 1 skill codé en main ; classes non couvertes ⇒ vide ; GO : `hero_state_factory.go:111-148` (base skills charstats + tous les skills + Attack) | S-M | B1, F6 | P2 |

### C. Arbre de compétences complet

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| C1 | **Modèle Skill complet 210 skills** (données présentes : reqskill1-3, reqlevel, delay/perdelay, leftskill, srvmissile a/b/c, HitShift, SrcDam, MinDam/MaxDam + MinLevDam1-5, EType/EMin/EMax + EMinLev1-5, DmgSymPerCalc, EDmgSymPerCalc, ELen + ELevLen1-3, aurastate/aurastat*, passives) | C'est LA grosse pièce « rules » : chaque skill = record + calculs | `skills.rs:22` = `HeroSkill{code, level, missile}` ; tout le reste absent. Données : `patch_skills.txt` 210 records confirmées (30/classe × 7) | L | A2, A4 | **P1** (modèle+data) → P2 (comportements) |
| C2 | **Points de skill** : +1/niveau, dépense, plafond 20, skills hoquets | Personnalisation = cœur de la définition du jeu | Aucun `skillPoints` nulle part (`HeroStats lib.rs:212`, save v2) | S-M | A1, D1 | **P2** |
| C3 | **Prérequis d'arbre** (≥1 point dans reqskill1-3) | La progression de l'arbre | Données prêtes (`reqskill1-3`) ; moteur absent | S | C1, C2 | **P2** |
| C4 | **Synergies** (`DmgSymPerCalc` etc. : %/niveau des skills liés) | Le calcul de dégâts D2 tardif (Fire Bolt +13 %/lvl Fire Wall…) | Colonnes présentes (`patch_skills.txt`, cols 233/247/252) ; éval = CalcString | M | C1, A2 | **P2** |
| C5 | **UI arbre 3 onglets × 7 classes** : icônes DC6 par classe, points, libellés fr | « personnalisation » visible ; icônes = `skillicon.dc6` (déjà décodé, `demo/main.js:32`) + `soSkillicon/baSkillicon…` (d2data.mpq, sondes) ; textes = `skilldesc.txt` 220 rec. (sonde) | Go : `d2game/d2player/skilltree.go`, `skillicon.go` ; Rust : rien en jeu | L | C2, C3, (TBL fr) | P2-P3 |
| C6 | **Skills passifs / auras / maîtrises / invocations** (`aurastate`, `states.txt` 184 rec., `overlay.txt`, passivestat, sumskill) | Paladin auras, Barb maîtrises, Nécro squelettes : 80 % du contenu de l'arbre | Rien ; dépend du moteur d'états (buffs/débuffs) qui n'existe pas | XL | C1, A1, A2, E4 | P3 |

### D. Stats héros complètes

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| D1 | **Vitalité/Énergie séparées + dépense de 5 pts/niveau** (StatPerLevel=5) + formules ×PerVit/×PerEne | Le levelling D2 = CHOIX du joueur | `HeroStats` n'a que str/dex (`lib.rs:227-230`) ; données prêtes (`patch_charstats.txt` : LifePerVitality 4-12…) ; Go : `hero_stats_state.go:9-30` | M | A1, B1 | **P1-P2** |
| D2 | **Stamina + drain course + barre** (RunDrain=20, StaminaPerVitality) | Course illimitée aujourd'hui = pas D2 | Course marche (`PlayerInput::SetRun lib.rs:80`) sans jauge ; Go `hud.go:497-546` | S-M | D1 | P2 |
| D3 | **Résistances (4 + max) + Magic/Gold Find** | Pilotées par affixes et plafonnées par difficulté (−40/−100 NM/Hell) | Absentes (aucune clé de stat) ; n'existenteront qu'après A1 + D3-data (`ItemStatCost`) | M | A1, K2 | P2-P3 |
| D4 | **Régén mana** (`ManaRegen` charstats) + tick mana | Sans régén, la sorcière est à sec pour toujours | Aucun regen dans `tick` (`world.rs`) ; potions seulement | S | D1 | P2 |
| D5 | **XP 1→99 complet × 7 classes** | La boucle « créer → Baal » est longue ; plafond niv 10 = mur | `world.rs:59` `MAX_LEVEL=10`, `XP_TABLE` col. Barbarian ; `patch_experience.txt` = 99 × 7 (MaxLvl 99) | S | — | **P1** |
| D6 | **Or : cap par niveau (10 k × lvl), sac/stash séparés, achat = soustraction réelle** | Économie D2 ; conditionne vendors/gamble | `gold: u32` unique (`lib.rs:235`), pas de cap, pas de séparation | S | F9 | P3 |

### E. Formules de dégâts & toucher v1.12

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| E1 | **Dégâts d'arme réels** : min/max de l'arme ÉQUIPÉE (tables), ED items/skills, scaling str OU dex selon type (bow = dex), +max/min dmg, +dmg élémental | L'arme de départ 3-6 est une constante, pas un objet | `world.rs:873` rand(startWeapon 3-6) ×(1+str/100) ; `world.rs:913` idem au tir ; données : min/max dans `patch_weapons/armor.txt` | M-L | F1, F6, A1 | **P2** |
| E2 | **Toucher complet** : `100·AR/(AR+DR) · 2·alvl/(alvl+mlvl)` borné 5-95, AR héros/monstre des tables (+AR/Lvl), vs taille du monstre | La formule du port ignore alvl/mlvl (paliers de niveau) | `world.rs:205` `200·TH/(TH+AC)` ; `to_hit = ToHitFactor+dex` (`world.rs:392`) ; MonLvl TH fixe 8 (`world.rs:70`) | S-M | D1 | P2 |
| E3 | **HitShift + paliers de dégâts skills** (`×2^HitShift/256`, MinLevDam1-5/MaxLevDam1-5) | Les vraies courbes de dégâts des 210 skills | `d2missiles` : bolts figés niv 1 (firebolt 3-8, `lib.rs:30-32`), aucun HitShift (`MissileDef lib.rs:88-118`) ; colonnes présentes dans `patch_skills.txt` | M | C1, A2 | **P2** |
| E4 | **Éléments complets** : résists appliqués (héros/monstre), durées `ELen`/DoT poison, froid ralentit, absorb, immunités monstres | Feu/foudre/froid/poison = la moitié du gameplay | `DamageType` 6 taggé (`lib.rs:51`) ; ni durée ni résist ni DoT ; `states.txt`/`elemtypes` présents (sondes) | L | A1, C6, K2 | P3 |
| E5 | **Mods de coup** : deadly strike, blocage (BlockFactor charstats + bouclier), leech vie/mana (divisors DifficultyLevels), crushing blow, open wounds, pierce | Fin de levée de ambiguïté des affixes et des builds | Rien ; tout arrive avec A1/F3 (affixes) | M-L | A1, F3 | P3 |

### F. Items complets

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| F1 | **Objet riche** : grille X×Y (invwidth/height), weapon class, req str/dex/lvl, durabilité, prix (cost, cost mult/add), stacks | Base de tout (loot, équipement, vendors) | `ItemDef{code,name,kind,amount}` (`lib.rs:177`) ; colonnes prêtes dans `patch_weapons/armor/misc.txt` ; Go : `inventory_item_armor.go` etc. | M | — | **P2** |
| F2 | **Qualités** (low/supérieur/normal/magic/rare/set/unique/crafted) via `ItemRatio.txt` (5 rec.) | Le cœur du loot D2 : aujourd'hui un drop = un code de base nu | `loot.rs:78` `RolledDrop{code, gold}` — aucune qualité ; Go : `item_factory.go:163` rollDropModifier ; ItemRatio/qualityitems/lowqualityitems dans patch_d2.mpq (sondes) | M | A1, A4 | **P2** |
| F3 | **Affixes prefix/suffix** : filtre ilvl/type/groupes, poids, 1-3 mods, transform couleur, noms rares (rareprefix/raresuffix), Automagic | « magic ring of … » : sans ça, pas de loot D2 | Rien ; Go : `FindMatchingAffixes item_factory.go:324`, `item.go:331-386` ; données dans patch_d2.mpq (sondes : 669/747/45/154 rec.) | L | F2, A1, A2 | **P2-P3** |
| F4 | **Propriétés → stats** (property eval + les 14 `fn*` : charges, procs, +skills de classe, all-res…) | Les colonnes prop1-12 d'uniques/sets/affixes/cube parlent ce langage | Rien ; Go : `item_property.go:118` (le seul morceau testé du d2item Go) | L-XL | A1, A2 | P2-P3 |
| F5 | **Uniques + Sets** : résolution unique par code, propriétés, bonus de set COMPLETS (`Sets.txt` 32 rec. ≠ setitems pièces) | Chasse au loot = définition du jeu | Tables uniques/setitems extraites (`patch_uniqueitems.txt` 403, `patch_setitems.txt` 128) mais NON consommées ; Sets.txt à extraire (sonde OK) | M | F2, F4 | P3 |
| F6 | **Équipement 8 slots** (head/torso/legs/RA/LA/LH/RH/shield — D2 réel : +amulette, 2 anneaux, gants, bottes, ceinture, armes swap) **+ dérivation des stats depuis l'équipement** (AC, AR, dmg, stats…) | « personnalisation » ; l'état porté pilote E1/E2/D3 | AUCUN slot : « équipement de départ » = stats brutes (`data.rs:110-113`) ; Go : `character_equipment.go` (8 slots) ; géométrie réelle dans `inventory.txt` (sonde : rArm/torso/head/…) | L | A1, F1 | **P2** |
| F7 | **Grille inventaire multi-cases** (canFit X×Y, drag & drop, sprites d'items) | Les épées 2×4 ne tiennent pas dans un slot 1×1 | 40 slots 1-case (`world.rs:95-97`) ; `inventory.txt` Trade Page 1 = gridX 10 × gridY 4 (sonde — la géométrie D2 est là) ; Go : `inventory_grid.go` (canFit:187) | M | F1 | **P2** |
| F8 | **Stash 6×4 (+or stash) et Belt 4×4** (touches 1-4, potions auto-rangées) | PvE solo complet : gestion d'inventaire | Absents (inventaire plein = drop refusé, event `InventoryFull lib.rs:125`) ; géométries dans `inventory.txt` (Bank Page 6×4, belt) + `belts.txt` (sondes) | M | F7 | P2-P3 |
| F9 | **Identification** (items unidentified, rouleaux/tomes tsc/isc, Cain) | Uniques/rares dropent unidentified en D2 | Rien ; `tsc/isc` présents dans `patch_misc.txt` | S-M | F2, F5 | P3 |
| F10 | **Réparation/durabilité/vente** | Économie & vendors | Durabilité absente de F1 ; vente = G1 | S-M | F1, G1 | P3 |
| F11 | **Cube horadrim** : UI 3×4 (`inventory.txt` Transmogrify Box), moteur de recettes `patch_cubemain.txt` (150 rec. extraites : numinputs, output, mod 1-5, op/param/value) | Système emblématique ; convertit runes/gemmes/items | Rien ; Go : records seuls (`cubemain_record.go`), **pas de logique runtime — à construire** | M-L | F4, F7 | P3 |
| F12 | **Gemmes + runes + sockets + runewords** : `patch_gems.txt` 69 (mods par socket type), runes r01-r33 (misc), sockets (col. gemsockets), `patch_runes.txt` = table Runeword (169 rec. : Rune1-6, itype1-6, T1Code1-7) | Fin de chaîne du loot ; contenu de fin de jeu | Rien (aucun socket) ; tables extraites | M-L | F4, F7 | P3 |

### G. Vendors & gamble

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| G1 | **Marchands** : entités NPC (Akara/Charsi/Kashya/Gheed…), fenêtres de store (StorePage 3 rec.), prix achat/vente (cost × cost mult/add), réserve d'or du marchand, restock au changement de niveau de perso | Économie de la boucle PvE ; potions/antidotes en achats | **Aucun NPC** dans le port (spawn = monstres seulement, `world.rs` auto_spawn) ; Go : records seuls (`storepage_record.go`, `gamble_record.go`), pas de logique — à construire ; sprites NPC = DCC monstres (d2data.mpq) | XL | B2, F1, F7, A4 | **P3** |
| G2 | **Gamble (Gheed)** : prix (gamble cost misc + gamble.txt 125 rec.), odds unique/set/rare selon clvl (`DifficultyLevels.txt` GambleRare/Set/Unique/Uber/Ultra confirmés) | Le puits à or de fin de partie | Rien | M | G1 | P3 |

### H. Mercenaires

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| H1 | **Mercenaires** : embauche (A1 Rogue/A2 Desert/A3 Sorcerer/A5 Barb — le brief citait A1-A3-A5 ; la table v1.12 a 4 types), scaling HP/AR/Dmg/Resist /Lvl + /Exp/Lvl (`hireling.txt` 119 rec., extrait à titre de sonde le 2026-10-22), skills 1-5 avec niveaux, suivi du héros, **résurrection chez le NPC** (coût formule par niveau), survit entre les niveaux | Compagnon = pilier du solo D2 (les builds Hell en dépendent) | Rien ; Go : `hireling_record.go` + `hireling_description` = records seuls, pas d'entité ni de logique | L-XL | G1, A1, E1-E3 | **P3-P4** |

### I. Potions, consommables, régén

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| I1 | **Potions complètes** hp1-5 (soins croissants), mp1-5 (mana), rejuv rvs/rvl (35/100 %), antidote/stamina/thawing (états temporaires) | La belt D2 = 4×4 de potions variées | `hp1` unique, soin constant 60 (`world.rs:48 POTION_HEAL`) ; mana potions non utilisables (`loot.rs:100` misc_kind → hpot/rpot seulement) ; données `patch_misc.txt` complètes (147 rec.) | S-M | A1, C6 (états) | **P2** |
| I2 | **Parchemins/tomes TP & Identify + sort Town Portal** | Retour au camp sans marcher = qualité de vie PvE ; identify de F9 | Rien ; données dans `patch_misc.txt` (tsc/isc/tbk/isb) ; TP = missile/skill + waypoint (domaine cartes) | M | F9, C1 | P3-P4 |

### J. Mort & résurrection du héros

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| J1 | **Corps du héros** : reste au lieu de la mort avec l'équipement ; résurrection en allant au camp puis en récupérant le corps (ou clic sur le corps) | La mort doit COÛTER : aujourd'hui respawn auto 2 s pleine vie, sans perte | `world.rs:838 respawn_hero` (pleine vie, spawn, reset aggro) + `RESPAWN_MS=2000 world.rs:46` ; message page « résurrection au camp dans 2 s » (`demo/game.js:461`) | M | F6 (items du corps) | **P3** |
| J2 | **Perte d'or + XP à la mort** : or porté posé au sol avec le corps ; NM/Hell : DeathExpPenalty 5 %/10 % (`DifficultyLevels.txt` confirmé) | Idem, dimensionnée par difficulté | Rien (respawn ne touche ni or ni xp) | S | J1, K1 | P3 |

### K. Difficultés NM / Enfer

| Id | Manque | Pourquoi | État / preuve | Taille | Dépend. | Phase |
|---|---|---|---|---|---|---|
| K1 | **Enum difficulté + sélection + persistance** (Normal/NM/Hell débloqués en séquence) ; les niveaux re-rôlent par difficulté | « 3 difficultés » de la définition du jeu | 1 seule difficulté implicite ; save v2 sans champ difficulté (`save.rs:53`) ; Go : `d2enum/difficulty.go`, `difficultylevels_record.go` | S (mécanique) + cartes (autre domaine) | J2 | **P3-P4** |
| K2 | **Scaling MonLvl par difficulté** : colonnes `AC(N)/AC(H)`, `HP(N)/HP(H)`, `DM(N)/DM(H)`, `XP(N)/XP(H)` de `MonLvl.txt` (111 niveaux, extrait) | Les monstres NM/Hell = la même table, autres colonnes | `world.rs:66-82` : facteurs niveau 1 Normal codés en dur (HP×7/100, DM×2/100, XP×30/100, AC 6, TH 8) | S-M | A4 | **P4** |
| K3 | **Pénalités de difficulté côté héros** : ResistPenalty −40/−100, MonsterSkillBonus 3/7, UniqueDamageBonus 90/75/66, divisors leech/freeze (`DifficultyLevels.txt` confirmé) | C'est CE qui rend Hell difficile | Rien ; consommateurs D3/E4 | S-M | D3, E4, K1 | **P4** |
| K4 | **Boss & super-uniques** (résolution SuperUniques.txt — sonde OK —, propriétés monprop, auras d'élite monumods/UniqueModifier) : Baal doit exister | « mort de Baal » = condition de fin | Rien côté règles ; `patch_monstats.txt` 734 rec. extraites non consommées (6 dans le manifest) ; Go : `monster_super_unique_record.go` (records seuls) | L | C1, E4, (domaine monstres/cartes) | **P4** |

---

## 3. Definition of done — « systèmes héros & règles »

La boucle « créer → Baal » est FINIE pour ce domaine quand, **sans éditer de
code à la main pour ajouter du contenu** :

1. **7 classes créables** (nom + choix en menu), chacune démarre avec SES items
   équipés (charstats `item1-5`) et SON skill de départ réel.
2. **Niveaux 1→99** (table XP × 7 classes), +5 stats et +1 skill par niveau,
   dépensables dans les panneaux (vit/ene/stamina inclus, formules ×PerVit
   exactes, régén mana, stamina drain).
3. **Arbre complet navigable** : 210 skills, prérequis, synergies évaluées par
   CalcString, plafond 20, icônes DC6 par classe, libellés fr (skilldesc).
4. **Loot complet** : TreasureClassEx → ItemRatio (qualité) → affixes ilvl →
   propriétés → stats via Stat/StatList ; uniques/sets/runewords/gemmes
   identifiés, nommés, descriptions fr.
5. **Équipement** : 8+ slots, grille multi-cases, stash, belt 4×4 ; AC/AR/dmg/
   résists DÉRIVÉS de l'équipement (aucune stat de départ en dur).
6. **Économie** : marchands (achat/vente/réparation/réserve or), gamble Gheed,
   cube horadrim (recettes cubemain v1.12).
7. **Mercenaires** embauchables (4 types), scalés par `hireling.txt`,
   résurrectibles chez le NPC.
8. **Mort** : corps au sol, récupération, perte d'or + pénalité XP par
   difficulté.
9. **NM et Enfer jouables** : MonLvl colonnes (N)/(H), résists négatifs,
   pénalités DifficultyLevels — jusqu'à Baal Hell tué en conditions réelles.
10. Tout **déterministe au seed** (contrat RNG d2rules) et couvert par **golden
    tests** record-à-record contre les tables v1.12 (pattern
    `shortcut-extrait-mpq-hash-d2`) ; `cargo test --workspace` vert (baseline
    actuelle : 143 passed, `SYNTHESE.md` §phase 2).

Dépendances hors-domaine à commander tôt : TBL fr-FR (formats #21) pour les
noms/descriptions générés ; régions/warps/waypoints/quêtes et IA/sprites
super-uniques (rapports cartes & monstres) pour atteindre Baal.

## 4. Top 5 ordonné (chemin critique vers la DoD)

1. **A4 + A3 — extraire et brancher les tables** (`magicprefix/suffix`,
   `ItemRatio`, `ItemStatCost`, `skilldesc`, `hireling`, `Sets`, `inventory`,
   `gamble`, `DifficultyLevels`, `states`…) + lecture TSV runtime.
   Une journée de données débloque tout le reste.
2. **A1 + A2 — Stat/StatList + CalcString** : LE socle ; sans lui ni affixes,
   ni synergies, ni propriétés, ni uniques. Port direct du Go
   (`d2stats`, `d2calculation`) + `ItemStatCost` comme dictionnaire.
3. **C1→C4 + D1 + D5 — levelling complet** : modèle Skill 210 records,
   points (5 stats / 1 skill par niveau), prérequis, synergies, XP 99 × 7
   classes. La personnalisation existe, la sorcière ne jette plus un bolt figé.
4. **F1 + F6 + F7 + E1 + E2 — équipement & combat dérivés** : objet riche
   X×Y, 8 slots, stats dérivées des objets réels (dmg min/max tables, ED,
   toucher avec alvl/mlvl). L'équipement de départ devient de VRAIS objets.
5. **F2 + F3 + F4 + F5 — le loot D2 réel** : ItemRatio → qualités, affixes
   prefix/suffix filtrés par ilvl, propriétés → stats, uniques/sets. C'est le
   moment où « le loot tombe comme dans D2 » — condition sine qua non de la
   boucle créa→Baal.

Ensuite (P3-P4) : vendors/gamble/mercs/cube/sockets/runewords, mort avec
corps, puis difficultés NM/Enfer (MonLvl colonnes + pénalités) et super-
uniques/Baal — chaque rangée du tableau §2 cite sa propre preuve.
