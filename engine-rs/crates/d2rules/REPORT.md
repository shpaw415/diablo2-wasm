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
