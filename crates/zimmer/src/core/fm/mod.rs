//! frequency modulation: one sine bending another's phase.
//!
//! Two renderers, and the difference between them is how many operators are in
//! play — [`two`] is one modulator on one carrier, [`four`] is four sines wired
//! by a chosen algorithm. They share the arithmetic that makes FM what it is: a
//! sine's *phase* is offset by another sine, which multiplies one tone into a
//! spray of sidebands at `carrier ± k × modulator`, and the depth of the offset
//! (the **index**, in radians) decides how many of them there are, i.e. how
//! bright the result is.
//!
//! ## Which one a recipe should reach for
//!
//! [`two`] first, and usually. It is two numbers — a ratio and an index — which
//! is a thing a person can hold in their head, and it covers the timbres FM is
//! famous for at the percussive end: electric pianos, bells, glass, struck
//! metal. A recipe wanting a bell should not have to choose a routing to get
//! one.
//!
//! [`four`] when the sound needs something two operators cannot express, and
//! the test is structural rather than a matter of taste: **two operators can
//! only produce one modulator-carrier relationship.** A modulator that is
//! itself modulated, two unrelated modulators colouring one carrier, or two
//! complete voices layered into one note are each a second relationship, and
//! each is the difference between a bell and a horn. Brass, sustained pads,
//! most FM basses and any layered bell-into-body sound live there.
//!
//! ## Sidebands above Nyquist, in both
//!
//! FM's sidebands are not synthesised one at a time, so unlike an additive
//! series they cannot be dropped individually: the ones above half the sample
//! rate fold back into the audible band as inharmonic ringing. What a renderer
//! *can* refuse is an operator whose **own** frequency is past Nyquist, since
//! that operator's sine is already not the sine that was written. [`four`] does
//! exactly that, and its module doc carries both the reasoning and the
//! practical bound on the sidebands it cannot drop; [`two`] does not, and a
//! high `ratio` on a high note is the recipe's to keep sane.

pub(crate) mod feedback;
pub(crate) mod four;
pub(crate) mod two;
pub(crate) mod voicing;
