//! crates/edit/src/rehydrate/mod.rs — rebuild what a written model
//! changed in a live scene: its meshes (`reimport`) and the writes the tick follows
//! (`model_writes`). The base's `rehydrate` rebuilds one mesh from its reference;
//! this half re-baselines the prefab instances a reimport touches, so it sits on
//! the edit side.

pub mod model_writes;
mod reimport;

pub use reimport::{
    reimport_model, reimport_model_except, reimport_model_ids, reimport_written, Rig,
};

#[cfg(test)]
mod shared_tests;
#[cfg(test)]
mod sharing_edit_tests;
#[cfg(test)]
mod sharing_tests;
#[cfg(test)]
mod tests;
