//! Test fixture for the event system (#420): a 1920×1080 canvas (scale 1, so UI
//! pixels are reference units), image panels placed by their bottom-left corner,
//! a set of script handlers, and a one-call tick.

use std::collections::BTreeSet;

use glam::Vec2;

use super::{Delivery, EventSystem, Frame, UiHook};
use crate::components::{
    CanvasComponent, ImageComponent, RectTransformComponent, SelectableComponent,
};
use crate::core::input::InputState;
use crate::scene::Scene;
use crate::ui::UiLayout;

pub const SCREEN: Vec2 = Vec2::new(1920.0, 1080.0);

/// A scene with one root canvas; returns it and the canvas id.
pub fn scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    let canvas = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(canvas, Some(CanvasComponent::default()));
    (scene, canvas)
}

/// An Image panel under `parent` covering `min .. min + size` (UI pixels).
pub fn panel(scene: &mut Scene, parent: u32, min: Vec2, size: Vec2) -> u32 {
    let id = scene.add_entity("Panel".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: min,
        size_delta: size,
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.world.set_image(id, Some(ImageComponent::default()));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

/// A panel that is also a Selectable.
pub fn button(scene: &mut Scene, parent: u32, min: Vec2, size: Vec2) -> u32 {
    let id = panel(scene, parent, min, size);
    scene
        .world
        .set_selectable(id, Some(SelectableComponent::default()));
    id
}

/// Which `(entity, hook)` pairs have a script handler.
#[derive(Default)]
pub struct Handlers(pub BTreeSet<(u32, UiHook)>);

impl Handlers {
    pub fn on(mut self, id: u32, hooks: &[UiHook]) -> Self {
        self.0.extend(hooks.iter().map(|&h| (id, h)));
        self
    }
}

/// Everything a tick needs, driven like the engine drives it.
pub struct Rig {
    pub scene: Scene,
    pub input: InputState,
    pub events: EventSystem,
    pub handlers: Handlers,
}

impl Rig {
    pub fn new(scene: Scene, handlers: Handlers) -> Self {
        Self {
            scene,
            input: InputState::new(),
            events: EventSystem::default(),
            handlers,
        }
    }

    /// Move the pointer to UI pixel `(x, y)` (bottom-left origin).
    pub fn point(&mut self, x: f32, y: f32) {
        self.input.move_mouse(f64::from(x), f64::from(SCREEN.y - y));
    }

    /// Publish the pending input and run one event-system tick.
    pub fn tick(&mut self) -> Vec<Delivery> {
        self.input.begin_tick();
        let layout = UiLayout::compute(&self.scene.world, SCREEN);
        let handlers = &self.handlers.0;
        let handles = |id: u32, hook: UiHook| handlers.contains(&(id, hook));
        let frame = Frame {
            world: &self.scene.world,
            layout: &layout,
            input: &self.input,
            screen: SCREEN,
            handles: &handles,
        };
        self.events.process(&frame)
    }

    /// [`Rig::tick`], reduced to `(entity, hook)` pairs.
    pub fn hooks(&mut self) -> Vec<(u32, UiHook)> {
        self.tick().iter().map(|d| (d.entity, d.hook)).collect()
    }
}
