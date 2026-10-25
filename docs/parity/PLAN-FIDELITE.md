# PLAN-FIDÉLITÉ — Ce qui reste pour une fidélité complète de Diablo 2 (2026-10-24)

> État de départ : campagne d2w-re étapes 1-13 terminées (828 tests verts, +470 depuis
> le 22 oct). Boucle **créa → Baal Hell fermée** (grand E2E réel, 5 actes × 3
> difficultés, loot/affixes/cube/économie/quêtes/waypoints/.d2s legacy+D2R).
> Ce plan inventorie ce qui sépare encore le port d'une **fidélité complète** v1.12,
> en phases ordonnées. Sources : audits `finish-*.md`, `od2-features-*.md`,
> constats des étapes 6-13 (REPORT.md crates), gaps consignés (Fangskin (H), MonLvl 25/26).

## Rappel : hors-scope assumé (inchangé)
Multi-joueurs TCP/UDP, battle.net, cinématiques Bink, party/trade en ligne,
graphismes 3D D2R. Une fidélité « solo complète » est visée ici.

---

## Phase 14 — Audio & ambiance (M, ~2-3 j)
Aujourd'hui : 13 sfx globaux, zéro musique.
- [ ] Musiques par écran : menu, camp A1, et par acte (boucles sans couture, fondu)
- [ ] `SoundEnviron.txt` (50) : ambiance par région (jungle A3, lave A4, glace A5)
- [ ] `monsounds.txt` (141) : sons par monstre (attaque/mort/annonce superunique)
- [ ] SFX skills/missiles + events moteur (on-cast, on-hit, shrine, level-up)
- [ ] Volume/panning stéréo par position (panoramique écran)

## Phase 15 — Animations & sprites (XL, 2-3 sem)
Aujourd'hui : 7 héros × 4-5 modes, ~10 monstres, 6 tuiles d'exemple par acte.
- [ ] Extraction DCC massive : les 735 monstats de `monstats2.txt` (sprites par
      acte, incl. boss/vagues/villains) via le pipeline étape 1 (`shortcut-sprites-perso-d2`)
- [ ] Modes héros manquants : SC (cast), BL (bloc), GH (touché), S1-S4 (spells),
      KK (kick), TH (two-hand), 2HS — + modes armes par weapon class COF
- [ ] Overlays d'états : `overlay.txt` (294) / `states.txt` — auras visibles,
      poison (vert), froid (bleu), freeze, transformation, buffs shrines
- [ ] Missiles visuels : sprites des missiles réels (684 ids — extraire les DC6
      `missile` utilisés par les skills), trails
- [ ] Vitesse d'animation : `animdata` déjà branché — vérifier par mode/monstre
      (frames per direction, HDR)

## Phase 16 — Comportements des 210 skills (XL, 3-4 sem)
Aujourd'hui : dégâts/missiles/synergies CalcString + quelques comportements.
- [ ] Auras Paladin runtime (Defiance, Holy Fire… auras on/off, rayon, tick)
- [ ] Invocations : squelettes/golems (Nécro), loups/chêne/esprits (Druide) —
      IA compagnon + limites par niveau + `summod`
- [ ] Transformations : Loup/Ours Druide, Vampire Nihlathak (change form + stats)
- [ ] Traps Assassine (posables, déclenchées), charges Combos, finishers
- [ ] Paladin : charges (Vigueur…), conversions ; Nécro : malédictions complets
      (les 12, durée/resistance réduction)
- [ ] États persistants : poison stacking rules, freeze/Chill, stun, knockback
- [ ] Proc on-struck/on-levelup/on-kill (`item_props` charges déjà posés)

## Phase 17 — Items : chaîne finale (L, ~1 sem)
Aujourd'hui : qualités/affixes/uniques/sets/props → stats, cube 145 recettes.
- [ ] Runewords runtime : table extraite (169) — résolution sockets + runes
      ordonnés → bonus RW, nom RW affiché
- [ ] Socketing réel : joaillerie (gemmes par type de socket, bonus par socket),
      jewel dans unique/socketed, retrait (Horadric Cube)
