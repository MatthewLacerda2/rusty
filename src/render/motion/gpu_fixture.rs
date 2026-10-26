//! What the motion-vector GPU tests (#1435) share: a renderer on a tier that runs
//! motion blur, a scene whose base camera has it on, and the velocity target read
//! back as NDC vectors beside each pixel's view depth (#1436).

use glam::{Quat, Vec2, Vec3, Vec3Swizzles};

use crate::components::CameraComponent;
use crate::core::settings::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(crate) const W: u32 = 64;
pub(crate) const H: u32 = 48;

/// A headless renderer on the High tier, or `None` without an adapter.
pub(crate) fn renderer() -> Option<Renderer> {
    let mut r = crate::render::test_gpu::headless_or_skip(W, H)?;
    r.quality = QualityPreset::High;
    Some(r)
}

/// A view kept across frames: its history is what the tests exercise.
pub(crate) fn view(r: &Renderer) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2)
}

/// A lit scene (procedural sky) with one camera entity, `blur` its motion blur.
pub(crate) fn scene(blur: bool) -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let cam = scene.add_entity("Camera".to_string());
    let component = CameraComponent {
        motion_blur_active: blur,
        ..CameraComponent::default()
    };
    scene.world.set_camera(cam, Some(component));
    (scene, cam)
}

/// A unit box at `at`.
pub(crate) fn cube(scene: &mut Scene, at: Vec3) -> u32 {
    let id = create_entity(scene, "Cube", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = at;
    id
}

/// The camera at the origin looking down -Z, turned `yaw` degrees from it.
pub(crate) fn eye(yaw: f32) -> Camera {
    Camera::new(Vec3::ZERO, -90.0 + yaw, 0.0)
}

/// One velocity target, row-major from the top-left: each texel's NDC motion (xy)
/// and view depth (z).
pub(crate) struct Velocity(Vec<Vec3>);

impl Velocity {
    /// The velocity at pixel `(x, y)`.
    pub(crate) fn at(&self, x: u32, y: u32) -> Vec2 {
        self.0[(y * W + x) as usize].xy()
    }

    /// How far from its camera the surface at pixel `(x, y)` lies.
    pub(crate) fn distance(&self, x: u32, y: u32) -> f32 {
        self.0[(y * W + x) as usize].z
    }

    /// The velocity at the pixel holding NDC point `ndc`.
    pub(crate) fn at_ndc(&self, ndc: Vec2) -> Vec2 {
        let (x, y) = pixel(ndc);
        self.at(x, y)
    }

    pub(crate) fn centre(&self) -> Vec2 {
        self.at(W / 2, H / 2)
    }
}

/// Render `scene` through `cam` into `view` (play mode) and read its velocity
/// target; `None` when the frame wrote none.
pub(crate) fn render(
    r: &mut Renderer,
    view: &mut RenderView,
    scene: &Scene,
    cam: &Camera,
) -> Option<Velocity> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, cam, &out, false);
    let target = view.post_fx.motion.active_target()?;
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, &target.texture, (W, H), 8);
    let half = |b: &[u8]| f16_to_f32(u16::from_le_bytes([b[0], b[1]]));
    let texels = bytes
        .chunks(8)
        .map(|t| Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])));
    Some(Velocity(texels.collect()))
}

/// An IEEE half float's value (normals, subnormals and zero: what the target holds).
fn f16_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((bits >> 10) & 0x1f);
    let frac = f32::from(bits & 0x3ff);
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

/// The pixel holding NDC point `ndc`.
pub(crate) fn pixel(ndc: Vec2) -> (u32, u32) {
    let x = ((ndc.x * 0.5 + 0.5) * W as f32) as u32;
    let y = ((0.5 - ndc.y * 0.5) * H as f32) as u32;
    (x.min(W - 1), y.min(H - 1))
}

/// The NDC point at the centre of pixel `(x, y)`.
pub(crate) fn pixel_centre((x, y): (u32, u32)) -> Vec2 {
    let u = (x as f32 + 0.5) / W as f32;
    let v = (y as f32 + 0.5) / H as f32;
    Vec2::new(u * 2.0 - 1.0, 1.0 - v * 2.0)
}

/// The world direction `cam` sees through NDC point `ndc`.
pub(crate) fn unproject(cam: &Camera, ndc: Vec2) -> Vec3 {
    let inverse = cam.build_view_projection(W as f32 / H as f32).inverse();
    let far = inverse * ndc.extend(1.0).extend(1.0);
    (far.truncate() / far.w - cam.position).normalize()
}

/// Where world point `p` lands in NDC through `cam`.
pub(crate) fn project(cam: &Camera, p: Vec3) -> Vec2 {
    let clip = cam.build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    clip.truncate().truncate() / clip.w
}
