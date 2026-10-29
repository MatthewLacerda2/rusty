//! What the depth-of-field GPU tests (#1445) share: a view large enough for a few
//! pixels of blur, a lit scene whose camera carries a post-processing volume, a near
//! and a far cube side by side, and the finished frame read back.

use glam::{Quat, Vec3};

use crate::components::{
    DepthOfField, DofMode, MaterialAsset, MaterialComponent, PostProcessingComponent, SsaoSettings,
};
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

/// 160 × 120: the largest Bokeh circle is 3 px of radius (1/40 of the height).
pub(super) const W: u32 = 160;
pub(super) const H: u32 = 120;

/// The near cube's centre (2 m out, left) and the far one's (12 m out, right).
pub(super) const NEAR: Vec3 = Vec3::new(-0.45, 0.0, -2.0);
pub(super) const FAR: Vec3 = Vec3::new(2.4, 0.0, -12.0);

/// A headless renderer whose quality preset blurs at `1 / divisor` resolution on
/// every level, or `None` without an adapter.
pub(super) fn renderer(divisor: u32) -> Option<Renderer> {
    let mut r = crate::render::test_gpu::headless_or_skip(W, H)?;
    let d = &mut r.quality.depth_of_field_resolution_divisor;
    (d.low, d.medium, d.high) = (divisor, divisor, divisor);
    Some(r)
}

pub(super) fn view(r: &Renderer) -> RenderView {
    RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H)
}

/// A wide-open 85 mm lens (f/1.4) focused at `focus` metres: both cubes clamp to
/// the largest circle when out of focus.
pub(super) fn bokeh(focus: f32) -> DepthOfField {
    DepthOfField {
        mode: DofMode::Bokeh,
        focus_distance: focus,
        aperture: 1.4,
        focal_length: 85.0,
        ..DepthOfField::default()
    }
}

/// A lit scene with a camera entity whose active volume runs `dof` and nothing
/// else, the two cubes and a wall behind them. Returns the scene and the camera.
pub(super) fn scene(dof: DepthOfField) -> (Scene, u32) {
    let mut scene = Scene::new();
    crate::render::test_gpu::black_sky(&mut scene);
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let cam = scene.add_entity("Camera".to_string());
    scene.world.set_camera(cam, Some(Default::default()));
    let volume = PostProcessingComponent {
        bloom: crate::components::Bloom {
            active: false,
            ..Default::default()
        },
        ssr: crate::components::Ssr {
            active: false,
            ..Default::default()
        },
        ssao: SsaoSettings {
            active: false,
            ..Default::default()
        },
        depth_of_field: dof,
        ..default_post_processing()
    };
    scene.world.set_post_processing(cam, Some(volume));
    let near = cube(&mut scene, NEAR);
    scene.world.transform_mut(near).unwrap().scale = Vec3::splat(0.3);
    let far = cube(&mut scene, FAR);
    scene.world.transform_mut(far).unwrap().scale = Vec3::splat(1.5);
    // A flat, dark wall far behind both, filling the view: blurred, it stays one colour,
    // unlike the sky's clouds, so only a cube's own edges show its blur.
    let wall = cube(&mut scene, Vec3::new(0.0, 0.0, -40.0));
    scene.world.transform_mut(wall).unwrap().scale = Vec3::new(200.0, 200.0, 1.0);
    let dark = MaterialAsset {
        base_color: [0.05, 0.05, 0.3],
        roughness: 1.0,
        ..MaterialAsset::default()
    };
    scene.materials.insert("wall".into(), dark);
    let material = Some(MaterialComponent::new("wall".to_string()));
    scene.world.set_material(wall, material);
    (scene, cam)
}

/// A unit box at `at`.
pub(super) fn cube(scene: &mut Scene, at: Vec3) -> u32 {
    let id = create_entity(scene, "Cube", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position = at;
    id
}

/// The camera at the origin looking down -Z.
pub(super) fn eye() -> Camera {
    Camera::new(Vec3::ZERO, -90.0, 0.0)
}

/// Render `scene` into `view` (play mode) and read the finished frame (RGBA8).
pub(super) fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> Vec<u8> {
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &eye(), &out, false);
    readback::read_texture_rgba8(&r.device, &r.queue, view.color_target().unwrap(), W, H)
}

/// One frame of `scene` through a fresh view.
pub(super) fn shot(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut v = view(r);
    frame(r, &mut v, scene)
}

/// The largest per-channel difference between two frames over the pixels within
/// `reach` of where world point `p` lands: how much the blur changed that cube.
pub(super) fn change_around(a: &[u8], b: &[u8], p: Vec3, reach: i32) -> u8 {
    let clip = eye().build_view_projection(W as f32 / H as f32) * p.extend(1.0);
    let ndc = clip.truncate().truncate() / clip.w;
    let cx = ((ndc.x * 0.5 + 0.5) * W as f32) as i32;
    let cy = ((0.5 - ndc.y * 0.5) * H as f32) as i32;
    let mut most = 0;
    for y in (cy - reach).max(0)..(cy + reach).min(H as i32) {
        for x in (cx - reach).max(0)..(cx + reach).min(W as i32) {
            let i = ((y as u32 * W + x as u32) * 4) as usize;
            let d = (0..3).map(|c| a[i + c].abs_diff(b[i + c])).max().unwrap();
            most = most.max(d);
        }
    }
    most
}
