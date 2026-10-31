//! src/ui/text/ — the CPU half of UI text (#419).
//!
//! Everything about a `Text` that is not pixels: font assets ([`font`]) and their
//! outlines (`outline`), the rich-text subset ([`rich`]), shaping (`shape`),
//! measuring (`measure`), wrapping and overflow
//! (`lines`) and the final placement with alignment and auto-size ([`layout`]).
//! Pure and GPU-free, so headless layout — and the preferred sizes layout groups
//! (#421) read — is exactly the window's. The SDF atlas and the drawing live in
//! `render::ui::text`.

#[cfg(test)]
mod cluster_tests;
pub mod font;
pub mod layout;
mod lines;
mod measure;
mod outline;
pub mod rich;
mod shape;
#[cfg(test)]
mod shaping_tests;

pub use layout::{layout_text, measure_string, preferred_size, PlacedGlyph, TextLayout};
