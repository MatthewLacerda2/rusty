//! fonts as data: a parsed `.ttf` / `.otf` face, its glyphs'
//! flattened outlines and shaped runs.
//!
//! The font-only half of text, below everything that draws one: the UI's `Text`
//! (`ui::text`, which adds layout, rich text and the path cache) and Snyder's `text`
//! op (`procgen`, which fills the outlines into a mask) both name it, so the two
//! read the same glyphs, kerning and ligatures. Pure and file-free: a caller reads
//! the bytes, and every answer is a function of them.

#![deny(clippy::unwrap_used)]

mod face;
mod outline;
mod shape;

pub use face::{default_font, FontData, FontHandle, GlyphId};
pub use outline::Segment;
pub use shape::{shape, Shaped};
