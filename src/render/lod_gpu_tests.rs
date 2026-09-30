//! LOD groups on the GPU (#472): a row of props receding from the camera submits far
//! fewer triangles with a sphere LOD0 and a box LOD1 than with the sphere alone, each
//! prop still drawing exactly one level; and a group near enough for LOD0 renders
//! exactly the pixels of that LOD0 on its own. Skips when no adapter is present.

use glam::{Quat, Vec3};

use crate::components::{LodGroupComponent, LodLevel};
use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 96;
const H: u32 = 64;

/// A sun and one prop per `z` in `depths`, 1 m across: with `lod`, an empty group
/// carrying a sphere LOD0 (shown down to 30% of the screen) and a box LOD1 (never
/// culled); without, the sphere alone — the scene before LODs existed.
fn prop_row(depths: &[f32], lod: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    for (n, &z) in depths.iter().enumerate() {
        let x = if n % 2 == 0 { -1.0 } else { 1.0 };
        let at = Vec3::new(x, 0.5, -z);
        let sphere = create_entity(
            &mut scene,
            &format!("Prop_{n}_LOD0"),
            Some(Primitive::Sphere),
        );
        if !lod {
            scene.world.transform_mut(sphere).unwrap().position = at;
            continue;
        }
        let group = scene.add_entity(format!("Prop_{n}"));
        scene.world.transform_mut(group).unwrap().position = at;
        let cube = create_entity(&mut scene, &format!("Prop_{n}_LOD1"), Some(Primitive::Box));
        scene.set_parent(sphere, Some(group)).unwrap();
        scene.set_parent(cube, Some(group)).unwrap();
        let level = |screen_height, r| LodLevel {
            screen_height,
            renderers: vec![r],
        };
        let levels = vec![level(0.3, sphere), level(0.0, cube)];
        let lod = LodGroupComponent { levels, size: 1.0 };
        scene.world.set_lod_group(group, Some(lod));
    }
    scene
}

/// Render `scene` once from just behind the row; the pixels and what was submitted.
fn render(renderer: &mut Renderer, scene: &Scene) -> (Vec<u8>, RenderCounters) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 1.0), -90.0, -10.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, W, H);
    (px, renderer.frame_counters)
}

#[test]
fn gpu_lod_groups_cut_distant_triangles_and_keep_near_pixels() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let depths: Vec<f32> = (0..24).map(|i| 2.0 + i as f32 * 2.0).collect();
    let (_, before) = render(&mut renderer, &prop_row(&depths, false));
    let (_, after) = render(&mut renderer, &prop_row(&depths, true));
    eprintln!("LOD row (#472): before {before:?}\n                 after  {after:?}");

    // Every prop still draws exactly one level; the other is hidden, not culled.
    assert_eq!(after.visible_entities, before.visible_entities);
    assert!(
        before.visible_entities >= 12,
        "the row is on screen: {before:?}"
    );
    assert_eq!(after.lod_hidden_entities, depths.len() as u32);
    assert_eq!(before.lod_hidden_entities, 0);
    // Distant spheres became boxes, on screen and in the shadow casters.
    assert!(
        after.triangles * 3 < before.triangles,
        "LOD1 should cut the triangles: {before:?} -> {after:?}"
    );

    // Near enough for LOD0, a group is pixel-identical to its LOD0 alone.
    let (sphere_only, _) = render(&mut renderer, &prop_row(&[2.0], false));
    let (grouped, near) = render(&mut renderer, &prop_row(&[2.0], true));
    assert_eq!(near.lod_hidden_entities, 1, "the box is hidden: {near:?}");
    let distinct: std::collections::HashSet<_> = grouped.chunks(4).collect();
    assert!(distinct.len() > 4, "the frame actually shows the prop");
    let differing = sphere_only
        .chunks(4)
        .zip(grouped.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        differing, 0,
        "LOD0 through a group changed {differing} pixels"
    );
}
