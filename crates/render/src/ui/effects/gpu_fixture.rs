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

/// An overlay canvas at one unit a pixel; a RectMask panel holding an ellipse
/// Mask (a Shape, hidden) over a "map" Image. Returns (scene, panel, mask, map).
pub(super) fn minimap() -> (Scene, u32, u32, u32) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    let panel = element(&mut scene, root, [0.0, 0.0, 64.0, 64.0]);
    scene
        .world
        .set_rect_mask(panel, Some(RectMaskComponent::default()));
    let mask = element(&mut scene, panel, [8.0, 8.0, 48.0, 48.0]);
    scene.world.set_image(mask, None);
    let ellipse = ShapeComponent {
        kind: ShapeKind::Ellipse,
        ..Default::default()
    };
    scene.world.set_shape(mask, Some(ellipse));
    let hidden = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(mask, Some(hidden));
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
