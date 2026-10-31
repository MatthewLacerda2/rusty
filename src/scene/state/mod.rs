//! src/scene/state/ — the one inventory of the scene's state (#1649).
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
//!   the saved classes' fields, in `names_tests`.
//! - **Their behaviour is checked against it.** `lifecycle_tests` sets every field
//!   (each has a mark in `marks`, and a field with none fails), runs save → load,
//!   Play → Stop, edit → Undo, an edit-mode load over it and a Play-mode
//!   `Scene.Load` (`runners`), and compares what came back with what the class
//!   promises; `keyed_tests` does the same for the tables keyed by entity id
//!   (lightmap entries, owned decals, …), through the despawn door and the Undo of
//!   a Destroy.
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

pub(crate) mod inventory;

#[cfg(test)]
mod check;
#[cfg(test)]
mod class;
#[cfg(test)]
mod keyed_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod marks;
#[cfg(test)]
mod names_tests;
#[cfg(test)]
mod runners;

#[cfg(test)]
pub use class::{promise, Class, Field, Keyed, Lifecycle, Outcome};
