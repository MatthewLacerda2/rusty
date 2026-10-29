//! What the SSR GPU tests (#1526, #1542) share: a camera 1.5 m up looking down -Z
//! over a floor, a cube standing on it 6 m out, and the pixel of the floor whose
//! mirror ray climbs into the cube.

use glam::{Vec2, Vec3};

use crate::components::{MaterialAsset, MaterialComponent, PostProcessingComponent, Ssr};
use crate::core::settings::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 72;

/// The floor point 4 m out, whose mirror ray (from the eye) meets the cube's face
/// 0.56 m up.
pub(super) const REFLECTING: Vec3 = Vec3::new(0.0, 0.0, -4.0);

/// A headless renderer on the High tier (the one SSR runs on), or `None`.
pub(super) fn renderer() -> Option<Renderer> {
    let mut r = crate::render::test_gpu::headless_or_skip(W, H)?;
    r.quality = QualityPreset::High;
    Some(r)
}

/// The eye: 1.5 m up, looking down -Z.
pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.5, 0.0), -90.0, 0.0)
}

/// Put `id` at `position`, scaled by `scale`.
pub(super) fn place(scene: &mut Scene, id: u32, position: Vec3, scale: Vec3) {
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
}

/// A material named `name` of `base_color`, `metallic` and `roughness`.
pub(super) fn paint(scene: &mut Scene, id: u32, name: &str, surface: Surface) {
    let (base_color, metallic, roughness) = surface;
    let asset = MaterialAsset {
        base_color,
        metallic,
        roughness,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.into(), asset);
    let material = Some(MaterialComponent::new(name.to_string()));
    scene.world.set_material(id, material);
}

/// The scene with SSR `on`, the floor painted `floor` (base colour, metallic,
/// roughness) and the cube `cube`. No skybox: the procedural sky. Returns it and
/// the base camera's entity.
pub(super) fn scene(on: bool, floor: Surface, cube: Surface) -> (Scene, u32) {
    tuned(on, floor, cube, |_| {})
}

/// A material: base colour, metallic, roughness.
pub(super) type Surface = ([f32; 3], f32, f32);

/// [`scene`], its volume then changed by `tune` (SSAO on, say).
pub(super) fn tuned(
    on: bool,
    floor: Surface,
    cube: Surface,
    tune: impl FnOnce(&mut PostProcessingComponent),
) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let mut volume: PostProcessingComponent = default_post_processing();
    volume.ssr = Ssr {
        active: on,
        ..Default::default()
    };
    (volume.bloom.active, volume.ssao.active) = (false, false);
    tune(&mut volume);
    scene.world.set_post_processing(cam, Some(volume));
    let ground = create_entity(&mut scene, "Floor", Some(Primitive::Box));
    place(
        &mut scene,
        ground,
        Vec3::new(0.0, -0.05, -10.0),
        Vec3::new(20.0, 0.1, 30.0),
    );
    paint(&mut scene, ground, "floor", floor);
    let block = create_entity(&mut scene, "Cube", Some(Primitive::Box));
    scene.world.transform_mut(block).unwrap().position = Vec3::new(0.0, 0.5, -6.0);
    paint(&mut scene, block, "cube", cube);
    (scene, cam)
}

/// A mirror: white, metallic, as smooth as the forward pass allows.
pub(super) const MIRROR: Surface = ([0.9, 0.9, 0.9], 1.0, 0.0);
/// A fully rough white floor.
pub(super) const ROUGH: Surface = ([0.9, 0.9, 0.9], 0.0, 1.0);
/// A bright red, rough cube.
pub(super) const RED: Surface = ([1.0, 0.1, 0.1], 0.0, 1.0);

/// Render `scene` through [`eye`] into a fresh view and read the frame (RGBA8).
pub(super) fn frame(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
    draw(r, &mut view, scene, &eye())
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

/// The pixel world point `p` lands on through [`eye`].
pub(super) fn pixel(p: Vec3) -> (u32, u32) {
    let clip = eye().build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc: Vec2 = clip.truncate().truncate() / clip.w;
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as u32;
    (x.min(W - 1), y.min(H - 1))
}

/// Pixel `(x, y)`'s RGB.
pub(super) fn rgb(frame: &[u8], (x, y): (u32, u32)) -> [i32; 3] {
    let i = ((y * W + x) * 4) as usize;
    [0, 1, 2].map(|c| i32::from(frame[i + c]))
}

/// Pixel `p`'s linear RGB.
pub(super) fn linear(frame: &[u8], p: (u32, u32)) -> [f32; 3] {
    rgb(frame, p).map(|c| crate::core::color::srgb_to_linear(c as u8))
}
