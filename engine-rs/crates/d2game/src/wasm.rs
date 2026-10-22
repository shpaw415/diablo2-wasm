//! wasm-bindgen glue — the `D2Game` façade the browser talks to.
//!
//! Built exactly like `crates/d2wasm` (wasm-bindgen pinned 0.2.129):
//! `cargo build --release --target wasm32-unknown-unknown -p d2game` then
//! `wasm-bindgen --target web --out-dir ../../demo/game-pkg target/...`.
//!
//! Strings cross the boundary as JSON (`tick` input / events output), so
//! the JS side never needs to know the Rust contract types.

use wasm_bindgen::prelude::*;

use crate::game::GameCore;
use crate::world::{GameEvent, PlayerInput};

#[wasm_bindgen]
pub struct D2Game {
    core: GameCore,
}

#[wasm_bindgen]
impl D2Game {
    /// `manifest_json` — the raw `assets/manifest.json` text. `hero` — the
    /// selected class code (`"BA"`, `"SOR"`, `"PA"`…, case-insensitive, default
    /// `"BA"`): the hero's COF/DCC layers AND stats both come from
    /// `manifest.heroes[code]` when present, else the legacy `hero` block.
    #[wasm_bindgen(constructor)]
    pub fn new(manifest_json: &str, hero: &str) -> Result<D2Game, JsValue> {
        console_error_panic_lite();
        Ok(D2Game { core: GameCore::new(manifest_json, hero).map_err(js_err)? })
    }

    /// Resolved hero class as JSON `{"code":"BA","name":"Barbare"}` — HUD
    /// display + E2E probe. Valid before `finalize` too.
    #[wasm_bindgen(js_name = heroClass)]
    pub fn hero_class(&self) -> String {
        self.core.hero_class_json()
    }

    /// Feed one asset blob (manifest path + bytes), e.g. fetched files.
    pub fn provide(&mut self, path: &str, bytes: &[u8]) {
        self.core.provide(path, bytes.to_vec());
    }

    /// Compose the map + build sprites + spawn the world. Call once.
    pub fn finalize(&mut self) -> Result<(), JsValue> {
        self.core.finalize().map_err(js_err)
    }

