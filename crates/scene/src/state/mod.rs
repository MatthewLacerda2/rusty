//! crates/scene/src/state/ — the one inventory of the scene's state (#1649).
//!
//! Several lifecycles copy, put back or clear "the scene's state": a save and its
//! load, Play → Stop (`snapshot`), Undo's scene level (`undo::level`), a demo
//! replay closing over the edit scene (`dev::demo::Origin`, which is Stop's
//! snapshot), and a load over a live scene. Each used to keep its own list of what
//! that state is, and nothing tied the lists together, so every new field had to
//! be added to every lifecycle by hand and a missed one surfaced as a bug in
//! review (#1636, #1640, #1645).
//!
//! Here every field of `Scene` is classified once, as one `Class`, and the
//! class says what each `Lifecycle` owes it (`Class::owes`). The list is
//! `inventory`, and Undo's scene level is generated from its authored part:
//!
//! - **A new field can't skip it.** The inventory names `Scene`'s fields in an
//!   exhaustive pattern, so a field added to `Scene` and not here fails to compile.
//! - **Undo reads it.** The `Authored` fields are listed once, with Undo's hooks,
//!   and `undo::level` is expanded from that list.
//! - **The other lists are checked against it.** `SceneData`'s fields are exactly
//!   the saved classes' fields, in the edit side's `base_tests::state::names`.
//! - **Their behaviour is checked against it.** The edit side's
//!   `base_tests::state::lifecycle` sets every field (each has a mark in `marks`,
//!   and a field with none fails), runs save → load, Play → Stop, edit → Undo, an
//!   edit-mode load over it and a Play-mode `Scene.Load` (its `runners`), and
//!   compares what came back with what the class promises; `keyed_tests` does the
//!   same for the tables keyed by entity id
//!   (lightmap entries, owned decals, …), through the despawn door and the Undo of
//!   a Destroy, and `renumber_tests` holds every saved one to the renumbering a
//!   load runs when it moves incoming ids (`runtime::load::RENUMBERED`).
//!
//! The host's state (the save data, the
//! settings, the seed, the clock, the Game view), is not the scene's: a replay
//! borrows it from `Resources`, and its one list is `dev::demo::replay::lend`'s
//! `Host`, which the replay's install writes through, so it can only write what is
//! lent.
//!
//! Unity keeps the same split implicitly: `Undo.RecordObject` covers authored
//! data, the Lighting data asset sits beside the scene and survives Undo, and
//! play-mode state is thrown away on Stop.

// The inventory, its classes and a mark per field are test hooks: compiled in every
// build, since the lifecycles that save, load or snapshot are checked by the edit
// side's tests (`rusty_edit`'s `base_tests::state`).
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
