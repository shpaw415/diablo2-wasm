//! The `d2rules` gameplay contract, as seen by the rendering engine.
//!
//! The full contract is frozen by the project brief (see REPORT.md §5):
//! `World::new(level_collision, manifest)`, `tick(dt_ms, input) -> events`,
//! accessors `hero / monsters / items`, and the value types `Entity`
//! (`x, y: f32`, `facing: u8` 0–7, `anim: AnimState`, `anim_frame: u8`),
//! `PlayerInput` and `GameEvent`.
//!
//! Since crates/d2rules landed, these types are re-exported verbatim from
//! the real implementation — the `stub-world` feature and its local
//! `stub_world.rs` were deleted. `game.rs` only ever uses the contract
//! surface, so no other file needed changes beyond:
//!
//! * grid polarity: `compose.walk_rows()` yields `true` = WALKABLE, while
//!   the d2rules contract takes `true` = BLOCKED — `game.rs` inverts
//!   per-cell before `World::new`.
//! * `AnimState::Dd` (stub spelling) → `AnimState::DD` (d2rules).
//! * `World::monster_alive(i)` is an additive extra d2rules provides
//!   (false for out-of-range or DD corpses).

pub use d2rules::{AnimState, Entity, GameEvent, HeroStats, ItemDef, ItemOnGround, PlayerInput, World};

/// The real `d2rules` behind the game loop.
pub use d2rules as world_impl;
