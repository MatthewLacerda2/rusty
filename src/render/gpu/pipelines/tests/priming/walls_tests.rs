//! Depth priming (#1499) is invisible: a camera that runs SSAO starts its forward
//! pass from the prepass's depth, and the frame is the same, pixel for pixel, as the
//! unprimed pass, with motion vectors on or off. The scene holds every case the
//! prepass draws differently from the forward pass: walls given back to front, an
//! unlit solid (absent from the prepass) half hidden behind a lit one, and a
//! half-dissolved surface variant (a cut, #648) in front, and a render scale below 1
//! (#1498), whose prepass and view depth are both the scaled size. On Metal nothing
//! primes, so there it checks only that. Skips with no adapter.

use std::cell::RefCell;

use glam::{Quat, Vec3};

use super::cut_depth_tests::dissolve;
use super::mask_tests::Dissolve;
use crate::components::{CameraComponent, MaterialAsset, MaterialComponent};
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::render_scale::RenderScale;
use crate::render::{readback, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::Primitive;
use crate::scene::authoring::{create_entity, default_light, default_post_processing, material};
use crate::scene::{Camera, Scene};

const W: u32 = 288;
const H: u32 = 192;

/// A box at `position` scaled by `scale`, coloured `rgb`.
fn block(scene: &mut Scene, position: Vec3, scale: Vec3, rgb: [f32; 3]) -> u32 {
    let id = create_entity(scene, "Block", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    let name = format!("{rgb:?}");
    let colour = MaterialAsset {
        base_color: rgb,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.clone(), colour);
    let component = MaterialComponent::new(name);
    scene.world.set_material(id, Some(component));
    id
}

/// Layered walls under an AO volume whose camera has motion blur `blur`, and the
/// dissolving wall in front, drawn by `d`'s shader.
pub(super) fn walls(d: &Dissolve, blur: bool) -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let floor = (Vec3::new(0.0, -0.5, -8.0), Vec3::new(30.0, 1.0, 30.0));
    block(&mut scene, floor.0, floor.1, [0.3; 3]);
    let rgb = [[0.1, 0.2, 0.8], [0.1, 0.7, 0.2], [0.8, 0.6, 0.1]];
    for ((x, z), rgb) in [(0.0, -14.0), (-1.5, -10.0), (1.5, -7.0)]
        .into_iter()
        .zip(rgb)
    {
        block(
            &mut scene,
            Vec3::new(x, 1.5, z),
            Vec3::new(4.0, 3.0, 0.3),
            rgb,
        );
    }
    // A light's own mesh draws unlit, so the prepass skips it (#436): in front of
    // the green wall, which must not paint over it.
    let unlit = block(
        &mut scene,
        Vec3::new(-2.2, 1.0, -8.5),
        Vec3::splat(1.2),
        [1.0, 0.0, 0.0],
    );
    let light = crate::components::LightComponent {
        intensity: 0.0,
        ..default_light()
    };
    scene.world.set_light(unlit, Some(light));
    let front = (Vec3::new(0.0, 1.0, -4.0), Vec3::new(3.0, 2.0, 0.2));
    let cut = block(&mut scene, front.0, front.1, [0.9; 3]);
    let key = material::ensure_material_key(&mut scene, cut).unwrap();
    material::set_shader(&mut scene.materials, &key, d.shader.clone());

    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = true;
    scene.world.set_post_processing(volume, Some(vc));
    let lens = CameraComponent {
        motion_blur_active: blur,
        ..CameraComponent::default()
    };
    scene.world.set_camera(volume, Some(lens));
    (RefCell::new(scene), cut)
}

/// One frame of `scene` at render scale `scale`, primed or not: pixels and counters.
fn frame(r: &mut Renderer, scene: &Scene, primed: bool, scale: f32) -> (Vec<u8>, RenderCounters) {
    r.depth_priming = primed;
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    view.set_render_scale(RenderScale::new(scale));
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 3.0), -90.0, -5.0);
    r.render(&mut view, scene, &cam, &out, false);
    let target = view.color_target().unwrap();
    let px = readback::read_texture_rgba8(&r.device, &r.queue, target, W, H);
    (px, r.frame_counters)
}

#[test]
fn gpu_a_primed_forward_pass_draws_the_unprimed_frame() {
    // Black (cut) in the middle half of u, so a slot opens through both faces.
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| {
        let cut = x == 1 || x == 2;
        image::Rgba([if cut { 0 } else { 255 }; 4])
    });
    let d = Dissolve::with_mask("primed", &mask);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    r.set_quality(QualityPreset::High);
    for (blur, scale) in [(false, 1.0), (true, 1.0), (true, 0.5)] {
        let (scene, cut) = walls(&d, blur);
        dissolve(&scene, cut, &d, 0.5);
        let (plain, before) = frame(&mut r, &scene.borrow(), false, scale);
        let (primed, after) = frame(&mut r, &scene.borrow(), true, scale);
        assert_eq!(before.primed_passes, 0);
        assert_eq!(after.velocity_passes, u32::from(blur));
        // Metal never primes (`priming`), and there two unprimed frames already
        // differ by a level on a few edge pixels (CI, 2026-10-28): nothing to compare.
        if !r.priming_recommended {
            assert_eq!(after.primed_passes, 0, "Metal doesn't prime");
            continue;
        }
        assert_eq!(after.primed_passes, 1, "the AO camera primes");
        let distinct: std::collections::HashSet<_> = plain.chunks(4).collect();
        assert!(distinct.len() > 8, "the frame shows the walls");
        let differing = plain
            .chunks(4)
            .zip(primed.chunks(4))
            .filter(|(a, b)| a != b);
        assert_eq!(
            differing.count(),
            0,
            "priming changed pixels ({blur}, {scale})"
        );
    }
}
