//! The base's tests that reach the edit side: each checks a base module (a
//! skeleton, the mesh library, Undo, the state inventory, …) through a save and
//! load, a Play snapshot, an authoring op or a prefab, so it moves with the edit
//! side and reaches the base through its public API. Each file is named for the
//! base module it checks.

mod camera_lens;
mod decal;
mod io;
mod mesh_library;
mod physics_material;
mod reflection_probes;
mod round_trips;
mod skeleton;
mod state;
mod undo;
