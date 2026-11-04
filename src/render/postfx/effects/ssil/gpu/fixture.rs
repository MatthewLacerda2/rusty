//! What the SSIL GPU tests share: a white floor, a red wall 6 m out and a point
//! light 0.7 m in front of it, seen from 1.5 m up looking down -Z and a little down,
//! with no tonemap, so a frame reads back linear.

use glam::{Vec2, Vec3};

use crate::components::{
    LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent, Ssil, Tonemap,
};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_light, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 72;

/// The floor point the tests read: half a metre in front of the wall, beside the
/// light's pool.
pub(super) const FLOOR: Vec3 = Vec3::new(1.5, 0.0, -5.5);

/// The light's intensity: bright enough to light the wall red, not so bright the
/// floor under it clips white.
pub(super) const LAMP: f32 = 6.0;

pub(super) fn renderer() -> Option<Renderer> {
    crate::render::test_gpu::headless_or_skip(W, H)
}

/// The eye, turned `yaw` degrees off -Z.
pub(super) fn eye(yaw: f32) -> Camera {
    Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0 + yaw, -12.0)
}

fn block(scene: &mut Scene, name: &str, (position, scale): (Vec3, Vec3), color: [f32; 3]) {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    drop(t);
    let asset = MaterialAsset {
        base_color: color,
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(name.to_string())));
}

/// The scene with SSIL `ssil` and the light at `light` intensity (0: none at all,
/// so only the ambient lights it). Returns it and the light's entity.
pub(super) fn scene(ssil: Ssil, light: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_color = Vec3::splat(0.05);
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut volume = default_post_processing();
    (volume.bloom.active, volume.ssao.active, volume.ssr.active) = (false, false, false);
    volume.tonemapping.mode = Tonemap::None;
    volume.ssil = ssil;
    scene.world.set_post_processing(cam, Some(volume));
    let floor = (Vec3::new(0.0, -0.05, -10.0), Vec3::new(20.0, 0.1, 30.0));
    block(&mut scene, "floor", floor, [0.9, 0.9, 0.9]);
    let wall = (Vec3::new(0.0, 1.5, -6.0), Vec3::new(6.0, 3.0, 0.2));
    block(&mut scene, "wall", wall, [1.0, 0.05, 0.05]);
    let lamp = scene.add_entity("Lamp".to_string());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 1.0, -5.3);
    let point = LightComponent {
        light_type: LightType::Point,
        intensity: light,
        range: 4.0,
        cast_shadows: false,
        mode: LightMode::Realtime,
        ..default_light()
    };
    if light > 0.0 {
        scene.world.set_light(lamp, Some(point));
    }
    (scene, lamp)
}

/// SSIL on, at `intensity`, at its defaults otherwise.
pub(super) fn on(intensity: f32) -> Ssil {
    Ssil {
        active: true,
        intensity,
        ..Ssil::default()
    }
}

/// Render `scene` through `cam` into `view` and read the frame (RGBA8).
pub(super) fn draw(
    r: &mut Renderer,
    view: &mut RenderView,
    scene: &Scene,
    cam: &Camera,
) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, cam, &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// A fresh view.
pub(super) fn view(r: &Renderer) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H)
}

/// The last of `frames` frames of `scene` drawn into one fresh view from [`eye`].
pub(super) fn settled(r: &mut Renderer, scene: &Scene, frames: u32) -> Vec<u8> {
    let mut v = view(r);
    let mut last = Vec::new();
    for _ in 0..frames {
        last = draw(r, &mut v, scene, &eye(0.0));
    }
    last
}

/// The pixel world point `p` lands on through [`eye`].
pub(super) fn pixel(p: Vec3) -> (u32, u32) {
    pixel_at(p, 0.0)
}

/// The pixel world point `p` lands on through [`eye`] turned `yaw` degrees.
pub(super) fn pixel_at(p: Vec3, yaw: f32) -> (u32, u32) {
    let clip = eye(yaw).build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as u32;
    (x.min(W - 1), y.min(H - 1))
}

/// The summed linear RGB of the 5x3 patch around `p`.
pub(super) fn patch(frame: &[u8], p: Vec3) -> [f32; 3] {
    patch_at(frame, p, 0.0)
}

/// [`patch`] through [`eye`] turned `yaw` degrees.
pub(super) fn patch_at(frame: &[u8], p: Vec3, yaw: f32) -> [f32; 3] {
    let (x, y) = pixel_at(p, yaw);
    let mut sum = [0.0; 3];
    for y in y.saturating_sub(1)..(y + 2).min(H) {
        for x in x.saturating_sub(2)..(x + 3).min(W) {
            let i = ((y * W + x) * 4) as usize;
            for c in 0..3 {
                sum[c] += crate::core::color::srgb_to_linear(frame[i + c]);
            }
        }
    }
    sum
}

/// The whole frame's summed linear light.
pub(super) fn energy(frame: &[u8]) -> f32 {
    let lin = |c: &u8| crate::core::color::srgb_to_linear(*c);
    frame
        .chunks_exact(4)
        .flat_map(|px| px[..3].iter().map(lin))
        .sum()
}
