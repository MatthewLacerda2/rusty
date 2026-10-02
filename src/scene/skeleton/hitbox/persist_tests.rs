//! Generated hitboxes survive a save/load (#464): bones are rebuilt from the model,
//! and each hitbox comes back under its bone, re-bound by the bone's name.

use super::super::fixture::hero;
use super::*;
use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};

#[test]
fn hitboxes_come_back_under_their_bones_after_a_reload() {
    let mut scene = Scene::new();
    let owner = hero(&mut scene, "hitbox_reload");
    let opts = HitboxOptions {
        min_size: 0.0,
        ..HitboxOptions::default()
    };
    let made = scene.generate_hitboxes(owner, &opts).unwrap();
    assert_eq!(
        made.len(),
        2,
        "one hitbox per joint of the fixture: {made:?}"
    );

    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(
        &mut loaded,
        serde_json::from_str::<SceneData>(&json).unwrap(),
    );

    let layer = loaded
        .layers
        .index_of("Hitbox")
        .expect("the layer was saved");
    assert_eq!(
        loaded.collision_matrix.filter_mask(layer),
        0,
        "and its matrix row"
    );
    for (name, before) in &made {
        let bone = loaded.find_bone(owner, name).expect("the bone was rebuilt");
        let after = loaded.hitbox_of(bone).expect("its hitbox re-attached");
        assert_eq!(loaded.world.layer(after), layer);
        let shape = |s: &Scene, id| s.world.collider(id).map(|c| c.shape.clone());
        assert_eq!(shape(&loaded, after), shape(&scene, *before), "{name}");
        let pos = |s: &Scene, id| s.world.transform(id).map(|t| t.position);
        assert_eq!(pos(&loaded, after), pos(&scene, *before), "{name}");
    }
    let again = loaded.generate_hitboxes(owner, &opts).unwrap();
    let ids: Vec<u32> = again.iter().map(|(_, id)| *id).collect();
    let kept: Vec<u32> = made
        .iter()
        .map(|(n, _)| {
            loaded
                .hitbox_of(loaded.find_bone(owner, n).unwrap())
                .unwrap()
        })
        .collect();
    assert_eq!(
        ids, kept,
        "regenerating after a load refits, never duplicates"
    );
}
