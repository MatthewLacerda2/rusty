//! Every field of `Scene` classified once: its class says what each lifecycle owes it.
//! Test hooks, compiled in every build: the edit side's `base_tests::state` checks them.

#[doc(hidden)]
pub mod class;
#[doc(hidden)]
pub mod inventory;
#[cfg(test)]
mod keyed_tests;
#[doc(hidden)]
pub mod marks;
#[cfg(test)]
mod renumber_tests;

#[doc(hidden)]
pub use class::{promise, Class, Field, Keyed, Lifecycle, Outcome};
