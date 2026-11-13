//! The lens and film effects' ops (#1439): Vignette, Chromatic Aberration and Film
//! Grain, each with its `active` flag and Unity's parameters, clamped to the ranges
//! Unity's volume overrides take.

// sorted: one module per lens effect; none reads another
pub mod chromatic_aberration;
pub mod film_grain;
pub mod vignette;

#[cfg(test)]
mod tests;
