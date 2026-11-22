//! A fingerprint is what the bake reads and nothing else: a scene loaded
//! straight after its own bake matches it, however the bake's process differed
//! from the one loading it.

use std::sync::Arc;

use rusty::edit::authoring::material::define_asset;
use rusty::lighting::occlusion::test_level::corridors;
use rusty::lighting::occlusion::{bake_occlusion, fingerprint};
use rusty::scene::lighting::occlusion::save_sidecar;
use rusty::scene::{MaterialAsset, MaterialComponent, Scene};

/// The case from the game: every static mesh draws with `"wood"`, which the
/// scene file's library lacks (the game's script defines it). A headless run
/// defines it in memory, bakes and writes the PVS; a fresh load, the editor's,
/// never ran that script.
#[test]
fn a_fresh_load_matches_a_bake_that_defined_a_material_in_memory() {
    let dir = crate::temp::dir().join("occlusion_fresh_load");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut level = corridors();
    for id in level.world.ids_with_mesh() {
        let wood = Some(MaterialComponent::new("wood"));
        level.world.set_material(id, wood);
    }
    level.materials.remove("wood");
    rusty::edit::save_to_file(&level, &path).unwrap();

    let mut baking = Scene::new();
    rusty::edit::load_from_file(&mut baking, &path).unwrap();
    define_asset(&mut baking.materials, "wood", MaterialAsset::default());
    let data = bake_occlusion(&baking, 2).0.expect("occluders");
    *baking.occlusion = Some(Arc::new(data));
    save_sidecar(&baking, &path).unwrap();

    let mut editor = Scene::new();
    rusty::edit::load_from_file(&mut editor, &path).unwrap();
    let pvs = editor.occlusion.as_deref().expect("the bake loaded");
    assert!(pvs.objects.iter().any(|o| o.occluder));
    for baked in &pvs.objects {
        let now = fingerprint(&editor, baked.entity);
        assert_eq!(now, Some(baked.fingerprint), "entity {}", baked.entity);
    }
}
