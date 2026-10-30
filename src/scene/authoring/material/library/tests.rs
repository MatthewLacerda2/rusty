//! Tests for deleting a library material (#1576).

use super::*;
use crate::components::MaterialComponent;
use crate::scene::decal::DecalSpec;

/// A scene with material `"tile"` on two slots of `a` and the particles of `b`, a
/// live decal stamping it, and `"spare"` used by nothing.
fn scene() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene
        .materials
        .insert("tile".into(), MaterialAsset::default());
    scene
        .materials
        .insert("spare".into(), MaterialAsset::default());
    let a = scene.add_entity("Floor".into());
    let pair = vec!["tile".to_string(), "tile".to_string()];
    scene
        .world
        .set_material(a, Some(MaterialComponent { materials: pair }));
    let b = scene.add_entity("Sparks".into());
    let mut emitter = crate::components::ParticleEmitterComponent::default();
    emitter.render.material = Some("tile".into());
    scene.world.set_particles(b, Some(emitter));
    let spec = DecalSpec {
        material: Some("tile".into()),
        ..Default::default()
    };
    scene.spawn_decal(glam::Vec3::ZERO, glam::Vec3::Y, spec);
    (scene, a, b)
}

#[test]
fn an_unused_material_is_deleted_and_an_absent_one_is_an_error() {
    let (mut scene, ..) = scene();
    let deleted = delete_asset(&mut scene, "spare", false).unwrap();
    assert!(deleted.moved.is_empty() && deleted.created.is_empty());
    assert!(!scene.materials.contains_key("spare"));
    let err = delete_asset(&mut scene, "spare", false).unwrap_err();
    assert!(err.contains("no material asset"), "{err}");
}

#[test]
fn a_material_in_use_is_refused_naming_every_user() {
    let (mut scene, a, b) = scene();
    let err = delete_asset(&mut scene, "tile", false).unwrap_err();
    assert!(scene.materials.contains_key("tile"), "nothing changed");
    let floor = format!("entity {a} \"Floor\"");
    for user in [&format!("{floor} slot 0"), &format!("{floor} slot 1")] {
        assert!(err.contains(user.as_str()), "{err}");
    }
    assert!(
        err.contains(&format!("entity {b} \"Sparks\" particles")),
        "{err}"
    );
    assert!(err.contains("decal 1") && err.contains("force"), "{err}");
}

#[test]
fn force_moves_every_user_to_the_default_material() {
    let (mut scene, a, b) = scene();
    let deleted = delete_asset(&mut scene, "tile", true).unwrap();
    assert_eq!(deleted.moved.len(), 4);
    assert!(!scene.materials.contains_key("tile"));
    let own = entity_material_key(a);
    assert_eq!(
        scene.world.material(a).unwrap().materials,
        [own.clone(), own.clone()]
    );
    assert_eq!(
        scene.materials[&own].base_color, [1.0; 3],
        "the default white"
    );
    assert_eq!(deleted.created, [own]);
    assert_eq!(scene.world.particles(b).unwrap().render.material, None);
    assert_eq!(scene.decals[0].material, None);
}

#[test]
fn restore_undoes_a_forced_delete_in_one_step() {
    let (mut scene, a, b) = scene();
    scene.materials.get_mut("tile").unwrap().metallic = 0.7;
    let deleted = delete_asset(&mut scene, "tile", true).unwrap();
    restore_asset(&mut scene, deleted).unwrap();
    assert_eq!(scene.materials["tile"].metallic, 0.7);
    assert_eq!(scene.world.material(a).unwrap().materials, ["tile", "tile"]);
    let particles = scene.world.particles(b).unwrap().render.material.clone();
    assert_eq!(particles.as_deref(), Some("tile"));
    assert_eq!(scene.decals[0].material.as_deref(), Some("tile"));
    assert!(
        !scene.materials.contains_key(&entity_material_key(a)),
        "the default it made is gone again"
    );
    assert_eq!(users(&scene, "tile").len(), 4);
}

#[test]
fn deleting_an_entitys_own_material_leaves_it_a_fresh_default() {
    let mut scene = Scene::new();
    let a = scene.add_entity("Box".into());
    let own = entity_material_key(a);
    scene
        .world
        .set_material(a, Some(MaterialComponent::for_entity(a)));
    let red = MaterialAsset {
        base_color: [1.0, 0.0, 0.0],
        ..Default::default()
    };
    scene.materials.insert(own.clone(), red.clone());
    let deleted = delete_asset(&mut scene, &own, true).unwrap();
    assert_eq!(
        scene.materials[&own].base_color, [1.0; 3],
        "a fresh default"
    );
    restore_asset(&mut scene, deleted).unwrap();
    let back = scene.materials[&own].base_color;
    assert_eq!(back, red.base_color, "undo brings the old one back");
}

#[test]
fn restore_into_another_scene_is_refused() {
    let (mut scene, ..) = scene();
    let deleted = delete_asset(&mut scene, "spare", false).unwrap();
    let mut other = Scene::new();
    assert!(restore_asset(&mut other, deleted).is_err());
    assert!(!other.materials.contains_key("spare"));
}
