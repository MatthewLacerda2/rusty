//! crates/edit/src/lib.rs — Scene editing: the verbs that write a scene, and the
//! paths that build one from files.
//!
//! The base (`rusty-scene`) holds the `Scene` and every type its fields hold; this
//! crate sits above it so an authoring edit rebuilds only the crates that write or
//! build a scene (`script`, `editor`, `dev`, `app`, `shell`, `preview`) and the root
//! package, never AI or UI. `render`, `physics`, `navigation` and `lighting` name
//! this crate from their tests only: their libraries do not rebuild, their test
//! targets do.
//!
//! Submodules:
//!   authoring — the shared edit verbs the editor, the API and loads all go through
//!   serialize — World <-> SceneData: `to_scene_data` and the `apply_*` functions
//!   io        — save, load into a live scene, the unsaved check, default-scene seeding
//!   rehydrate — rebuild a written model's meshes (`reimport`, `model_writes`)
//!   load      — a play-mode load's swap and the file-only half it takes (`prepare`)
//!   mesh      — point an entity's Mesh at a shared mesh (`set_shared_mesh`)
//!   prefab    — `.prefab` files: extract, instantiate, linked instances
//!   asset_instance, lod_instance, model_instance — instantiate an imported model
//!   default_scene — the default scene's builder, materials and seeded assets
//!   snapshot  — edit-mode snapshot/restore around Play
//!
//! What this crate needs of the base is the base's public API (`SceneData`,
//! `rusty_scene::io::read_scene_file`, `RehydratedScene`, the `runtime::load` marks).

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod asset_instance;
pub mod authoring;
pub mod default_scene;
pub mod io;
pub mod load;
pub mod lod_instance;
pub mod mesh;
pub mod model_instance;
pub mod prefab;
pub mod rehydrate;
pub mod serialize;
pub mod snapshot;

#[cfg(test)]
mod base_tests;

pub use io::{
    build_default_scene, bundled, has_unsaved_edits, load_from_file, load_parsed, load_rehydrated,
    print_refresh, save_lighting, save_to_file, seed_default_scene, seed_default_scripts,
    seed_project, seed_workspace, LightingSave, LoadError, DEFAULT_SCENE_PATH,
    DEFAULT_SCRIPTS_DEST_DIR, LIGHTING_IN_PLAY,
};
pub use prefab::link::{list_instance_overrides, reimport_instance, revert_instance_overrides};
pub use prefab::{
    extract_prefab, instantiate_prefab, is_prefab_path, load_and_instantiate,
    load_and_instantiate_linked, read_prefab_file, save_prefab, PrefabData, PREFAB_EXTENSION,
};
pub use rehydrate::model_writes;
pub use serialize::{apply_scene_data, to_scene_data};
pub use snapshot::SceneSnapshot;
