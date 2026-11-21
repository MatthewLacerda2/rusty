//! Making a song the length the picture needs.
//!
//! Every assertion here is a **sample count**, not "it rendered": the whole
//! point of `fit` is that 43 seconds is exactly 1,896,300 samples, and a test
//! that only asked whether something came out would pass on a song of the
//! wrong length.

mod anchors;
mod capped;
mod fitting;
mod levels;
mod resolving;
mod setup;
mod wrapping;
