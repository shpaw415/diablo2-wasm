# impl-monstres-boss — contenu monstre data-driven + Andariel (W06/W07/W08 + W11 A1)

> 2026-10-23. Chantier XL de `finish-monde.md`. Livré par le membre puis
> câblage finalisé par l'intégration (le membre a atteint sa limite de steps
> en pleine passe game.rs — détail des fixes en fin de rapport).

## Livré

### `d2game/src/monsters.rs` (860 lignes) — données + plan de spawn
- **Planches par niveau** : lecture des colonnes `mon1-10` du `levels.txt`
  réel (LvlModify? planches A1 réelles : fallen/zombie/quillrat…), densités
  `MonDen`, packs `MinGrp/MaxGrp`.
- **monumod.txt (43 mods)** : mods champion/unique pondérés (`cpick`/`upick`),
  chance champion 20 %, boosts HP/dégâts/toucher (`*constant desc`).
- **SuperUniques.txt (67 records)** : boss nommés (Bishibosh, Rakanishu…) —
  leaders champion / uniques dorés nommés de la planche.
- **MonLvl.txt** : facteurs de spawn réels (HP/DM/TH % bruts) — `d2rules`
  reste pur : les facteurs sont calculés côté d2game, passés par
  [`SpawnRule`] (hp_pct/dmg_pct/th_pct, TC overridée, rang).
- **TC par rang** : `TreasureClass1/2/3` → TC effectives normal/champion/
  unique + TC boss dédiée (« Act 1 Champ A », « Andariel »).
- API : `plan_level_spawns(...)` → plan de packs déterministe au seed (Lcg
  partagé compose).

### `d2rules/src/world.rs` — rangs + spawn explicite (W06-W08)
- `MonKind` (Normal/Champion/Unique/SuperUnique/Boss, sérialisé, sauvegardé)
  + `SpawnRule` (facteurs + rang + TC) + `World::spawn_monster_rule(code, x,
  y, rule)` — l'appelant fournit les facteurs, le monde reste pur.
- Drops : TC effective par rang (champion/unique/boss) branchée sur la chaîne
  ItemRatio/affixes livrée en vague 1.

### Boss Andariel (W11 slice Acte I)
- Sprites réels extraits : `assets/dcc/AN{NU,WL,A1,DD}*.COF/.dcc` (shortcut
  sprites-perso-d2).
- Manifest : tableau `bosses` (code AN, stats monstats réelles — 2562 PV,
  AC 84, dmg 76-238, TH 153, exp 916 — TC « Andariel », sfx dédiés) +
  `questEvent: "andariel_dead"` émis à sa mort (contrat pour le moteur de
  quêtes, câblage en intégration).
- Spawn scripté en fin de Catacombs niveau 4 ; mort → événement + drop TC
  boss réel.

### Câblage rendu (finalisé par l'intégration)
- `game.rs` : spawn des packs par plan au chargement de niveau (fallback
  historique conservé si tables absentes), `mon_kinds` par entité, teintes
  `monster_tint()` — champion bleu, unique/super-unique doré, boss rouge
  (multiplication RGB simple, simplification documentée des transforms
  palette D2), `blit_sprite`/`blit_entity` + paramètre tint.

## Tests

d2game lib : **68 passed / 0 failed** (9 tests monsters.rs : planches A1
réelles, packs, rangs, super-uniques, TC par rang ; + suite complète non
régressive). Workspace final : **274 passed / 0 failed / 1 ignored**.

## Fixes d'intégration (game.rs à moitié câblé au handoff du membre)

1. Re-export `MonKind, SpawnRule` manquant dans `d2game/src/world.rs`.
2. `blit_entity`/`blit_sprite` : signature tint (6e argument) + application
   RGB ; `monster_tint()` défini (manquant).
3. Move `rule` après `spawn_monster_rule` (E0382) → `rule_kind` capturé avant.
4. Test `hero_modes_built_from_named_blocks` : fournir aussi les assets du
   tableau `bosses` (Andariel non fourni → notes parasites).

## Restant (W11 suite)

- Duriel → Mephisto → Diablo → **Baal** (trône + vagues) : même gabarit que
  Andariel (records monstats + sprites + TC + arène).
- Teintes palette réelles (palshift.dat) au lieu de la multiplication RGB.
- Placement `umon` exact des super-uniques (positions du DS1).
