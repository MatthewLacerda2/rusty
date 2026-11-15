//! crates/procgen/src/catalog/generators/outline/ — the generators that fill an
//! outline (#2190): lettering (`text`) and SVG path data (`path`). Both are one
//! stamp centred in the tile, a coverage mask, their math in
//! [`crate::ops::outline`].

// sorted: one module per outline generator
pub mod path;
pub mod text;

#[cfg(test)]
mod tests;