    #[wasm_bindgen(js_name = setViewport)]
    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.core.set_viewport(w, h);
    }

    /// Hero HUD stats (`HeroStats`) as a JSON object string — level, life,
    /// mana, xp, gold… polled by the demo HUD each frame.
    #[wasm_bindgen(js_name = heroStats)]
    pub fn hero_stats(&self) -> String {
        self.core.hero_stats_json()
    }

    /// Inventory (40 slots, `null` = empty) as a JSON array string.
    #[wasm_bindgen(js_name = inventory)]
    pub fn inventory(&self) -> String {
        self.core.inventory_json()
    }

    /// Hero animation state as JSON `{"anim":"RN","frame":3,"facing":6,
    /// "run":true,"moving":false}` — HUD run flag + E2E probes.
    #[wasm_bindgen(js_name = heroState)]
    pub fn hero_state(&self) -> String {
        self.core.hero_state_json()
    }

    /// Animation modes listed in the manifest but skipped (missing files),
    /// as a JSON array of strings — the page logs them, the game still runs.
    #[wasm_bindgen(js_name = spriteNotes)]
    pub fn sprite_notes(&self) -> String {
        self.core.sprite_notes_json()
    }

    /// Serialize the world (JSON v1) — demo Save button → localStorage.
    #[wasm_bindgen(js_name = saveGame)]
    pub fn save_game(&self) -> String {
        self.core.save_world()
    }

    /// Restore a world from `saveGame` JSON. Throws (`JsValue`) on invalid
    /// save or a save from another map. Map/sprites are untouched.
    #[wasm_bindgen(js_name = loadGame)]
    pub fn load_game(&mut self, json: &str) -> Result<(), JsValue> {
        self.core.load_world(json).map_err(js_err)
    }

    /// Advance the simulation. `input_json` (nullable) mirrors the
    /// `PlayerInput` contract:
    /// `{"type":"move","x":123,"y":45}` · `{"type":"attack","x":..,"y":..}`
    /// · `{"type":"pickup"}` · `{"type":"setRun","on":true}`
    /// · `{"type":"useItem","slot":0}`.
    /// Returns the tick's `GameEvent`s as a JSON array string.
    pub fn tick(&mut self, dt_ms: u32, input_json: Option<String>) -> String {
        let input = input_json.as_deref().and_then(parse_input);
        let events = self.core.tick(dt_ms, input);
        serde_json::to_string(&events).unwrap_or_else(|_| "[]".into())
    }

    /// RGBA framebuffer (`viewport_width * viewport_height * 4`) for
    /// `putImageData` — floor, shadows, entities already composited.
    pub fn render(&mut self) -> Vec<u8> {
        self.core.render().to_vec()
    }

    #[wasm_bindgen(js_name = viewportWidth)]
    pub fn viewport_width(&self) -> u32 {
        800 // kept in sync with game.rs default; see also set_viewport
    }

    #[wasm_bindgen(js_name = viewportHeight)]
    pub fn viewport_height(&self) -> u32 {
        600
    }

    /// Hero feet position in viewport px — for the JS click ring / cursor.
    #[wasm_bindgen(js_name = heroScreen)]
    pub fn hero_screen(&self) -> Vec<f32> {
        let (x, y) = self.core.hero_screen();
        vec![x, y]
    }

    #[wasm_bindgen(js_name = mapPixelSize)]
    pub fn map_pixel_size(&self) -> Vec<u32> {
        let (w, h) = self.core.map_pixel_size();
        vec![w, h]
    }

    /// Collision grid as 0/1 bytes (row-major), for d2rules/debug overlays.
    #[wasm_bindgen(js_name = collisionGrid)]
    pub fn collision_grid(&self) -> Vec<u8> {
        self.core.collision()
    }

    #[wasm_bindgen(js_name = gridSize)]
    pub fn grid_size(&self) -> Vec<u32> {
        let (w, h) = self.core.grid_size();
        vec![w, h]
    }

    /// Alive monster index nearest to a world point within `radius`
    /// sub-tiles, or -1 (JS click router: attack vs move).
    #[wasm_bindgen(js_name = monsterNear)]
    pub fn monster_near(&self, wx: f32, wy: f32, radius: f32) -> i32 {
        self.core.monster_near(wx, wy, radius).map(|i| i as i32).unwrap_or(-1)
    }

    /// Click (viewport px) → world sub-tile coords `[x, y]`.
    #[wasm_bindgen(js_name = screenToWorld)]
    pub fn screen_to_world(&self, px: f32, py: f32) -> Vec<f32> {
        let (x, y) = self.core.screen_to_world(px, py);
        vec![x, y]
    }

    /// Compositor stats — surfaced in the demo status line.
    #[wasm_bindgen(js_name = composeStats)]
    pub fn compose_stats(&self) -> Vec<u32> {
        let s = self.core.compose_stats();
        vec![
            s.drawn as u32,
            s.hidden as u32,
            s.empty as u32,
            s.out_of_bounds as u32,
        ]
    }
}

fn parse_input(json: &str) -> Option<PlayerInput> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let x = |v: &serde_json::Value| v.get("x").and_then(|n| n.as_i64()).unwrap_or(0) as i32;
    let y = |v: &serde_json::Value| v.get("y").and_then(|n| n.as_i64()).unwrap_or(0) as i32;
    match v.get("type").and_then(|t| t.as_str())? {
        "move" => Some(PlayerInput::MoveTo { x: x(&v), y: y(&v) }),
        "attack" => Some(PlayerInput::Attack { x: x(&v), y: y(&v) }),
        "pickup" => Some(PlayerInput::Pickup),
        "setRun" => Some(PlayerInput::SetRun(
            v.get("on").and_then(|b| b.as_bool()).unwrap_or(false),
        )),
        "useItem" => Some(PlayerInput::UseItem(
            v.get("slot").and_then(|n| n.as_u64()).unwrap_or(0) as usize,
        )),
        _ => None,
    }
}

/// Events serialize via serde (stub world derives it; the real d2rules
/// must too — noted in world.rs).
#[allow(dead_code)]
fn _events_are_serializable(e: &[GameEvent]) -> String {
    serde_json::to_string(e).unwrap_or_else(|_| "[]".into())
}

fn js_err(msg: String) -> JsValue {
    JsValue::from_str(&msg)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

/// Panic hook without the `console_error_panic_hook` dep: log via
/// `console.error` so failures are visible in the page, never silent.
fn console_error_panic_lite() {
    std::panic::set_hook(Box::new(|info| {
        error(&format!("d2game panic: {info}"));
    }));
}
