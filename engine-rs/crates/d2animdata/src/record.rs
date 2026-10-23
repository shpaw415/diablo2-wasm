//! Record of AnimData.d2 — port of OpenDiablo2 `record.go` + `events.go`.

/// One animation record: frames per direction, speed (and thus FPS) and the
/// 144 per-frame trigger events for a single animation token (the COF file
/// stem, e.g. `BAWL1HS` = `<class><mode><weapon class>`).
///
/// Fields mirror the Go private fields; accessors are provided for the
/// derived values (`fps`, `frame_duration_ms`, `event`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationDataRecord {
    /// Token, 8-byte field NUL-stripped (max 7 chars, Go `record.name`).
    pub name: String,
    /// Go `framesPerDirection`.
    pub frames_per_direction: u32,
    /// Go `speed` — raw u16; FPS = 25 × speed ÷ 256 (record.go `FPS`).
    pub speed: u16,
    /// The 144 raw event bytes (`events.go`: 0 none, 1 attack, 2 missile,
    /// 3 sound, 4 skill). Kept raw: unknown values survive round-trips;
    /// [`AnimationDataRecord::event`] maps them to [`AnimationEvent::None`]
    /// like a modded byte would be treated by the stock events.
    pub events: [u8; NUM_EVENTS],
}

use crate::NUM_EVENTS;

/// Event that can happen on a frame of animation (events.go `AnimationEvent`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AnimationEvent {
    /// Frame without trigger (0).
    #[default]
    None = 0,
    /// Frame where a melee attack connects (1).
    Attack = 1,
    /// Frame where a missile spawns (2).
    Missile = 2,
    /// Frame where a sound plays (3).
    Sound = 3,
    /// Frame where a skill effect fires (4).
    Skill = 4,
}

impl AnimationEvent {
    pub fn from_u8(v: u8) -> Option<AnimationEvent> {
        match v {
            0 => Some(AnimationEvent::None),
            1 => Some(AnimationEvent::Attack),
            2 => Some(AnimationEvent::Missile),
            3 => Some(AnimationEvent::Sound),
            4 => Some(AnimationEvent::Skill),
            _ => None,
        }
    }
}

const SPEED_DIVISOR: f64 = 256.0; // animdata.go speedDivisor
const SPEED_BASE_FPS: f64 = 25.0; // animdata.go speedBaseFPS
const MILLISECONDS: f64 = 1000.0; // animdata.go milliseconds

impl AnimationDataRecord {
    /// Frames per second for this record (record.go `FPS`):
    /// `25 × speed ÷ 256`.
    pub fn fps(&self) -> f64 {
        SPEED_BASE_FPS * self.speed as f64 / SPEED_DIVISOR
    }

    /// Duration in milliseconds a frame is displayed (record.go
    /// `FrameDurationMS`).
    pub fn frame_duration_ms(&self) -> f64 {
        MILLISECONDS / self.fps()
    }

    /// Event at frame `idx`, [`AnimationEvent::None`] when out of range or
    /// not a known event byte (record.go `Event`).
    pub fn event(&self, idx: usize) -> AnimationEvent {
        self.events
            .get(idx)
            .copied()
            .and_then(AnimationEvent::from_u8)
            .unwrap_or(AnimationEvent::None)
    }

    /// Triggered events as `(frame index, event)` pairs, in frame order —
    /// the iterator form of record.go's `Events` map (which only stores
    /// non-none entries).
    pub fn triggered_events(&self) -> impl Iterator<Item = (usize, AnimationEvent)> + '_ {
        self.events
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| match AnimationEvent::from_u8(b) {
                Some(AnimationEvent::None) | None => None,
                Some(e) => Some((i, e)),
            })
    }
}
