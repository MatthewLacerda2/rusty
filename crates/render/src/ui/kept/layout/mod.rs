mod held;
mod markers;
mod watch;

use std::rc::Rc;

use glam::Vec2;

use held::Elements;
use markers::Markers;
use rusty_ecs::World;
use rusty_ui::{CanvasSpace, TextureSizes, UiLayout, UiView};
use watch::{Watch, Written};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    Rebuild,
    Screen,
    Textures,
    Structure,
    Content,
    Transform,
    Marker,
}

impl Why {
    /// Listed in the order of `stats::pairs::UI_RELAID`'s counter names: change both together.
    pub(crate) const ALL: [Why; 7] = [
        Why::Rebuild,
        Why::Screen,
        Why::Textures,
        Why::Structure,
        Why::Content,
        Why::Transform,
        Why::Marker,
    ];
}

type CanvasKey = (u32, Option<(i32, bool, Vec2, f32)>);

struct Held {
    layout: Rc<UiLayout>,
    screen: Vec2,
    camera: bool,
    canvases: Vec<CanvasKey>,
    textures: Rc<TextureSizes>,
    elements: Elements,
    markers: Option<Markers>,
    placed: bool,
}

#[derive(Default)]
pub(crate) struct KeptLayout {
    watch: Option<Watch>,
    held: Option<Held>,
    why: Option<Why>,
}

impl KeptLayout {
    pub(crate) fn update(&mut self, world: &World, view: &UiView) -> Rc<UiLayout> {
        let written = match &mut self.watch {
            Some(watch) => watch.read(world),
            None => {
                self.watch = Some(Watch::open(world));
                None
            }
        };
        self.why = match (&self.held, &written) {
            (Some(held), Some(written)) => held.why(world, view, written),
            _ => Some(Why::Rebuild),
        };
        match (&mut self.held, self.why) {
            (Some(held), None) => {
                // Kept, so the new table is equal: hold it, and later frames compare pointers, not
                // maps.
                if !Rc::ptr_eq(&held.textures, &view.textures) {
                    held.textures = Rc::clone(&view.textures);
                }
                if held.placed || written.is_some_and(|w| w.canvas) {
                    Rc::make_mut(&mut held.layout).respace(world, view);
                    held.placed = placed(&held.layout);
                }
                Rc::clone(&held.layout)
            }
            _ => self.compute(world, view),
        }
    }

    pub(crate) fn why(&self) -> Option<Why> {
        self.why
    }

    fn compute(&mut self, world: &World, view: &UiView) -> Rc<UiLayout> {
        let layout = Rc::new(UiLayout::compute_in(world, view));
        let markers = view
            .camera
            .as_ref()
            .and_then(|cam| Markers::read(world, &layout, cam, view.screen));
        let placed = placed(&layout);
        self.held = Some(Held {
            layout: Rc::clone(&layout),
            screen: view.screen,
            camera: view.camera.is_some(),
            canvases: canvas_keys(world, view.screen),
            textures: Rc::clone(&view.textures),
            elements: Elements::read(world, &layout),
            markers,
            placed,
        });
        layout
    }
}

impl Held {
    fn why(&self, world: &World, view: &UiView, w: &Written) -> Option<Why> {
        let e = &self.elements;
        if self.screen != view.screen || self.camera != view.camera.is_some() {
            return Some(Why::Screen);
        }
        if !Rc::ptr_eq(&self.textures, &view.textures) && *self.textures != *view.textures {
            return Some(Why::Textures);
        }
        let canvas_moved = w.activity.iter().any(|&id| e.was_canvas(id));
        if (w.canvas || canvas_moved) && self.canvases != canvas_keys(world, view.screen) {
            return Some(Why::Screen);
        }
        // A RectTransform write or an activation may join a laid-out parent, so it counts
        // there; a re-parent stamps the parent's `Children`, so the rest count only held.
        let joins = |&id: &u32| e.moved(world, id, true);
        let held_only = |&id: &u32| e.holds(id).then(|| e.moved(world, id, false)).flatten();
        let why = (w.rect.iter())
            .find_map(joins)
            .or_else(|| w.hierarchy.iter().find_map(held_only))
            .or_else(|| w.activity.iter().find_map(joins))
            .or_else(|| w.content.iter().find_map(held_only));
        if why.is_some() {
            return why;
        }
        if !e.spins_still(world) {
            return Some(Why::Transform);
        }
        match (&self.markers, &view.camera) {
            (Some(m), Some(cam)) if !m.still(world, cam, view.screen) => Some(Why::Marker),
            _ => None,
        }
    }
}

fn placed(layout: &UiLayout) -> bool {
    let root = |(id, r): (u32, &rusty_ui::UiRect)| r.canvas == id;
    layout
        .iter()
        .filter(|&e| root(e))
        .any(|(id, _)| layout.space(id) != CanvasSpace::Screen)
}

fn canvas_keys(world: &World, screen: Vec2) -> Vec<CanvasKey> {
    let ids = world.ids_with_canvas().into_iter();
    let key = |id: u32| {
        let c = world.canvas(id)?;
        if world.ancestors(id).any(|a| world.has_canvas(a)) {
            return Some((id, None));
        }
        let space = c.is_world_space();
        let fields = (c.sort_order, space, c.size(screen), c.scale_factor(screen));
        Some((id, Some(fields)))
    };
    ids.filter_map(key).collect()
}

#[cfg(test)]
mod tests;
