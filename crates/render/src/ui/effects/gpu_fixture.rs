//! The GPU fixture the kept-HUD tests (#2354) share: a minimap-shaped HUD and a
//! view rendered frame by frame.

use glam::{Vec2, Vec3};

use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{
    CanvasComponent, ImageComponent, MaskComponent, RectMaskComponent, RectTransformComponent,
    ShapeComponent, ShapeKind,
};
use rusty_scene::{Camera, Scene};

pub(super) const SIZE: u32 = 64;

/// A solid white Image at `[x, y, w, h]` (bottom-left anchored) under `parent`.
pub(super) fn element(scene: &mut Scene, parent: u32, rect: [f32; 4]) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::new(rect[0], rect[1]),
        size_delta: Vec2::new(rect[2], rect[3]),
        ..RectTransformComponent::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.world.set_image(id, Some(ImageComponent::default()));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

/// An empty overlay canvas at one unit a pixel: (scene, its root).
pub(super) fn canvas() -> (Scene, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    (scene, root)
}

/// An overlay canvas at one unit a pixel; a RectMask panel holding an ellipse
/// Mask (a Shape, hidden) over a "map" Image. Returns (scene, panel, mask, map).
pub(super) fn minimap() -> (Scene, u32, u32, u32) {
    let (mut scene, root) = canvas();
    let panel = element(&mut scene, root, [0.0, 0.0, 64.0, 64.0]);
    scene
        .world
        .set_rect_mask(panel, Some(RectMaskComponent::default()));
    let mask = element(&mut scene, panel, [8.0, 8.0, 48.0, 48.0]);
    ellipse(&mut scene, mask);
    hidden_mask(&mut scene, mask);
    let map = element(&mut scene, mask, [0.0, 0.0, 48.0, 48.0]);
    (scene, panel, mask, map)
}

/// Renders frames of one view, reporting each frame's Mask passes and pixels.
pub(super) struct Frames {
    pub(super) view: RenderView,
    out: wgpu::TextureView,
}

impl Frames {
    pub(super) fn new(renderer: &Renderer) -> Self {
        let view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, SIZE, SIZE);
        let out = view.color_target_view().expect("offscreen target");
        Self { view, out }
    }

    /// One frame of `scene`: its Mask passes and its pixels.
    pub(super) fn frame(&mut self, renderer: &mut Renderer, scene: &Scene) -> (u32, Vec<u8>) {
        let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
        renderer.render(&mut self.view, scene, &cam, &self.out, false);
        let size = self.view.size();
        let texture = self.view.color_target().expect("texture");
        let (device, queue) = (&renderer.device, &renderer.queue);
        let img =
            crate::readback::read_texture_rgba8(device, queue, texture, size.width, size.height);
        (self.view.ui.last_mask_passes(), img)
    }
}

/// A 4×4 PNG of one `rgba`, written under the test's temp dir; returns its path.
pub(super) fn texture_file(name: &str, rgba: [u8; 4]) -> String {
    let path = rusty_test_support::temp::dir().join(name);
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba(rgba));
    image.save(&path).expect("write the texture fixture");
    path.to_string_lossy().into_owned()
}

/// Make `id` a Mask whose graphic only shapes the clip.
pub(super) fn hidden_mask(scene: &mut Scene, id: u32) {
    let hidden = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(id, Some(hidden));
}

/// Make `id`'s graphic a white ellipse in place of its Image.
pub(super) fn ellipse(scene: &mut Scene, id: u32) {
    scene.world.set_image(id, None);
    let shape = ShapeComponent {
        kind: ShapeKind::Ellipse,
        ..Default::default()
    };
    scene.world.set_shape(id, Some(shape));
}
