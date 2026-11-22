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
