//! src/procgen/catalog/filter/blurs/mod.rs — the filters that smear an image along
//! a kernel, or keep its difference from one.

// sorted: one module per op; none reads another
pub mod blur;
pub mod directional_blur;
pub mod highpass;
pub mod sharpen;
pub mod slope_blur;