- [ ] Items crafted (recettes cube + jewel : affixes fixes + roulés ilvl)
- [ ] Éthéré (durabilité ÷2 non réparable, +50 % def/dmg), indestructible,
      personnalisé (`Personalized`), ear, charges d'usage (tomes déjà faits)
- [ ] Gemmes upgrade via cube déjà couvert — brancher les bonus socket

## Phase 18 — UI fidèle (L, ~1-1,5 sem)
Aujourd'hui : DOM/canvas fillText (décision EXP-T1/T2 assumée), panneaux MVP.
- [ ] Arbres de compétences : fonds DC6 réels (`skltree_d_back/i_back` extraits)
      + icônes par classe (DC6 extraits, branchés) + tooltips descriptifs
      (`skilldesc` + `stringfunctions` : dégâts calculés affichés)
- [ ] Panneau personnage complet : résists 4, MF/GF, XP bar, `defense`, blocage
- [ ] Mini-panneau (Q), panneau quêtes par acte avec visuels
- [ ] Cube visuel 3×4 (transmutation au clic), UI gamble Gheed, screenportail
- [ ] Ceinture visuelle 4×4 avec icônes potions (DC6 items extraits)

## Phase 19 — Interop .d2s totale (M, ~3-5 j)
Aujourd'hui : lecture/écriture legacy 0x60 + lecture D2R 0x61 validées (Javazon).
- [ ] Écriture D2R 0x61 bit-exacte (Huffman inverse, D2QuestDataStrc complète)
- [ ] Fichier mercenaire séparé (.d2s merc), flags hardcore/expansion
- [ ] Round-trip vs un 2e save retail (Etal/autre perso) + golden bits
- [ ] Import export depuis le menu (liste de slots multi-persos)

## Phase 20 — DRLG procédural exact (XL, optionnel, 3+ sem)
Aujourd'hui : Blood Moor synthétique (36 presets réels) + maze/preset/wilderness
seedés maison ; les layouts ne sont PAS ceux du retail à grain égal.
- [ ] Port de l'algorithme DRLG Blizzard (grilles Vis/Warp/LvlSub + presets
      `Def`/`Dt1Mask`, règles de placement) pour layouts identiques au retail
- [ ] Zones spéciales : Cave/Nest/Pit variants, Arkane (layouts verticaux),
      Worldstone (arène Baal), Tristram (preset figé)
- [ ] Vaches secrètes (Wirt's leg + tome → niveau vaches) — contenu présent

## Phase 21 — Divers fidélité (S-M, en continu)
- [ ] Fangskin (H) : trou de la table retail (`TreasureClassEx` v1.12) — garde
      `has_tc` en place ; documenter comme écart source, pas port
- [ ] Commentaires stale MonLvl 25/26 (quests_actes2/finale) — code OK
- [ ] Andariel : API stats d2rules (aujourd'hui data-driven d2game)
- [ ] `AutoMap.txt` Cel1-4 : rendu précis des segments si mapping DT1 propre
- [ ] Ré-audit complet des 122 features vs SYNTHESE.md (baseline mise à jour)

---

## Ordre recommandé
1. **15 (sprites/animations)** — impact visuel immédiat, outillage déjà en place
2. **16 (skills)** — le cœur du gameplay fidèle
3. **17 puis 18** — items + UI (peuvent chevaucher 15/16 sur arbres séparés)
4. **14 (audio)** — indépendant, peut tourner en parallèle partout
5. **19 (interop .d2s)** — après un 2e save retail fourni
6. **20 (DRLG exact)** — la seule pièce « recherche » restante, optionnelle

## Critère de « fidélité complète » (DoD)
Solo, hors multi/cinématiques : 7 classes × 210 skills comportés, 735 monstres
animés avec sons, 135 niveaux + zones secrètes, loot complet (runewords inclus),
audio par acte, UI fidèle, saves interopérables avec le retail — tout verrouillé
par des golden tests vs tables v1.12, workspace vert.
