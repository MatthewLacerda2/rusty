//! src/ui/text/ — the CPU half of UI text (#419).
//!
//! Everything about a `Text` that is not pixels: font assets ([`font`]), the
//! rich-text subset ([`rich`]), measuring (`measure`), wrapping and overflow
//! (`lines`) and the final placement with alignment and auto-size ([`layout`]).
//! Pure and GPU-free, so headless layout — and the preferred sizes layout groups
//! (#421) read — is exactly the window's. The SDF atlas and the drawing live in
//! `render::ui::text`.

pub mod font;
pub mod layout;
mod lines;
mod measure;
pub mod rich;

pub use layout::{layout_text, preferred_size, PlacedGlyph, TextLayout};
