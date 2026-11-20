//! Whether a held PVS is what a bake of the scene would write (#2348,
//! [`is_current`](crate::occlusion::is_current)): Generate Lighting keeps it only then.

use glam::Vec3;

use crate::occlusion::test_level::{block, corridors};
use crate::occlusion::{bake_occlusion, is_current, OcclusionBakeReport, Reach};
use rusty_components::RenderMode;
use rusty_scene::lighting::occlusion::OcclusionData;
use rusty_scene::{MaterialAsset, MaterialComponent, Scene};

fn baked(scene: &Scene) -> OcclusionData {
    bake_occlusion(scene, 2).0.expect("occluders")
}

fn kept(scene: &Scene, held: &OcclusionData) -> bool {
    is_current(scene, held, &Reach::everywhere())
}

/// The first prop: a static box that occludes.
fn prop(scene: &Scene) -> u32 {
    let ids = scene.world.ids_with_mesh().into_iter();
    ids.max().expect("a static mesh")
}

#[test]
fn an_unchanged_scene_matches_even_with_its_lights_moved() {
    let mut scene = corridors();
    let held = baked(&scene);
    assert!(kept(&scene, &held));
    for id in scene.world.ids_with_light() {
        scene.world.transform_mut(id).unwrap().position += Vec3::X;
    }
    assert!(kept(&scene, &held), "a light is no input of the bake");
}

#[test]
fn a_moved_added_or_removed_static_mesh_does_not_match() {
    let scene = corridors();
    let held = baked(&scene);
    let mut moved = corridors();
    let id = prop(&moved);
    moved.world.transform_mut(id).unwrap().position += Vec3::X * 0.01;
    assert!(!kept(&moved, &held), "moved");
    let mut added = corridors();
    block(&mut added, "Extra", Vec3::splat(3.0), Vec3::splat(3.5));
    assert!(!kept(&added, &held), "added");
    let mut removed = corridors();
    removed.world.despawn(prop(&removed));
    assert!(!kept(&removed, &held), "removed");
}

#[test]
fn a_changed_occluder_flag_or_layer_does_not_match() {
    let scene = corridors();
    let held = baked(&scene);
    let mut cutout = corridors();
    let id = prop(&cutout);
    let asset = MaterialAsset {
        render_mode: RenderMode::Cutout,
        ..MaterialAsset::default()
    };
    cutout.materials.insert("see-through".into(), asset);
    let material = Some(MaterialComponent::new("see-through"));
    cutout.world.set_material(id, material);
    assert!(!kept(&cutout, &held), "no longer occludes");
    let mut flag = held.clone();
    flag.objects[0].occluder = !flag.objects[0].occluder;
    assert!(!kept(&scene, &flag), "the held flag alone differs");
    let mut layer = corridors();
    layer.world.set_layer(prop(&layer), 3);
    assert!(!kept(&layer, &held), "another layer");
}

#[test]
fn another_bake_version_does_not_match() {
    let scene = corridors();
    let mut held = baked(&scene);
    held.stamp.version += 1;
    assert!(!kept(&scene, &held));
}

#[test]
fn a_kept_pvs_reports_what_its_bake_reported() {
    let scene = corridors();
    let (data, report) = bake_occlusion(&scene, 2);
    let data = data.expect("occluders");
    assert_eq!(OcclusionBakeReport::of_kept(&data), report);
    let encoded = rusty_scene::lighting::occlusion::encode(&data);
    assert_eq!(report.bytes, encoded.len(), "the length without encoding");
}
