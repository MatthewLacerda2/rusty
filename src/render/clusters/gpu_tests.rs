//! Clustered lighting on the GPU (#434): eight point lights over a dark floor all
//! light their own patch (the old forward uniform kept four), and lights outside
//! the view are culled before binning — the frame is pixel-identical without them
//! and their cluster work is zero. Skips when no adapter is present.

use glam::{Vec2, Vec3};

use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 128;
const H: u32 = 96;
const LIGHTS: usize = 8;

/// Looking straight down at the floor from 12 m up.
fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 12.0, 0.0), -90.0, -89.0)
}

/// Where each in-view light sits: a ring of radius 4, half a metre over the floor.
fn ring(n: usize) -> Vec3 {
    let a = n as f32 / LIGHTS as f32 * std::f32::consts::TAU;
    Vec3::new(4.0 * a.cos(), 0.5, 4.0 * a.sin())
}

/// A dark floor, no ambient or sun, and a short-range point light at each `ring`
/// position but `skip`, then one per `hidden` position.
fn scene(skip: Option<usize>, hidden: &[Vec3]) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().scale = Vec3::new(30.0, 1.0, 30.0);
    let shown = (0..LIGHTS).filter(|&n| Some(n) != skip).map(ring);
    for (n, at) in shown.chain(hidden.iter().copied()).enumerate() {
        let id = create_entity(
            &mut scene,
            &format!("Lamp_{n}"),
            Some(Primitive::PointLight),
        );
        scene.world.transform_mut(id).unwrap().position = at;
        let mut light = scene.world.light(id).unwrap().clone();
        (light.range, light.intensity) = (1.5, 4.0);
        scene.world.set_light(id, Some(light));
    }
    scene
}

fn render(renderer: &mut Renderer, scene: &Scene) -> (Vec<u8>, RenderCounters) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();
    renderer.render(&mut view, scene, &camera(), &out, false);
    let texture = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&renderer.device, &renderer.queue, texture, W, H);
    (px, renderer.frame_counters)
}

/// The summed RGB of the pixel under world point `p` on the floor.
fn brightness(px: &[u8], p: Vec3) -> u32 {
    let at = camera().world_to_screen(Vec3::new(p.x, 0.0, p.z), Vec2::new(W as f32, H as f32));
    let (x, y) = (at.position.x as u32, H - 1 - at.position.y as u32);
    let i = ((y * W + x) * 4) as usize;
    px[i..i + 3].iter().map(|&c| u32::from(c)).sum()
}

#[test]
fn gpu_more_than_four_point_lights_all_contribute() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (all, counters) = render(&mut renderer, &scene(None, &[]));
    assert_eq!(counters.lights_visible as usize, LIGHTS, "{counters:?}");
    assert_eq!(counters.lights_dropped, 0, "{counters:?}");
    let dark = brightness(&all, Vec3::ZERO);
    for n in 0..LIGHTS {
        let lit = brightness(&all, ring(n));
        assert!(lit > dark + 60, "light {n} lit nothing: {lit} vs {dark}");
        // Taking that one lamp away darkens exactly its own patch.
        let (without, _) = render(&mut renderer, &scene(Some(n), &[]));
        let left = brightness(&without, ring(n));
        assert!(
            left + 60 < lit,
            "light {n} did not contribute: {left} vs {lit}"
        );
    }
}

#[test]
fn gpu_lights_outside_the_view_cost_nothing() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    // Above and behind the camera, and far off to every side.
    let hidden: Vec<Vec3> = (0..64)
        .map(|i| match i % 4 {
            0 => Vec3::new(i as f32, 20.0, 0.0),
            1 => Vec3::new(80.0 + i as f32, 0.5, 0.0),
            2 => Vec3::new(-80.0, 0.5, i as f32),
            _ => Vec3::new(0.0, 0.5, 80.0 + i as f32),
        })
        .collect();
    let (base, base_counts) = render(&mut renderer, &scene(None, &[]));
    let (crowded, counts) = render(&mut renderer, &scene(None, &hidden));
    assert_eq!(counts.lights_culled, 64, "{counts:?}");
    assert_eq!(counts.lights_visible, base_counts.lights_visible);
    assert_eq!(counts.light_cluster_refs, base_counts.light_cluster_refs);
    assert!(base.iter().any(|&c| c > 40), "the frame shows the lamps");
    let differing = base.iter().zip(&crowded).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "off-screen lights changed {differing} pixels");
}
