//! The lens and film effects of a post-processing volume (#1439): Vignette,
//! Chromatic Aberration and Film Grain, Unity URP/HDRP's volume overrides of the same
//! names with their parameters. Each is off until its `active` flag is set; its
//! strengths then start at a visible amount (Unity's start at `0`, which reads as
//! "broken" when an agent ticks the box and sees nothing).

// sorted: one module per lens effect; none reads another
mod chromatic_aberration;
mod film_grain;
mod vignette;

// sorted: each effect's settings, re-exported by name
pub use chromatic_aberration::ChromaticAberration;
pub use film_grain::FilmGrain;
pub use vignette::Vignette;
