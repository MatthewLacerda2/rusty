//! Bloom's mip chain on lavapipe (#1700): a small lamp on a dark wall glows far past
//! the old 5×5 gaussian's reach, the scatter widens it, and a view too small for a
//! second mip renders no darker than without bloom.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent, PostProcessingComponent};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 192;
const H: u32 = 128;

/// A black wall filling the view and, at its centre, a small box lit by its own
/// `emissive`; the camera's volume blooms as `tune` sets it.
fn scene(emissive: f32, tune: impl FnOnce(&mut PostProcessingComponent)) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let wall = glowing(&mut scene, "Wall", Vec3::new(0.0, 0.0, -6.0), 0.0);
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(60.0, 60.0, 0.2);
    let dot = glowing(&mut scene, "Dot", Vec3::new(0.0, 0.0, -3.0), emissive);
    scene.world.transform_mut(dot).unwrap().scale = Vec3::splat(0.05);
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut vc: PostProcessingComponent = default_post_processing();
    (vc.ssr.active, vc.ssao.active) = (false, false);
    tune(&mut vc);
    scene.world.set_post_processing(cam, Some(vc));
    scene
}

fn glowing(scene: &mut Scene, name: &str, at: Vec3, emissive: f32) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = at;
    let asset = MaterialAsset {
        base_color: [0.0; 3],
        emissive: [emissive; 3],
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name)));
    id
}

/// One frame of `scene` through a fresh `w` × `h` view (RGBA8).
fn shot(r: &mut Renderer, scene: &Scene, (w, h): (u32, u32)) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, w, h);
    let out = view.color_target_view().unwrap();
    let eye = Camera::new(Vec3::ZERO, -90.0, 0.0);
    r.render(&mut view, scene, &eye, &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), w, h)
}

/// The summed brightness of the row of pixels `from..to` right of the centre.
fn light_right(frame: &[u8], from: u32, to: u32) -> u32 {
    let row = H / 2 * W;
    let pixel = |x: u32| ((row + W / 2 + x) * 4) as usize;
    (from..to)
        .map(|x| (0..3).map(|c| frame[pixel(x) + c] as u32).sum::<u32>())
        .sum()
}

/// A frame with bloom off, the baseline the glow is measured over (the tonemapper
/// lifts the black wall a little).
fn unbloomed(r: &mut Renderer) -> Vec<u8> {
    shot(r, &scene(40.0, |vc| vc.bloom.active = false), (W, H))
}

#[test]
fn gpu_a_small_lamp_glows_far_past_the_old_reach() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let off = unbloomed(&mut r);
    let on = shot(&mut r, &scene(40.0, |_| ()), (W, H));
    // The old blur reached ±4 full-resolution texels past the dot's few pixels.
    let (far_off, far_on) = (light_right(&off, 16, 40), light_right(&on, 16, 40));
    eprintln!("[bloom] 16..40 px right of the lamp: off {far_off}, on {far_on}");
    assert!(far_on > far_off + 200, "the glow reaches 16..40 px out");
}

#[test]
fn gpu_scatter_spreads_the_glow_wider() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let base = light_right(&unbloomed(&mut r), 24, 64);
    let tight = shot(&mut r, &scene(40.0, |vc| vc.bloom.scatter = 0.0), (W, H));
    let wide = shot(&mut r, &scene(40.0, |vc| vc.bloom.scatter = 1.0), (W, H));
    let glow = |frame: &[u8]| light_right(frame, 24, 64).saturating_sub(base);
    let (tight, wide) = (glow(&tight), glow(&wide));
    eprintln!("[bloom] glow 24..64 px out: scatter 0 {tight}, scatter 1 {wide}");
    assert!(wide > tight * 2, "{tight} -> {wide}");
}

#[test]
fn gpu_views_too_small_for_a_chain_render_no_darker_than_without_bloom() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let sum = |frame: &[u8]| frame.iter().map(|&c| c as u32).sum::<u32>();
    for size in [(1, 1), (3, 2), (2, 300), (9, 9)] {
        let on = shot(&mut r, &scene(40.0, |_| ()), size);
        let off = shot(&mut r, &scene(40.0, |vc| vc.bloom.active = false), size);
        assert_eq!(on.len(), (size.0 * size.1 * 4) as usize, "{size:?}");
        // Bloom only adds light: a one-mip chain or a one-texel view never darkens.
        assert!(
            sum(&on) >= sum(&off),
            "{size:?}: {} < {}",
            sum(&on),
            sum(&off)
        );
    }
}
