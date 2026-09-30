//! src/render/ui/text/ — the GPU-facing half of UI text (#419).
//!
//! - `sdf` — one glyph's signed distance field, generated on the CPU.
//! - `atlas` — the per-font dynamic atlas those fields pack into (uploaded when
//!   dirty by the pass); `gpu` — their `R8Unorm` copies.
//! - `quads` — a `Text` + its rect → SDF glyph quads, laid out by the sim's own
//!   `ui::text` so what draws is exactly what the sim measured; `emit` appends
//!   them to the canvas mesh through the element's corners.
//!
//! The quads batch with the rest of the UI pass: one draw per run of glyphs from
//! the same font under the same clip. The shader (`ui.wgsl`) draws fill, outline,
//! shadow and glow from the one field.

pub(crate) mod atlas;
pub(crate) mod emit;
pub(crate) mod gpu;
pub(crate) mod quads;
pub(crate) mod sdf;

#[cfg(test)]
mod mesh_tests;
