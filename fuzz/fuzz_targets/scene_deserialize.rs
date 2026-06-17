#![no_main]
//! Fuzz the scene-load path (#119): the "what happens when someone hands the
//! engine a corrupt save file" class of bug. Mirrors `Scene::load_from_file` —
//! arbitrary bytes -> UTF-8 -> `SceneData` (serde) -> `apply_scene_data` (the
//! rehydration that rebuilds meshes/colliders/skeletons). Surfaces panics,
//! hangs, and unguarded `unwrap`s in the deserializer and rehydration.

use libfuzzer_sys::fuzz_target;
use rusty::scene::{apply_scene_data, Scene, SceneData};

fuzz_target!(|data: &[u8]| {
    // Scene files are JSON text; non-UTF-8 inputs can't be a scene, so skip them
    // (serde_json would reject them anyway) and spend the budget on plausible
    // documents.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(scene_data) = serde_json::from_str::<SceneData>(text) {
        // A well-formed document still must not panic the rehydration path.
        let mut scene = Scene::new();
        apply_scene_data(&mut scene, scene_data);
    }
});
