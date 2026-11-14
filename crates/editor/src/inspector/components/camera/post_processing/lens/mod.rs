//! The Lens Effects sections of the Post Processing card (#1439): Chromatic
//! Aberration, Vignette and Film Grain, each a checkbox and, when on, its knobs,
//! in the order the composite runs them.

// sorted: one module per lens section; none reads another
pub(super) mod chromatic_aberration;
pub(super) mod film_grain;
pub(super) mod vignette;
