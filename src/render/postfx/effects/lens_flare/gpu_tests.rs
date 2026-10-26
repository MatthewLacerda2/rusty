//! Lens flare on lavapipe (#1443): a bright dot left of the centre throws a ghost
//! on the right, mirrored through the centre, on Medium and High; a scene with
//! nothing above bloom's threshold is byte-identical with the flare on and off.

use glam::Vec3;

use crate::components::{MaterialAsset, MaterialComponent, PostProcessingComponent};
use crate::core::settings::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 128;
const H: u32 = 96;

/// A black wall filling the view and a small box lit by its own `emissive`, a
/// quarter of the width left of the centre; the camera's volume blooms, and
/// flares when `flare` holds.
fn scene(emissive: f32, flare: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = 0.0;
    let wall = glowing(&mut scene, "Wall", Vec3::new(0.0, 0.0, -6.0), 0.0);
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(60.0, 60.0, 0.2);
    let dot = glowing(&mut scene, "Dot", dot_position(), emissive);
    scene.world.transform_mut(dot).unwrap().scale = Vec3::splat(0.15);
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut vc: PostProcessingComponent = default_post_processing();
    (vc.ssr.active, vc.ssao.active) = (false, false);
    vc.lens_flare.active = flare;
    scene.world.set_post_processing(cam, Some(vc));
    scene
}

/// The dot's centre: 3 m out, left of the centre.
fn dot_position() -> Vec3 {
    Vec3::new(-1.0, 0.0, -3.0)
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
    let material = Some(MaterialComponent::new(name.to_string()));
    scene.world.set_material(id, material);
    id
}

fn eye() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

fn renderer(tier: QualityPreset) -> Option<Renderer> {
    let mut r = crate::render::test_gpu::headless_or_skip(W, H)?;
    r.quality = tier;
    Some(r)
}

/// One frame of `scene` through a fresh view (RGBA8).
fn shot(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, &eye(), &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// The pixel the world point `p` lands on.
fn pixel_of(p: Vec3) -> (i32, i32) {
    let clip = eye().build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc = clip.truncate().truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as i32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as i32;
    (x, y)
}

/// The summed brightness of the pixels within `reach` of `(cx, cy)`.
fn light_around(frame: &[u8], (cx, cy): (i32, i32), reach: i32) -> u32 {
    let mut sum = 0;
    for y in (cy - reach).max(0)..(cy + reach).min(H as i32) {
        for x in (cx - reach).max(0)..(cx + reach).min(W as i32) {
            let i = ((y as u32 * W + x as u32) * 4) as usize;
            sum += (0..3).map(|c| frame[i + c] as u32).sum::<u32>();
        }
    }
    sum
}

#[test]
fn gpu_a_bright_dot_throws_a_ghost_across_the_centre() {
    for tier in [QualityPreset::Medium, QualityPreset::High] {
        let Some(mut r) = renderer(tier) else { return };
        let off = shot(&mut r, &scene(40.0, false));
        let on = shot(&mut r, &scene(40.0, true));
        let (dx, dy) = pixel_of(dot_position());
        assert!(dx < W as i32 / 2, "the dot is left of the centre ({dx})");
        let mirror = (W as i32 - 1 - dx, H as i32 - 1 - dy);
        let (ghost_off, ghost_on) = (light_around(&off, mirror, 6), light_around(&on, mirror, 6));
        eprintln!("[lens_flare] {tier:?} mirror of the dot: off {ghost_off}, on {ghost_on}");
        assert!(
            ghost_on > ghost_off + 200,
            "{tier:?}: a ghost lands opposite the dot ({ghost_off} -> {ghost_on})"
        );
    }
}

#[test]
fn gpu_without_a_bright_pixel_the_flare_changes_nothing() {
    let Some(mut r) = renderer(QualityPreset::High) else {
        return;
    };
    // Below bloom's threshold (0.8): the bright pass, and so the flare, is zero.
    let off = shot(&mut r, &scene(0.5, false));
    let on = shot(&mut r, &scene(0.5, true));
    assert!(
        light_around(&on, pixel_of(dot_position()), 2) > 0,
        "the dot draws"
    );
    assert!(
        on == off,
        "a flare with nothing bright is byte-identical to none"
    );
}

#[test]
fn gpu_low_holds_no_flare_target() {
    let Some(mut r) = renderer(QualityPreset::Low) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 4);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, &scene(40.0, true), &eye(), &out, false);
    assert!(view.post_fx.targets.get::<super::FlareTargets>().is_none());
}
