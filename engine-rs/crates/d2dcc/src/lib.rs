//! d2dcc — Diablo 2 DCC (sprite animation) and COF (composite order)
//! decoders.
//!
//! Byte-faithful Rust port of the OpenDiablo2 `d2dcc`/`d2cof` Go packages
//! (reference snapshot in `/tmp/opencode/dcc-spec`). No file I/O — the
//! crate builds unchanged for `wasm32-unknown-unknown`.
//!
//! ```no_run
//! use d2dcc::{Dcc, Cof};
//!
//! let bytes = std::fs::read("BAWL1HSTR.dcc").unwrap();
//! let dcc = Dcc::decode(&bytes).unwrap();
//! for frame in &dcc.directions[0].frames {
//!     // 8-bit palette indices laid out in the direction box
//!     let _ = &frame.pixel_data;
//! }
//! let cof = Cof::parse(&std::fs::read("BAWL1HS.COF").unwrap()).unwrap();
//! let order = cof.draw_order(0, 0); // composite draw order, frame 0
//! ```

mod bitmuncher;
mod cell;
mod cof;
mod dcc;
mod dir_lookup;
mod direction;
mod error;
mod frame;
mod pixel_buffer;
pub mod render;

pub use cell::DccCell;
pub use cof::{Cof, CofLayer, CompositeType};
pub use dcc::{Dcc, DCC_SIGNATURE};
pub use dir_lookup::dir64_to_dcc;
pub use direction::DccDirection;
pub use error::DccError;
pub use frame::DccDirectionFrame;
pub use pixel_buffer::PixelBufferEntry;
