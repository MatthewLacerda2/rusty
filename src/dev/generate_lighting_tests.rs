//! Generate Lighting (#832): both bakes in order, with the scene's settings, and the
//! result in the scene file. The probe step skips without a GPU adapter, so the
//! probe-bake flags are not asserted; the bakes themselves are tested beside them.

use glam::Vec3;

use super::{generate_lighting, GenerateReport, UNSAVED};
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::{LightingSave, Scene};

/// A saved scene with one static, lightmappable floor and cheap bake settings.
fn saved_floor(name: &str) -> (Scene, String) {
    let dir = crate::test_temp::dir().join(format!("generate_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Plane));
    scene.world.set_static(floor, true);
    // A collider gives probe placement static bounds to fill.
    let shape = ColliderShape::Box {
        size: Vec3::new(10.0, 4.0, 10.0),
    };
    let collider = ColliderComponent {
        active: true,
        shape,
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(floor, Some(collider));
    scene.update_entity_collider(floor);
    let s = &mut scene.lighting_settings.lightmaps;
    (s.texels_per_unit, s.samples, s.bounces, s.max_resolution) = (2.0, 4, 1, 16);
    scene.save_to_file(&path).unwrap();
    (scene, path)
}

#[test]
fn gpu_generate_bakes_lightmaps_then_probes_and_writes_the_scene() {
    let (mut scene, path) = saved_floor("writes");
    let report = generate_lighting(&mut scene, Some(&path), None).unwrap();
    assert_eq!(report.lightmaps, 1);
    assert!(report.probes.auto_placed_light, "the probe step ran after");
    assert_eq!(report.saved, LightingSave::Written);

    let mut back = Scene::new();
    back.load_from_file(&path).unwrap();
    assert_eq!(back.lightmaps, scene.lightmaps, "no re-save needed");
    assert_eq!(back.probes.probes.len(), scene.probes.probes.len());
    assert_eq!(back.lighting_settings, scene.lighting_settings);
}

#[test]
fn gpu_generate_bakes_with_the_scenes_settings() {
    let (mut scene, path) = saved_floor("settings");
    scene.lighting_settings.lightmaps.directional = false;
    generate_lighting(&mut scene, Some(&path), None).unwrap();
    assert!(scene.lightmaps.directions.is_empty(), "directional was off");
}

#[test]
fn an_unsaved_scene_cannot_generate() {
    let mut scene = Scene::new();
    assert_eq!(
        generate_lighting(&mut scene, None, None),
        Err(UNSAVED.to_string())
    );
}

#[test]
fn the_summary_says_whether_the_scene_needs_saving() {
    let mut report = GenerateReport {
        lightmaps: 2,
        probes: Default::default(),
        saved: LightingSave::Written,
    };
    assert_eq!(
        report.summary(),
        "Generated lighting: 2 lightmap(s), 0 light probe(s), 0 reflection probe(s); saved"
    );
    report.saved = LightingSave::NeedsSceneSave;
    assert!(report.summary().ends_with("; save the scene to keep it"));
}
