//! A song key, notes written as degrees of it, and the lift a key makes
//! expressible.
//!
//! The shape follows the chord tests one level up: **a shorthand must equal
//! what it is shorthand for.** A degree must render byte-identically to the
//! note it names, and a diatonic lift to the phrase written out a step higher
//! — because the moment either stops being true, the document has quietly
//! stopped describing what is heard.

mod degrees;
mod document;
mod lifts;
mod setup;
