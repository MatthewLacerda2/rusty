//! The lens and film effects (#1439): chromatic aberration on the HDR image before the
//! grade, the vignette and film grain on the graded image after it, Unity URP's
//! uber-pass formulas (`engine/shaders/postfx/effects/`).

// sorted: one module per lens effect; none reads another
pub(crate) mod chromatic_aberration;
pub(crate) mod film_grain;
pub(crate) mod vignette;
