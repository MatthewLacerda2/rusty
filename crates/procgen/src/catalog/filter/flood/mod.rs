//! The readers relabel `flood_fill`'s islands from its alpha: islands never touch, so the
//! alpha gives back the same islands at any float precision.

// sorted: one module per op; the readers share only `ops::flood`
pub mod bbox;
pub mod fill;
pub mod gradient;
pub mod random;
#[cfg(test)]
mod tests;
