//! src/procgen/catalog/filter/flood/mod.rs — the flood-fill ops (#1741), Substance
//! Designer's *Flood Fill* family: `flood_fill` finds a mask's islands, and the
//! readers give each island its own value. Their math is
//! [`crate::procgen::ops::flood`].
//!
//! `flood_fill`'s output travels between nodes as an ordinary image: alpha 1 on
//! an island and 0 off it, which the readers relabel (islands never touch, so the
//! alpha gives back the same islands, at any float precision), and a colour per
//! island in RGB that only a preview reads.

//!
//! A file is named for its op without the folder's `flood_` (`fill.rs` holds
//! `flood_fill`), as the `prefix-group` rule asks.

// sorted: one module per op; the readers share only `ops::flood`
pub mod bbox;
pub mod fill;
pub mod gradient;
pub mod random;
#[cfg(test)]
mod tests;
