//! Test fixture for the layout-group tests (#421): a 1920×1080 canvas at scale 1
//! holding one centred panel, with helpers to add a group and sized children.

use glam::Vec2;

use super::UiLayout;
use crate::components::{LayoutElementComponent, LayoutGroupComponent, RectTransformComponent};
use crate::scene::Scene;

pub(super) const HD: Vec2 = Vec2::new(1920.0, 1080.0);

/// A canvas and a `size` panel centred on it (its bottom-left is returned too).
pub(super) struct Fixture {
    pub scene: Scene,
    pub panel: u32,
    pub origin: Vec2,
}

impl Fixture {
    pub fn new(size: Vec2, group: LayoutGroupComponent) -> Self {
        let mut scene = Scene::new();
        let root = scene.add_entity("Canvas".to_string());
        scene.world.set_canvas(root, Some(Default::default()));
        let panel = scene.add_entity("Panel".to_string());
        let rt = RectTransformComponent {
            size_delta: size,
            ..RectTransformComponent::default()
        };
        scene.world.set_rect_transform(panel, Some(rt));
        scene.world.set_layout_group(panel, Some(group));
        scene.set_parent(panel, Some(root)).expect("parent exists");
        let origin = HD * 0.5 - size * 0.5;
        Self {
            scene,
            panel,
            origin,
        }
    }

    /// A child of `parent` with an optional LayoutElement and a `size` size delta.
    pub fn child(&mut self, parent: u32, le: Option<LayoutElementComponent>, size: Vec2) -> u32 {
        let id = self.scene.add_entity("Child".to_string());
        let rt = RectTransformComponent {
            size_delta: size,
            ..RectTransformComponent::default()
        };
        self.scene.world.set_rect_transform(id, Some(rt));
        self.scene.world.set_layout_element(id, le);
        self.scene
            .set_parent(id, Some(parent))
            .expect("parent exists");
        id
    }

    /// `id`'s laid-out `(min, size)` relative to the panel's bottom-left.
    pub fn rect(&self, id: u32) -> (Vec2, Vec2) {
        let layout = UiLayout::compute(&self.scene.world, HD);
        let r = layout.get(id).expect("laid out").rect;
        (r.0 - self.origin, r.1)
    }
}

/// A LayoutElement with preferred sizes (`None` axes left unset).
pub(super) fn preferred(w: Option<f32>, h: Option<f32>) -> Option<LayoutElementComponent> {
    Some(LayoutElementComponent {
        preferred_width: w,
        preferred_height: h,
        ..LayoutElementComponent::default()
    })
}

pub(super) fn assert_rect(got: (Vec2, Vec2), min: [f32; 2], size: [f32; 2]) {
    let ok = (got.0 - Vec2::from(min)).abs().max_element() < 1e-3
        && (got.1 - Vec2::from(size)).abs().max_element() < 1e-3;
    assert!(ok, "got {got:?}, want min {min:?} size {size:?}");
}
