//! What a mark over a note does, and that every way of writing one keeps it.
//!
//! The shape follows the feel tests beside it: **an articulation is a
//! difference from the note as written**, so every assertion here is a
//! comparison between two renders and never a reading of one. What is asserted
//! is the mechanism rather than the outcome — that an accent raises the level
//! *and* the brightness, that a ghost is short and dull and early rather than
//! merely quiet, that staccato moves the gate and leaves the page alone — for
//! the reason a test of "it changed" would pass for a mark that did the wrong
//! thing.
//!
//! Four files: [`marks`] for what each mark does to the samples,
//! [`composition`] for how one meets the section, the curve and the player that
//! also have a say in the note, [`forms`] for the mark surviving every way an
//! entry can be written, and [`glide`] for the one mark whose effect is a pitch
//! and therefore has a *direction* to get wrong.

mod composition;
mod forms;
mod glide;
mod marks;
mod setup;
