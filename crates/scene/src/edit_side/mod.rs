//! crates/scene/src/edit_side/ — the halves of the scene's document, file and load
//! paths that write a scene through the authoring verbs, kept apart from the base so
//! they can leave it whole for the edit crate (with `authoring`, `prefab`, the
//! instancing modules, `default_scene` and `snapshot`):
//!
//!   serialize — World <-> SceneData: `to_scene_data` and the `apply_*` functions
//!   io        — save, load into a live scene, the unsaved check, default-scene seeding
//!   rehydrate — rebuild a written model's meshes (`reimport`, `model_writes`)
//!   load      — a play-mode load's swap and the file-only half it takes (`prepare`)
//!   mesh      — point an entity's Mesh at a shared mesh (`set_shared_mesh`)
//!
//! The base names nothing here: what these halves need of it is the base's public
//! API (`SceneData`, `crate::io::read_scene_file`, `RehydratedScene`, the
//! `runtime::load` marks).

pub mod io;
pub mod load;
pub mod mesh;
pub mod rehydrate;
pub mod serialize;
