//! The selection outline rings what the forward pass drew (#1160): a skinned mesh in
//! its pose, not its bind pose, and a LOD group at the level the camera shows, not
//! LOD0. GPU tests skip with no adapter; the target choice is tested without one.

use glam::{Mat4, Vec3};

use super::outline::outline_targets;
use super::outline_tests::{frame, outline_stats, RES};
use crate::asset::{JointTransform, SkinData};
use crate::components::mesh::primitives::generate_sphere;
use crate::components::{LodGroupComponent, LodLevel, MeshComponent};
use crate::render::lod::LodSelection;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// A one-joint skinned sphere (radius 0.8) whose pose moves it by `pose`, selected
/// at the origin.
fn posed_sphere(pose: Vec3) -> Scene {
    let (vertices, indices) = generate_sphere(0.8, 12, 16);
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        local_bind: vec![JointTransform::default()],
        parents: vec![None],
        joint_nodes: vec![0],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["root".into()],
    };
    let mesh = MeshComponent {
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        pose_palette: vec![Mat4::from_translation(pose)],
        is_dirty: Default::default(),
        ..MeshComponent::new("PosedSphere", vertices, indices)
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Soldier".to_string());
    scene.world.set_mesh(id, Some(mesh));
    scene.selected_entity_id = Some(id);
    scene
}

/// An empty group at the origin over a LOD0 box at -X and a LOD1 box at +X. LOD0's
/// threshold is never reached, so the camera always shows LOD1. Returns the scene
/// and (group, LOD0, LOD1).
fn lod_switched() -> (Scene, [u32; 3]) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let group = scene.add_entity("Prop".to_string());
    let mut child = |name, x| {
        let id = create_entity(&mut scene, name, Some(Primitive::Box));
        scene.set_parent(id, Some(group)).unwrap();
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (Vec3::new(x, 0.0, 0.0), Vec3::splat(0.8));
        id
    };
    let (lod0, lod1) = (child("Prop_LOD0", -0.55), child("Prop_LOD1", 0.55));
    let level = |screen_height, r| LodLevel {
        screen_height,
        renderers: vec![r],
    };
    let levels = vec![level(10.0, lod0), level(0.0, lod1)];
    let lod = LodGroupComponent { levels, size: 1.0 };
    scene.world.set_lod_group(group, Some(lod));
    (scene, [group, lod0, lod1])
}

/// The outline's horizontal centroid, in pixels right of the frame's centre.
fn centroid_right(r: &mut crate::render::Renderer, scene: &Scene, tag: &str) -> f32 {
    let (count, centroid, filled) = outline_stats(&frame(r, scene, Vec3::ZERO));
    eprintln!("[outline, {tag}] {count} px, centroid {centroid:?}");
    assert!(count > 20, "{tag}: the selection shows an outline");
    assert!(!filled, "{tag}: the outline is a silhouette, not a fill");
    centroid[0] - RES as f32 / 2.0
}

#[test]
fn gpu_the_outline_follows_a_skinned_pose_and_the_shown_lod() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    // Posed 0.4 m right of its bind pose: about 10 px at this distance.
    let posed = centroid_right(&mut r, &posed_sphere(Vec3::X * 0.4), "posed");
    assert!(posed > 5.0, "the outline rings the pose, {posed} px right");
    let rest = centroid_right(&mut r, &posed_sphere(Vec3::ZERO), "rest");
    assert!(rest.abs() < 4.0, "an unposed skin rings its bind pose");

    // LOD1 (+X) shows; selecting the group or the hidden LOD0 rings LOD1.
    let (mut scene, [group, lod0, _]) = lod_switched();
    for (tag, selected) in [("group", group), ("hidden LOD0", lod0)] {
        scene.selected_entity_id = Some(selected);
        let right = centroid_right(&mut r, &scene, tag);
        assert!(
            right > 8.0,
            "{tag}: the outline rings LOD1, {right} px right"
        );
    }
}

#[test]
fn a_lod_selection_outlines_the_level_the_camera_shows() {
    let (scene, [group, lod0, lod1]) = lod_switched();
    let cam = Camera::new(Vec3::Z * 3.0, -90.0, 0.0);
    let lod = LodSelection::for_camera(&scene, &cam);
    assert_eq!(outline_targets(&scene, group, &lod), vec![lod1]);
    assert_eq!(outline_targets(&scene, lod0, &lod), vec![lod1]);
    assert_eq!(outline_targets(&scene, lod1, &lod), vec![lod1]);
    // The finest selection (what the static bake draws) shows LOD0 instead.
    let finest = LodSelection::finest(&scene);
    assert_eq!(outline_targets(&scene, lod1, &finest), vec![lod0]);
}
