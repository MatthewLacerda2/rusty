//! crates/ui/src/text/ — the CPU half of UI text.
//! Everything about a `Text` that is not pixels: font assets by path ([`font`];
//! parsing, outlines and shaping are `rusty_font`'s), the rich-text subset
//! ([`rich`]), measuring (`measure`), wrapping and overflow
//! (`lines`) and the final placement with alignment and auto-size ([`layout`]).
//! Pure and GPU-free, so headless layout — and the preferred sizes layout groups
//! read — is exactly the window's. The SDF atlas and the drawing live in
//! `render::ui::text`.

#[cfg(test)]
mod cluster_tests;
pub mod font;
pub mod layout;
mod lines;
mod measure;
pub mod rich;
#[cfg(test)]
mod shaping_tests;

pub use layout::{layout_text, measure_string, preferred_size, PlacedGlyph, TextLayout};
