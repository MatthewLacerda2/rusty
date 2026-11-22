//! Each effect is off until its `active` flag is set, then its strengths start at a visible amount
//! (Unity's start at `0`, which reads as broken when an agent ticks the box and sees nothing).

// sorted: one module per lens effect; none reads another
mod chromatic_aberration;
mod film_grain;
mod vignette;

// sorted: each effect's settings, re-exported by name
pub use chromatic_aberration::ChromaticAberration;
pub use film_grain::FilmGrain;
pub use vignette::Vignette;
