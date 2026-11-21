//! A song whose tempo moves: every beat-keyed thing follows the map.
//!
//! The map itself is held to its integral in `song::clock::map`'s own tests.
//! What is checked here is the promise the clock was built for — that notes,
//! gates, section rows, windows, faders and `fit` all read the one map, so
//! none of them lands a tempo change away from the others.

mod document;
mod fitting;
mod placement;
mod setup;
