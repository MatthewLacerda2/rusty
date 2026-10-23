//! A Play/Stop round-trip folds an Add-menu staged default into the library the way
//! the inspector does (#1215): it fills an empty key and never replaces an authored
//! asset (#997).

use rusty::scene::snapshot::SceneSnapshot;
use rusty::scene::{MaterialAsset, MaterialComponent, Scene};

/// An entity referencing its own key with `default` staged, as the Add menu leaves it
/// before the inspector folds the default in. Returns the id and the key.
fn staged_entity(scene: &mut Scene, default: MaterialAsset) -> (u32, String) {
    let id = scene.add_entity("Crate".to_string());
    let reference = MaterialComponent::for_entity(id);
    let key = reference.first().to_string();
    scene.world.set_material(id, Some(reference));
    assert!(scene.world.stage_pending_material(id, default));
    (id, key)
}

fn tinted(metallic: f32) -> MaterialAsset {
    MaterialAsset {
        metallic,
        ..MaterialAsset::default()
    }
}

fn play_stop(scene: &mut Scene) {
    let snapshot = SceneSnapshot::capture(scene);
    snapshot.restore(scene);
}

#[test]
fn play_stop_keeps_an_authored_material_over_a_staged_default() {
    let mut scene = Scene::new();
    let (id, key) = staged_entity(&mut scene, tinted(0.0));
    let authored = MaterialAsset {
        base_color: [0.5, 0.25, 0.125],
        roughness: 0.2,
        ..tinted(0.8)
    };
    scene.materials.insert(key, authored);

    play_stop(&mut scene);

    let kept = scene.material_asset_of(id).expect("reference resolves");
    assert_eq!(kept.metallic, 0.8, "authored asset unchanged");
    assert_eq!(kept.base_color, [0.5, 0.25, 0.125]);
    assert_eq!(kept.roughness, 0.2);
    assert_eq!(scene.materials.len(), 1);
}

#[test]
fn play_stop_folds_a_staged_default_into_an_empty_key() {
    let mut scene = Scene::new();
    let (id, key) = staged_entity(&mut scene, tinted(0.3));

    play_stop(&mut scene);

    assert_eq!(
        scene.materials[&key].metallic, 0.3,
        "the default fills the key"
    );
    assert_eq!(scene.material_asset_of(id).map(|m| m.metallic), Some(0.3));
}
