//! GPU instancing's visual-equivalence guard (#470): a prop-heavy scene rendered with
//! instanced draws must produce exactly the pixels of one draw per entity, while
//! issuing far fewer draw calls. Skips when no adapter is present.

use glam::{Quat, Vec3};

use crate::components::{MaterialAsset, MaterialComponent, RenderMode};
use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 96;
const H: u32 = 64;

fn material(scene: &mut Scene, id: u32, name: &str) {
    scene.world.set_material(
        id,
        Some(MaterialComponent {
            material: name.to_string(),
        }),
    );
}

/// A 6×6 yard of three prop meshes under a shadow-casting sun: most share the default
/// material, every 4th is red (a second batch per mesh), every 7th is translucent
/// glass (the sorted pass), so every grouping rule is on screen at once.
fn prop_yard() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let red = MaterialAsset {
        base_color: [0.9, 0.1, 0.1],
        ..MaterialAsset::default()
    };
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.4,
        base_color: [0.2, 0.5, 0.9],
        ..MaterialAsset::default()
    };
    scene.materials.insert("red".to_string(), red);
    scene.materials.insert("glass".to_string(), glass);
    let kinds = [Primitive::Box, Primitive::Sphere, Primitive::Cylinder];
    for n in 0..36u32 {
        let id = create_entity(
            &mut scene,
            &format!("Prop_{n}"),
            Some(kinds[n as usize % 3]),
        );
        let (x, z) = ((n % 6) as f32, (n / 6) as f32);
        {
            let mut t = scene.world.transform_mut(id).unwrap();
            t.position = Vec3::new(x * 1.6 - 4.0, 0.5, -z * 1.6);
            t.scale = Vec3::splat(0.7);
        }
        if n % 7 == 3 {
            material(&mut scene, id, "glass");
        } else if n % 4 == 1 {
            material(&mut scene, id, "red");
        }
    }
    scene
}

/// Render `scene` once with instancing on or off; the pixels and what was submitted.
fn render(renderer: &mut Renderer, scene: &Scene, instancing: bool) -> (Vec<u8>, RenderCounters) {
    renderer.instancing = instancing;
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 5.0, 8.0), -90.0, -30.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, W, H);
    (px, renderer.frame_counters)
}

#[test]
fn gpu_instanced_props_render_the_same_pixels_as_one_draw_each() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = prop_yard();
    let (per_entity, before) = render(&mut renderer, &scene, false);
    let (instanced, after) = render(&mut renderer, &scene, true);

    // The same work reached the GPU — every entity, every triangle — in fewer calls.
    assert!(
        before.visible_entities >= 30,
        "the yard is on screen: {before:?}"
    );
    assert_eq!(after.visible_entities, before.visible_entities);
    assert_eq!(after.triangles, before.triangles);
    assert!(
        after.draw_calls * 3 < before.draw_calls,
        "instancing should collapse the props: {before:?} -> {after:?}"
    );
    // One caster draw per mesh per cascade (#435): each cascade is its own depth pass.
    let cascades = crate::render::passes::shadows::cascades::MAX_CASCADES as u32;
    assert!(
        after.shadow_draws <= 3 * cascades,
        "one caster draw per mesh per cascade: {after:?}"
    );

    // And the image is identical, pixel for pixel.
    let distinct: std::collections::HashSet<_> = per_entity.chunks(4).collect();
    assert!(distinct.len() > 8, "the frame actually shows the props");
    let differing = per_entity
        .chunks(4)
        .zip(instanced.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differing, 0, "instancing changed {differing} pixels");
}
