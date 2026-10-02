//! Hitbox fitting and generation (#464) over the mini humanoid in `rig`.

use glam::Vec3;

use super::rig::{rig, skin, vertices};
use super::*;
use crate::components::{CapsuleAxis, ColliderShape};

fn shape_of(fits: &[HitboxFit], slot: usize) -> Option<&ColliderShape> {
    fits.iter().find(|f| f.slot == slot).map(|f| &f.shape)
}

#[test]
fn limbs_get_capsules_flat_parts_boxes_and_fingers_fold_into_the_hand() {
    let fits = fit_hitboxes(&vertices(), &skin(1.0), &HitboxOptions::default());
    assert_eq!(
        fits.iter().map(|f| f.slot).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(
        matches!(shape_of(&fits, 0), Some(ColliderShape::Box { .. })),
        "pelvis"
    );
    assert!(
        matches!(shape_of(&fits, 1), Some(ColliderShape::Box { .. })),
        "chest"
    );
    let Some(&ColliderShape::Capsule {
        radius,
        height,
        axis,
    }) = shape_of(&fits, 2)
    else {
        panic!("the head is a capsule");
    };
    assert_eq!(axis, CapsuleAxis::Y);
    assert!((radius - 0.105).abs() < 1e-5 && (height - 0.26).abs() < 1e-5);
    // The arm runs along X from its joint (x 0.25) to the finger tip (x 0.64).
    let Some(&ColliderShape::Capsule { height, axis, .. }) = shape_of(&fits, 3) else {
        panic!("the arm is a capsule");
    };
    assert_eq!(axis, CapsuleAxis::X);
    assert!(
        (height - 0.39).abs() < 1e-5,
        "the finger folded in: {height}"
    );
    let arm = fits.iter().find(|f| f.slot == 3).unwrap();
    assert!(
        arm.center.abs_diff_eq(Vec3::new(0.195, 0.0, 0.0), 1e-5),
        "{:?}",
        arm.center
    );
}

#[test]
fn an_allowlist_hands_unlisted_bones_to_their_parent() {
    let opts = HitboxOptions {
        bones: Some(vec!["spine".into(), "head".into()]),
        ..HitboxOptions::default()
    };
    let fits = fit_hitboxes(&vertices(), &skin(1.0), &opts);
    assert_eq!(fits.iter().map(|f| f.slot).collect::<Vec<_>>(), [1, 2]);
    // The arm and finger folded into the chest, which now reaches x 0.64.
    let Some(&ColliderShape::Box { size }) = shape_of(&fits, 1) else {
        panic!("the chest is a box");
    };
    assert!((size.x - 0.84).abs() < 1e-5, "{size:?}");
}

#[test]
fn min_size_is_in_model_units_under_a_scaled_skeleton() {
    // At 0.01 the joint frame measures the 0.26 head as 26 units; the threshold
    // still reads model units.
    let opts = |min_size| HitboxOptions {
        min_size,
        ..HitboxOptions::default()
    };
    let small = fit_hitboxes(&vertices(), &skin(0.01), &opts(0.25));
    assert!(shape_of(&small, 2).is_some(), "the 0.26 head clears 0.25");
    let big = fit_hitboxes(&vertices(), &skin(0.01), &opts(0.3));
    assert!(shape_of(&big, 2).is_none(), "but not 0.3");
}

#[test]
fn generated_hitboxes_hang_from_their_bones_on_a_layer_that_touches_nothing() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    let made = scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let names: Vec<&str> = made.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["pelvis", "spine", "head", "arm"]);
    let layer = scene
        .layers
        .index_of("Hitbox")
        .expect("the layer was created");
    assert_eq!(scene.collision_matrix.filter_mask(layer), 0);
    assert!(!scene.collision_matrix.can_collide(0, layer));
    for (name, hitbox) in &made {
        let bone = scene.find_bone(owner, name).unwrap();
        assert_eq!(scene.world.parent_id(*hitbox), Some(bone));
        assert_eq!(scene.world.layer(*hitbox), layer);
        assert_eq!(scene.hit_bone(*hitbox), Some(bone), "a hit names the bone");
        assert_eq!(scene.root_of(*hitbox), owner, "and the character");
    }
    let loose = scene.add_entity("Crate".to_string());
    assert_eq!((scene.hit_bone(loose), scene.root_of(loose)), (None, loose));
}

#[test]
fn regenerating_is_repeatable_and_drops_hitboxes_that_no_longer_qualify() {
    let mut scene = Scene::new();
    let owner = rig(&mut scene);
    let first = scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    let shapes = |s: &Scene| -> Vec<_> {
        first
            .iter()
            .map(|(_, h)| s.world.collider(*h).map(|c| c.shape.clone()))
            .collect()
    };
    let before = shapes(&scene);
    let again = scene
        .generate_hitboxes(owner, &HitboxOptions::default())
        .unwrap();
    assert_eq!(again, first, "same skin, same hitbox entities");
    assert_eq!(shapes(&scene), before, "and the same boxes");
    let opts = HitboxOptions {
        min_size: 0.3,
        ..HitboxOptions::default()
    };
    let fewer = scene.generate_hitboxes(owner, &opts).unwrap();
    assert!(fewer.iter().all(|(n, _)| n != "head"));
    let head = scene.find_bone(owner, "head").unwrap();
    assert_eq!(
        scene.hitbox_of(head),
        None,
        "the head's stale hitbox is gone"
    );
}

#[test]
fn a_mesh_without_a_skin_is_refused() {
    let mut scene = Scene::new();
    let plain = scene.add_entity("Plain".to_string());
    assert!(scene
        .generate_hitboxes(plain, &HitboxOptions::default())
        .is_err());
}
