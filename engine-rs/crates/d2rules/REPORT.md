# Rapport d2rules — règles de jeu pures (MVP Phase 2)

Date : 2026-10-22 · Rust 1.99.1 · edition 2021 · **28 tests `cargo test` verts, 0 warning clippy, `wasm32-unknown-unknown` ✅**
Zéro rendu, zéro IO, zéro dépendance wasm : `serde`/`serde_json` seuls (purs). Espace : `crates/d2rules/`.

## 1. API livrée (contrat d2game respecté + extras additifs)

### Contrat exact (signatures inchangées)

```rust
World::new(level_collision: Vec<Vec<bool>>, manifest: serde_json::Value) -> World
World::with_seed(grille, manifest, seed: u64) -> World      // + graine (tests/replay)
World::tick(dt_ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent>
World::hero() -> &Entity · monsters() -> &[Entity] · items() -> &[ItemOnGround]
World::hero_stats() -> &HeroStats
World::save() -> String · World::load(&str) -> Result<World, ()>
PlayerInput { MoveTo{x,y}, Attack{x,y}, Pickup, UseItem(usize) }
GameEvent { HeroMoved, HeroAttacked, MonsterDied, ItemDropped, HeroLeveled, HeroDied, … }
Entity { x, y, facing, anim: AnimState, anim_frame } · AnimState { NU, WL, RN, A1, DD }
ItemOnGround { def, x, y } · ItemDef { code, name, kind, amount } · ItemKind { Potion, Gold, Weapon, Armor }
```

### Extras additifs (au-delà du contrat — tout est documenté dans les doc-comments)

