# Implémentation — compétences de départ (Phase 2, priorité n°1)

> Membre : d2rules. Périmètre respecté : **`engine-rs/crates/d2rules/` uniquement**
> (d2game/d2dcc intacts — game.rs jamais modifié, voir §Action requise).
> Audit de référence : `od2-features-gameplay.md` (A6/L6/G7), d2core
> `d2hero/hero_skill.go` + `hero_state_factory.go:111-148`.

## Livré

### `d2rules/src/skills.rs` (nouveau)
- `HeroSkill { code, level, missile: Option<String> }` (serde) + `is_melee()`.
- Skills de départ réels `patch_skills.txt` v1.12 :
  - **SOR** : `Fire Bolt` niv 1 → `srvmissile=firebolt` (Id 36, EType fire).
  - **BA** : `Bash` niv 1 — mêlée `h2h`, aucun missile (Id 126).
  - **PA** : `Sacrifice` niv 1 — mêlée `h2h`, aucun missile (Id 96).
  - Classe hors table ⇒ liste vide (jamais de panic).
- `starting_skills(code)` + `default_active(code)` : sélection par défaut
  SOR→Fire Bolt (tire firebolt), BA→Bash, PA→Sacrifice (mêlée).
- Les dégâts/élément ne sont PAS dupliqués ici : ils restent dans la table
  `d2missiles` (`dmgSrc=Skill`, firebolt 3–8 feu) — frontière de crates.

### Câblage `world.rs`
- `HeroData` + `skills: Vec<HeroSkill>`, `active_skill: usize`.
- `hero_ranged_attack` : décoche le missile du skill actif — carrier `Skill`
  via `MissileSystem::spawn` (dégâts/élément officiels de la table) ; repli
  `arrow` (dégâts d'arme ×force, `spawn_weapon`) si skill mêlée ou absent.
  Comportement BA inchangé (non-régression des tests flèches).
- API additive : `hero_skills()`, `active_skill()`, `select_skill(index)`.

### UI (sans toucher d2game)
- `HeroStats` + `skill: String`, `skill_level: u32` (`#[serde(default)]`) —
  transitent déjà par `hero_stats_json`/`hero_stats` de d2game → la page voit
  le skill actif sans changement côté d2game.

### Sauvegarde `save.rs` — **version 2**
- `Save` + `skills: Option<Vec<HeroSkill>>`, `active_skill: Option<usize>`.
- Rétro-compat : charge v1 **et** v2 ; sans champ `skills` ⇒ défauts de la
  classe ; `active_skill` hors bornes ⇒ défaut. Contrats intact : RNG monde +
  RNG missiles sérialisés, round-trip bit-exact (tests existants verts).

## Tests (`cargo test -p d2rules` : 44 passed, +5 nouveaux)
- `sorceress_starts_with_firebolt_and_ranged_fires_it` — état de départ,
  HUD stats, missile `def_id=firebolt`, élément Fire, dégâts 3–8 (pas les
  dégâts d'arme).
- `melee_classes_default_melee_and_fallback_arrow` — BA/PA mêlée, repli
  `arrow` physique ; classe inconnue ⇒ aucun skill, aucun panic.
- `select_skill_switches_active_and_stats` — index hors bornes refusé.
- `skills_survive_save_load_and_v1_saves_load_with_defaults` — v2 round-trip,
  v2 sans champ skills, **v1 réelle**, index corrompu.
- `sor_firebolt_vs_barbarian_arrow_diverge_on_seed` — même graine ⇒ mêmes
  jets de toucher (déterminisme préservé, seule la nature du projectile change).

## ⚠ Action requise côté d2game (fichier verrouillé — PAS appliqué)

`game.rs:969` `patch_save_for_level` teste `version != Some(1)` en dur ⇒
depuis le passage en v2, tout transfert de héros entre niveaux retombe sur un
monde neuf (stats/inventaire perdus) et 2 tests d2game échouent
(`level_switch_carries_hero_stats_and_inventory`,
`switch_to_bloodmoor_reloads_and_spawns`). Correctif validé chez moi
(28/28 d2game au moment du test) puis **annulé** (fichier interdit, un membre
y travaille — compose.rs en pleine refonte overlay à 21:53) :

```rust
// game.rs:969 — accepter v1 (anciennes pages) et v2 :
if !matches!(v.get("version").and_then(|x| x.as_u64()), Some(1) | Some(2)) {
```

## Suite (hors périmètre de ce brief)
- Coût mana des sorts, bonus dégâts Bash/Sacrifice, anim SC, icônes/arbre.
- icebolt/chargedbolt/magicarrow déjà dans la table d2missiles : il suffit
  d'étendre `starting_skills` + `select_skill`.
