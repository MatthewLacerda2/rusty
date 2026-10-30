//! Deleting a material a prefab carries, and extracting one (#1576): a deleted
//! material stays gone through prefab propagation, and a `.prefab` carries every
//! material its entities name.

use super::*;
use crate::components::{MaterialComponent, ParticleEmitterComponent};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{
    apply_scene_data, extract_prefab, instantiate_prefab, load_and_instantiate_linked,
    reimport_instance, save_prefab, to_scene_data, SceneData, SceneSnapshot,
};

/// A one-entity prefab carrying material `"Stone"` on its slot, saved to a temp file.
fn stone_prefab(tag: &str) -> String {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Root", Some(Primitive::Box));
    let stone = MaterialAsset::default();
    scene.materials.insert("Stone".into(), stone);
    let used = Some(MaterialComponent::new("Stone"));
    scene.world.set_material(root, used);
    let name = format!("rusty_1576_{tag}.prefab");
    let path = crate::test_temp::dir().join(name);
    let path = path.to_string_lossy().into_owned();
    save_prefab(&scene, root, &path).unwrap();
    path
}

#[test]
fn a_deleted_material_a_prefab_carries_stays_gone() {
    let path = stone_prefab("stays_gone");
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    delete_asset(&mut scene, "Stone", true).unwrap();
    let fallback = entity_material_key(root);
    assert_eq!(scene.world.material(root).unwrap().first(), fallback);

    // Play/Stop: the snapshot comes back through propagation.
    SceneSnapshot::capture(&scene).restore(&mut scene);
    assert!(!scene.materials.contains_key("Stone"), "Play/Stop");
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.materials.contains_key("Stone"), "reimport");

    // Save and reload: the next save does not write it again.
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let mut reloaded = Scene::new();
    apply_scene_data(
        &mut reloaded,
        serde_json::from_str::<SceneData>(&json).unwrap(),
    );
    assert!(!reloaded.materials.contains_key("Stone"), "reload");
    assert!(!to_scene_data(&reloaded).materials.contains_key("Stone"));
    assert_eq!(reloaded.world.material(root).unwrap().first(), fallback);
}

#[test]
fn a_prefab_carries_its_particles_material() {
    let mut scene = Scene::new();
    let root = create_entity(&mut scene, "Sparks", None);
    scene
        .materials
        .insert("spark".into(), MaterialAsset::default());
    let mut emitter = ParticleEmitterComponent::default();
    emitter.render.material = Some("spark".into());
    scene.world.set_particles(root, Some(emitter));
    let prefab = extract_prefab(&scene, root).unwrap();
    assert!(prefab.materials.contains_key("spark"));

    let mut elsewhere = Scene::new();
    let copy = instantiate_prefab(&mut elsewhere, &prefab, None);
    assert!(elsewhere.materials.contains_key("spark"));
    let users = users(&elsewhere, "spark");
    assert_eq!(users, vec![MaterialUser::Particles { id: copy }]);
}