| Élément | Rôle |
|---|---|
| `GameEvent::{HeroMissed, HeroHit, HeroRespawned, GoldGained, HeroPickedUp, PotionUsed, InventoryFull}` | événements fins pour HUD/son (additifs, rien retiré) |
| `ItemDef.amount` (+ `Default`, `ItemDef::gold`/`potion`) | pile d'or au sol ; **construire les littéraux avec `..Default::default()`** |
| `HeroStats` (level, xp, xp_to_next, life/max, mana/max, str, dex, defense, to_hit, gold, walk_speed, potion_heal) | lecture HUD |
| `World::{inventory(), spawn_monster(code,x,y), clear_monsters(), set_hero_spawn(), is_walkable(), width/height(), elapsed_ms(), hero_spawn(), hero_is_moving()}` | placement fin + introspection (d2game place ses monstres s'il le veut) |
| `World::with_seed` | déterminisme : même graine ⇒ même partie (testé tick par tick) |

## 2. Décisions gameplay (chacune citée depuis les tables)

1. **Grille de collision** : `grid[y][x] == true` ⇒ bloquée ; hors-bornes ⇒ bloqué. Centre de tuile
   `(tx, ty)` = `(tx + 0.5, ty + 0.5)` ; `Entity.x/y` en espace continu, `HeroMoved.path_end` = `floor`.
2. **A\* 8 directions**, coût 10/14, heuristique octile, **coupe de coin interdite** (les deux
   orthogonales d'une diagonale doivent être libres). Cible bloquée ⇒ snap sur la tuile marchable
   la plus proche (anneaux ≤ 4). Cible injoignable ⇒ input ignoré (aucun événement).
3. **Vitesses** : 1 tuile = 1 yard ⇒ héros `walkSpeed` = 6 tuiles/s ; monstres `Velocity` = 5/1/3
   (FA/ZM/SI). `dt` tronqué à 100 ms (anti-tunneling). Course RN réservée (pas d'endurance au MVP).
4. **Scaling MonLvl (GAME-DATA §2.1)** appliqué aux valeurs brutes du manifest, ligne niveau 1
   Normal : HP = rand(minHP..maxHP) × 7/100 (min 1) ⇒ Fallen 1–4 PV ; dégâts = rand × 2/100 ⇒
   FA/SI 1–2, ZM 1–3 ; XP = Exp × 30/100 ⇒ **FA 18, ZM 33, SI 21** ; **AC effectif 6, TH 8**
   (constantes `MONLVL_*` citées, prêtes à recevoir d'autres lignes de MonLvl).
5. **Combat** (formules GAME-DATA §3) : `chance% = 200×TH/(TH+AC)` bornée [5, 95].
   Héros : TH = `ToHitFactor`(20) + dex(20) = 40 → ~95 % vs AC 6. Dégâts héros =
   `floor(rand(3..6) × (1 + 30/100))` = 3–7. Défense héros par coup = dex/4 + rand(minAC..maxAC
   buckler 4..6). Swing héros = 800 ms (anim A1) ; cooldown monstre 1500 ms + grâce 600 ms à
   l'entrée en ATTACK. Blocage bouclier : non implémenté (phase 2+, doc §3).
6. **FSM monstre** : Idle (errance ±3 tuiles, timer 1,5–4,5 s) → aggro ≤ 6 tuiles → Chase
   (A* vers héros, re-path 400 ms, anim WL) → ≤ 1,5 tuile → Attack (anim A1, face au héros,
   frappes périodiques) ; **laisse > 10 tuiles** ⇒ retour Idle (évite les trains sur toute la
   carte — rayon ajouté au brief, documenté). HP ≤ 0 ⇒ anim DD 1,6 s (16 frames × 100 ms) puis
   retrait + `MonsterDied`. Héros mort ⇒ tous les monstres décrochent ; respawn 2 s après
   `HeroDied`, pleine vie, au point d'apparition, XP conservée (aucune pénalité MVP).
7. **XP/niveaux** : seuils CUMULÉS colonne Barbarian de `patch_experience.txt` lignes 1..10
   (`XP_TABLE`) : niveau 2 à 500, 3 à 1500, … 10 à 72144. Level-up : +2 vie max, +1 mana max
   (`LifePerLevel`/`ManaPerLevel` en quarts ÷ 4, manifest), soin = gain seulement (pas de full
   heal, comme D2). Plafond MVP 10 ; `xp_to_next` = restants (HUD), 0 au plafond.
8. **Drops MVP** (brief) : 60 % rien / 25 % or rand(5..15) ≈ niveau×10 / 15 % potion `hp1`
   (soin 60, `patch_misc.txt`). Un seul roll `rng.below(100)` — remplacer `roll_drop` par
   TreasureClassEx (`Act 1 H2H A` : 100/21/16/21/2 déjà documentés GAME-DATA §5) en phase 3.
9. **Inventaire** : grille 10×4 = 40 slots `Option<ItemDef>`, index rangée par rangée ;
   `Pickup` = objet le plus proche ≤ 2 tuiles ; or → compteur `gold` (jamais en grille) ;
   inventaire plein ⇒ `InventoryFull`, l'objet **reste au sol**. `UseItem` potion ⇒
   `heal = min(60, max − vie)` exact dans l'événement `PotionUsed`.
10. **Spawn automatique** (le manifest ne porte pas de positions) : héros + un monstre par
    entrée `monsters`, sur des tuiles marchables espacées (BFS depuis le centre, espacement
    ≥ 6 tuiles) — **déterministe, indépendant de la graine**. `spawn_monster`/`clear_monsters`/
    `set_hero_spawn` permettent à d2game de reprendre la main.
11. **Sauvegarde JSON versionnée** (`version: 1`) : RNG, horloge, héros (stats, inventaire,
    chemin en vol, timers), monstres (FSM, PV, timers), objets au sol, grille (RLE trivial
    '0'/'1' par rangée) **et manifest embarqué** — `load(json)` n'a pas d'autres paramètres,
    conformément au contrat. Rechargement = **bit-exact** : 300 ticks de continuation
    identiques (événements + positions) après save/load, testés.
12. **RNG** : SplitMix64 privé, état sérialisé ; modulo bias négligeable au MVP.

## 3. Tests (28 — tous déterministes, graine fixe, manifest réel embarqué en compile-time)

Déplacement : arrivée exacte + `HeroMoved`, anim WL + facing E(6), détour par passage, héros
jamais sur tuile bloquée, cible injoignable ignorée · Combat : kill Fallen (XP 18 exact),
kill Zombie (XP 33), cooldown 800 ms (spam 700 ms = 1 seul swing) · Monstres : aggro 6 /
poursuite / frappe, laisse 10 (Zombie décroché), mort + respawn · XP : 28 Fallen → niveau 2
(+2 vie, +1 mana, seuil 1500) · Drops : or (montant 5..15) ramassé, potion ramassée + bue
(soin exact min(60, manquant)), pickup hors portée no-op, inventaire plein refuse ·
Déterminisme : même graine = tick pour tick identique, graines différentes divergent ·
Save/load : roundtrip + 300 ticks identiques, JSON invalide/version fausse refusés.

## 4. Écarts et caveats (aucun bloquant MVP)

| Écart | Détail |
|---|---|
| `MonsterDied.index` | = position dans `monsters()` **au moment de l'événement** ; une mort antérieure DANS LE MÊME tick décale les indices suivants (retrait en fin d'anim DD, ordre préservé). d2game peut s'appuyer sur `xp` + coordonnées du drop. |
| `ItemDef.amount` | champ additif au contrat — les littéraux d2game doivent utiliser `..Default::default()`. |
| `load -> Result<_, ()>` | pas d'erreur typée (contrat) ; version du format + sanités (grille, gabarits, bornes). |
| Or au sol | montant dans `ItemDef.amount` ; `HeroMoved.path_end` en tuiles `floor`. |
| Hors périmètre MVP | course/endurance (RN réservé), mana (statique), blocage, régén, TreasureClassEx réel, `.d2s` réel, stats points par niveau (5/niveau non dépensables), multi-monstres par pack/champions. |
| Perf | A* par monstre ≤ 1×/400 ms sur grilles ≤ ~100×100 : négligeable ; pas de panel d'optimisation au MVP. |

## 5. Fichiers

```
crates/d2rules/
├── Cargo.toml        serde + serde_json (purs, wasm-friendly)
├── REPORT.md         ce rapport
└── src/
    ├── lib.rs        types du contrat + constantes gameplay documentées
    ├── rng.rs        SplitMix64 déterministe (+ tests)
    ├── path.rs       A* 8-dir octile, anti-coupe-de-coin, snap (+ tests)
    ├── data.rs       lecture tolérante du manifest (défauts niveau 1)
    ├── world.rs      World/tick, FSM, combat, XP, drops, inventaire
    ├── save.rs       save/load JSON versionné (grille + manifest embarqués)
    └── tests.rs      28 tests de simulation déterministe
```

Contrat vérifié : `cargo test` workspace complet vert (d2mpq/d2tool/d2dc6/d2dcc/d2wasm/**d2game**
intactes — seuls `members` du workspace et ma crate ont été touchés).

---

## Étape 11c — Économie & inventaire (2026-10-24, arbre d2w-cube)

Nouveau module PUR `src/economy.rs` (+ 1 ligne `pub mod economy;` dans `lib.rs`,
branchement additif `world.rs`/`save.rs`, panneau démo `demo/stash-ui.js`).

### Or (Basin Wiki « Gold », wiki fan officieux — règles v1.10+)
* Plafond porté = **10 000 × niveau** (`carry_cap`) ; tout gain au-delà est
  refusé, le reliquat RESTE au sol (Pickup tronqué + auto-pickup).
* Or du stash séparé : `Stash::deposit_gold`/`withdraw_gold` ; plafond stash
  par difficulté 300 000 / 500 000 / 800 000 (Normal/NM/Enfer, source fan
  citée, constante documentée).
* Achat = débit RÉEL + réserve vendeur créditée (`purchase`, miroir pur de
  `TradeSession::buy`).

### Stash 6×4
* Géométrie `inventory.txt` « Bank Page » (brief 11c) : grille partagée
  `InventoryGrid` (canFit/find_first_fit de `inventory.rs`), items riches
  `EquipItem` (tailles réelles), indices stables.
* État `World::gold_stash` + accesseurs `stash`/`stash_mut`/
  `stash_deposit_gold`/`stash_withdraw_gold` ; **save additive `gold_stash`**
  (`#[serde(default)]` — saves v3 antérieures chargent un stash vide 6×4,
  test de rétro-compat inclus).
* Démo : `demo/stash-ui.js` (window.D2Stash, miroir des plafonds/canFit,
  persistence localStorage `d2w.stash.v1`) + **1 ligne** `game.js`
  (`import("./stash-ui.js")`). Aucun endpoint wasm changé ⇒ pas de rebuild
  game-pkg.

### Durabilité (colonnes réelles `durability`/`nodurability`)
* Valeurs réelles : `durability_of` (hax 28, cap 12, stu 32, misc 0).
* Usure : arme −1/coup porté (`WEAR_PER_HIT`), pièce −1/coup reçu
  (`WEAR_PER_BLOW`) — simplification documentée (le retail use une pièce
  aléatoire : Basin « Durability »).
* Brisée (= 0) ⇒ non utilisable, plus de dégâts (`weapon_damage` /
  `item_effective_damage` renvoient (0,0), mode 2 mains respecté).
* Réparation : `floor(cost × (max − cur) / max)` — formule linéaire ASSUMÉE
  et documentée (wikis non fiables ; le brief 11c le permet), garde sur
  l'or (`repair`).
* Branchement World (usure au coup) : contrat documenté — attend
  l'intégration de l'équipement 7c (`equipment.rs` non branché sur le
  World, cf. choix J1 `corpse.rs`).

### Vente & auto-pickup
* Vente officielle D2 = **cost/4 plancher** (`sell_price` ; Basin « Shops » :
  marchands payent 25 %) — goldens sur objets réels : hp1 30⇒7, cap 64⇒16,
  stu 2385⇒596.
* Auto-pickup de l'or en zone (`PICKUP_RANGE` 2 tuiles) pendant le
  déplacement (`world_auto_pickup`, fin d'`update_hero`, MOUVEMENT requis —
  immobile : rien), sous plafond porté ; reliquat au sol.

### Tests (+9 → workspace 681 passed / 0 failed / 1 ignored)
Cap or 10 000×niveau · auto-pickup sous cap · stash or plafonds/dépôt/retrait ·
vente+achat objets réels · durabilité tables réelles (usure/bris/réparation) ·
stash canFit 6×4 (dépôt/retrait/plein/trop grand) · roundtrip save `gold_stash`
+ rétro-compat · pickup monde capé · auto-pickup en mouvement. JS : `node --check`
stash-ui.js/game.js OK.

## 14a — Sons monstres data-driven (mon_audio.rs, 2026-10-24)

### Table + résolution (monsounds.txt v1.12, 141 rec dont 1 séparateur → 140 familles)
* `family_of(token)` : jeton monstats → famille (`zombie2`→`zombie`) — chiffres
  finaux retirés, remontée des `_`, alias documentés : `ancientbarb→ancient`
  (Anciens = sons barbare), `griswold→zombieunique`, `nihlathakboss→nihlathak`,
  `hephasto→smithdemon` (col MonSound superuniques.txt), `skmage→sk_mage`
  (lignes sk_mage/necromage identiques). Familles hors table (`venomlord`,
  `baalhighpriest`, `snowyeti`, `succubuswitch`) → `None`, repli générique JS.
* `event_file(token, Attack|Hit|Death|Taunt)` : cellule brute TABLE ;
  cellule vide = pas de son en retail (squelettes sans voix d'attaque) —
  fidélité, pas d'invention.
* `PORT_CODES` (23) : codes sprites du port → famille (6 manifest + planches + 16 boss/
  super-uniques/vagues, miroir JS vérifié par test de parité).

### Fichiers réellement embarqués (couche index.json)
* Extraction RÉELLE : `tools/extract_mon_sfx.py` — sons =
  `data\global\sfx\monster\<famille>\*.wav` (d2sfx.mpq), annonces =
  `data\local\sfx\act*\…\*taunt*.wav` (d2speech.mpq), noms inventoriés via
  `data\global\excel\sounds.txt` (d2data.mpq, col FileName). NOTE : chemins
  retail `data\global\monsounds\…` ABSENTS de cette release digitale (sweep
  568 sondes sur d2sfx/d2data/d2exp/d2char/patch_d2/d2speech = 0 hit).
* Bilan : 56 familles indexées, 124 entrées sonorisées, 110 wav uniques
  (~6 Mo) pour 43 familles (incl. planches A1 goatman/foulcrow/cr_lancer) ;
  dossiers taillés (imp, overseer, baal, succubus, bloodlord,
  ancient/barbarian…) → repli générique documenté côté JS (hit.wav /
  fallen_die.wav du port + 3 replis legacy fallen/zombie/quillrat).
* Taunts réels extraits : andariel, bloodraven, countess, duriel, radament,
  summoner, smith, mephisto, izual, diablo.

### Consommation (aucun endpoint wasm modifié — zéro rebuild)
Lecteur `demo/mon-sfx.js` (autoportant, pattern music.js) : `feed(ev)` sur les
GameEvent existants — `HeroAttacked`/`MissileHit`/`MercAttacked` → HIT du
monstre cible, `HeroHit` → ATTACK du monstre frappeur, `MonsterDied` → DEATH ;
taunt à la première implication d'un `kind` ∈ super_unique/boss/champion
(monsterInfo). Panning stéréo (`monsterInfo().sx`), volume par distance
écran au héros (`heroScreen`), autoplay premier geste, anti-spam 70 ms,
suit le bouton « Son ». game.js : 1 ligne (`if (window.D2MonSfx)
window.D2MonSfx.feed(ev);`) ; game.html : 1 ligne script.

### Tests (+10 → workspace 848 passed / 0 failed / 1 ignoré)
140 familles · goldens fallen/zombie/quillrat/boss/taunts · 5 alias · familles
absentes → None · 67 classes super-uniques résolues · PORT_CODES manifest+finale
· index 56/124/110 avec RIFF/WAVE vérifié sur disque · parité JS (chaque fichier
indexé présent dans mon-sfx.js, codes mappés).

## Étape 19b — interop .d2s : récompenses oracle + gate Anciens + cap stash (2026-10-25)

Réconciliation des valeurs chiffrées avec le README de l'oracle GPL
`d2w-oracle/d2s-file-generator` (valeurs de format = FAITS, réimplémentation
propre — aucun code copié), mission étape 19b PLAN-FIDÉLITÉ.

### Vérification (1) — les journaux attribuent-ils les récompenses EXACTES ?
OUI, déjà alignés (golden `golden_interop_oracle_cited_rewards_per_difficulty`) :
+4 skill points PAR difficulté = 1 (Den of Evil A1Q1) + 1 (Radament A2Q1) +
2 (The Fallen Angel A4Q1) ; +5 stats (Lam Esen) ; +20 vie (Oiseau de Feu) ;
+10 % résists tout (Prison de glace).

### Ajouts additifs (2/3)
* **Prison de glace appliquée au héros** : nouveau champ
  `HeroStats::res_all_pct` (`#[serde(default)]` — save v3 rétro-compatible,
  saves antérieures chargent à 0) ; `World::quest_actes45_event` additionne
  +10 par difficulté (chaîne N→NM→H = +30, golden `golden_world_three_
  difficulties_chain_gates_and_resists`).
* **Gate retail « Rite de Passage »** (oracle : niveau minimal POUR FAIRE la
  quête) : `ANCIENTS_MIN_LEVELS = [20, 40, 60]` — sous le seuil de la
  difficulté, l'autel des Cieux (a5q5) n'active pas et les 3 Anciens ne
  spawnent pas. Contexte RUNTIME (`set_level_gate`) posé à chaque event par
  le World, JAMAIS sérialisé ; journal pur (tests/`from_json`) non gate.
* **Cap d'or stash = 2 500 000** (economy.rs) : la table fan-wiki
  300k/500k/800k est remplacée par le fait de format .d2s confirmé DEUX fois
  (README oracle « stashed gold 0..2 500 000 » ; vrai .d2s javazon du corpus
  d2d2s : `goldbank = 2_500_000`). Signature `stash_gold_cap(diff)` conservée
  (API game/UI inchangée).

### Tests rouges revalidés avant correction (procédure livraison)
`panels_18b_golden` (d2game) : 2 asserts `goldCap` 300k/500k = ANCIENNE table
fan-wiki non officielle → corrigés vers 2 500 000 avec justification ; le
remplissage boucle 40×10 000 (400k < 2.5M) est remplacé par un pré-remplissage
save près du cap + 1 dépôt débordant (message fr « Stash plein : plafond
2500000 » vérifié). Aucun endpoint wasm changé → pas de rebuild pkg.

### Tests (+4 → workspace 1015 passed / 0 failed / 1 ignoré, --no-fail-fast)
golden récompenses citées (SP 1+1+2=4, stats 5, vie 20, rés 10, cap 2.5M) ·
gate 20/40/60 journal + World (19/39/59 bloqués, 20/40/60 passent) · chaîne 3
difficultés (+10/+20/+30 résists) · save rétro-compatible (champ absent → 0,
round trip +10, gate jamais sérialisée).

## Étape 21a — Divers fidélité : 3 gaps 13a fermés (2026-10-25, arbre d2w-audit)

Fichiers : `src/fidelity_gaps.rs` (NOUVEAU — doc des écarts + 4 tests) ·
`src/quests_actes2.rs` (additif : `ANDARIEL` + `andariel_stats()` + const
`lvl::CATACOMBS_4`) · `src/lib.rs` (1 ligne : `pub mod fidelity_gaps;`) ·
en-tête GAPS `tests/progression_golden.rs` (6/7/8 → fermés, doc seule).

### GAPS 7 — « Fangskin (H) », trou de la TABLE retail (écart source, pas port)
`superuniques.txt:14` (col `TC(H)`) référence « Fangskin (H) », ABSENT de
TreasureClassEx v1.12 (aucune rangée « Fangskin » du tout ; golden 13a
`holes == ["Fangskin (H)"]`). La garde `has_tc` de `World::roll_drop` est
documentée dans `fidelity_gaps.rs` (écart source — Blizzard a oublié la
rangée, on n'invente PAS la donnée) et PROUVÉE bout en bout : le nom troué
fourni directement au drop ⇒ repli MVP seul ({gld, hp1} sur 40 seeds, pas de
panic) ; le chemin VIVANT (base « Act 2 Super B » → variante composée
« Act 2 (H) Super B », TCE:771) dropne de vrais objets — le trou n'est
jamais atteint.

### GAPS 6 — Andariel : API stats d2rules
`quests_actes2::andariel_stats()` — MÊME convention que `boss_stats` :
`patch_monstats.txt:158` runtime (Level 12 N, HP 2562, AC 84, Exp 916,
TC « Andariel », res [0,0,-50,50,50,80]) × `MonLvl.txt` ligne 12 (N)
(AC 72, TH 111, HP 40 %, DM 8 %, XP 140 %) → `SpawnRule` rang SuperUnique
(branchement superuniques : TC dédiée → variante retail au drop, comme les
autres boss nommés). Record statique `ANDARIEL` (code `AN`, repaire
Catacombes 4 id 37 `patch_levels.txt:39`, slot porteur a1q6). Scope
conservé : `boss_stats("andariel")` reste `None` (golden 13a inchangé).
Consommation prouvée : `spawn_monster_rule("AN", …, rule)` → 2562 × 40 %
= 1024 PV effectifs, AC 72. Le spawn acte I live reste d2game (manifest
`bosses[]`), qui dispose maintenant de la source d2rules.

### GAPS 8 — commentaires MonLvl 25/26 (quests_actes2/finale)
Constat 13a (« 98/521 », DM 18) : dans CET arbre les commentaires portent
déjà les vraies valeurs (re-vérifié ligne à ligne contre `MonLvl.txt` :
ligne 22 = 84, ligne 26 = 106/DM 17, ligne 25 = 150/254/100/16/470) — la
correction datait de la re-vérification tables du 2026-10-24, le GAPS
n'était pas à jour. Verrou non-régression `fidelity_gaps::
monlvl_25_26_comment_claims` : les valeurs COMMENTÉES doivent rester celles
que sortent `boss_stats` (Duriel 84/14, 132/221 ; Mephisto 106/17, 156/265)
et `finale_stats` (Hephasto 25 : 100/16, 150/254, XP 470) — un commentaire
qui re-mente rend son test rouge.

### Tests (+4 → workspace 1025 passed / 0 failed / 1 ignoré, avec la 21b parallèle)
`fangskin_hole_guarded_no_panic_fallback` · `fangskin_live_path_drops_real_chain` ·
`andariel_stats_golden_real_tables` · `monlvl_25_26_comment_claims` (tous dans
`src/fidelity_gaps.rs`). Aucun endpoint wasm changé → pas de rebuild pkg.
Note arbre : la passe 21b parallèle a posé un stub `fidelity_gaps.rs` le
2026-10-25 (compilation restaurée pendant l'écriture 21a) — remplacé par ce
corps complet, contrats inchangés.
