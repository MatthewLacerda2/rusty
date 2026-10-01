//! Scene builders for the soft/shaped mask and backdrop screenshots (#426, #428):
//! a 128-unit overlay canvas at one unit per pixel, placed UI elements, a soft
//! circle sprite written to disk, and the minimap's render-texture camera.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{
    CanvasComponent, ImageComponent, MaskComponent, RectTransformComponent, RenderTarget,
};
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::{Camera, Scene};

pub const SIZE: u32 = 128;

/// An overlay canvas at one reference unit per pixel on a `SIZE` shot.
pub fn overlay(scene: &mut Scene) -> u32 {
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    root
}

/// A solid `color` Image under `parent` at `rect` = `[x, y, w, h]` (bottom-left
/// anchored, reference units = pixels).
pub fn element(scene: &mut Scene, parent: u32, rect: [f32; 4], color: Vec4) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::new(rect[0], rect[1]),
        size_delta: Vec2::new(rect[2], rect[3]),
        world_anchor: None,
    };
    scene.world.set_rect_transform(id, Some(rt));
    let image = ImageComponent {
        color,
        ..Default::default()
    };
    scene.world.set_image(id, Some(image));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

/// A white disc filling a `size`² sprite whose alpha falls from 1 to 0 over its
/// outer `soft` texels — written once per test process, returned as its path.
pub fn soft_circle(size: u32, soft: f32) -> String {
    let path = crate::temp::dir().join(format!("rusty_circle_{size}_{soft}.png"));
    let r = size as f32 / 2.0;
    let img = image::RgbaImage::from_fn(size, size, |x, y| {
        let d = Vec2::new(x as f32 + 0.5 - r, y as f32 + 0.5 - r).length();
        let a = ((r - d) / soft).clamp(0.0, 1.0);
        image::Rgba([255, 255, 255, (a * 255.0).round() as u8])
    });
    img.save(&path).expect("write the circle sprite");
    path.to_string_lossy().into_owned()
}

/// Make `id` a Mask whose graphic only shapes the clip.
pub fn hidden_mask(scene: &mut Scene, id: u32) {
    let mask = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(id, Some(mask));
}

/// A camera 0.5 m from a red box's face, so its whole `"rt:minimap"` is red,
/// beside a screen camera looking away (`screen_camera`). Built on the
/// render-texture screenshot's scene, with its own picture-in-picture removed.
pub fn minimap_world() -> Scene {
    use super::render_texture_scene::pip_scene;
    let mut scene = pip_scene(Some(RenderTarget::new("minimap", 64, 64)));
    for name in ["Canvas", "Pip"] {
        let id = scene.find_entity_by_name(name).expect("pip scene entity");
        scene.world.set_active(id, false);
    }
    let cam = scene.find_entity_by_name("PipCamera").expect("pip camera");
    scene.world.transform_mut(cam).expect("transform").position = Vec3::new(0.0, 0.0, 1.5);
    scene
}

/// Render `scene` from `cam` at `SIZE`²: the image (PNG rows top-down) and the
/// frame's counters, or `None` without an adapter.
pub fn shoot(
    scene: &Scene,
    cam: &Camera,
    name: &str,
) -> Option<(image::RgbImage, rusty::render::RenderCounters)> {
    let path = crate::temp::dir().join(name);
    let mut host = CaptureHost::new();
    let shot = capture_into(&mut host, scene, cam, &path, SIZE, SIZE);
    if !shot.expect("capture must not error") {
        eprintln!("[ui masks] no GPU/software adapter — skipping");
        return None;
    }
    let img = image::open(&path).expect("png").to_rgb8();
    let (counters, _) = host.last_frame.expect("a frame was drawn");
    Some((img, counters))
}
