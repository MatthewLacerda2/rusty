//! crates/render/src/ui/kept/layout/ — each view's UI layout, kept until what
//! `UiLayout::compute_in` reads changes (#2412). GPU-free.
//!
//! The layout's rects are a function of the screen size, the texture-size table,
//! which Canvases are roots and what each root's Canvas gives it (`sort_order`,
//! world space or not, its size and scale factor on this screen; a nested Canvas's
//! fields are never read), and, per element it lays out (`held`): its parent and
//! child order, its active flag, its `RectTransform`, `LayoutGroup` and
//! `LayoutElement`, its Transform's rotation and scale (a non-root element), the
//! `Image` and `Text` its content sizes it by (it has a `LayoutElement` or its parent
//! a `LayoutGroup`), and, while a `WorldAnchor` marker is placed (active, under a
//! screen-space root seen through a camera, `markers`), the camera's projection and
//! its targets' world positions.
//!
//! **A stamp selects, a value decides.** The World's change log names the ids
//! written since the last frame (`watch`; a channel nothing stamped is skipped
//! without collecting ids); a written id the layout laid out is read again and its
//! values compared with the held ones, and one it did not lay out counts only when it
//! joins a laid-out parent. A write of the same value, whoever makes it, costs a
//! comparison. What has no channel to read, or is read whole, is compared each
//! frame: the screen, the camera's presence, the texture table, every non-root
//! element's rotation and scale (a Transform cursor would have every Transform write
//! of the sim recorded, for a handful of UI elements), and the markers' camera and
//! targets. The Canvases are compared only on a frame a Canvas was written or one's
//! hierarchy moved.
//!
//! **What the layout does not read never recomputes it**: the Canvas's placement
//! (`sway` and its lag, `tilt`, `plane_distance`, `pixels_per_unit`), a root's own
//! Transform, and the camera while no marker is placed only move where the finished
//! rects land, the canvas *spaces*, placed again on a kept frame
//! ([`UiLayout::respace`]) when a root is not an overlay or a Canvas was written. So
//! a visor HUD swaying with a turning camera, or a world canvas carried through the
//! level, lays out nothing; a marker HUD lays out on every frame the camera or a
//! target moves, since that moves the marker's rect.
//!
//! **Not drawn, not kept exact.** An element that is not active in the hierarchy
//! draws nothing, so an inactive marker reads no camera: its rect (and its
//! children's) may lag the camera until it is activated, which lays out again.
//! Every rect the renderer draws equals a fresh `compute_in`'s.
//!
//! **Whole recomputes**: the first frame, `Changes::Rebuild` on any channel (another
//! World, Play and Stop, a load, an Undo or Redo, a dropped log), or any compared
//! value moved. [`Why`] names the reason, one `Debug.Stats` counter each
//! (`ui_relaid_<reason>`).

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

/// Why a frame computed its layout: the first check that failed, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    /// The first frame, or a channel answered `Changes::Rebuild`.
    Rebuild,
    /// The screen size, the camera's presence, or a Canvas the layout reads.
    Screen,
    /// The texture-size table.
    Textures,
    /// An element's RectTransform, LayoutGroup, LayoutElement, parent, children or
    /// active flag; or one joining.
    Structure,
    /// The Image or Text an element is sized by.
    Content,
    /// A non-root element's Transform rotation or scale.
    Transform,
    /// A placed marker's camera or target.
    Marker,
}

impl Why {
    /// Every reason, in the order of [`crate::RenderCounters::ui_relaid`].
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

/// What a Canvas gives the layout: its id and, for a root (no Canvas above it,
/// which `compute_in` lays out), its `sort_order`, world space, size and scale
/// factor on this screen. A nested canvas lays out as part of its root, which
/// reads none of its fields: only its id and that it is no root are compared.
type CanvasKey = (u32, Option<(i32, bool, Vec2, f32)>);

/// The layout a view last computed and what it was computed from.
struct Held {
    layout: Rc<UiLayout>,
    screen: Vec2,
    camera: bool,
    canvases: Vec<CanvasKey>,
    textures: Rc<TextureSizes>,
    elements: Elements,
    markers: Option<Markers>,
    /// Whether a root's space follows anything but the screen (a world or camera
    /// canvas), so a kept frame places the spaces again.
    placed: bool,
}

/// A view's kept UI layout (the module docs).
#[derive(Default)]
pub(crate) struct KeptLayout {
    watch: Option<Watch>,
    held: Option<Held>,
    /// Why the last [`Self::update`] computed the layout; `None` when it kept it.
    why: Option<Why>,
}

impl KeptLayout {
    /// This frame's layout of `world` through `view`: the held one, its spaces
    /// placed again, while nothing it read changed; else computed afresh.
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
                if held.placed || written.is_some_and(|w| w.canvas) {
                    Rc::make_mut(&mut held.layout).respace(world, view);
                    held.placed = placed(&held.layout);
                }
                Rc::clone(&held.layout)
            }
            _ => self.compute(world, view),
        }
    }

    /// Why the last [`Self::update`] computed the layout; `None` when it kept it.
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
    /// Why the held rects are not what `compute_in` lays out now, if they are not:
    /// the plain comparisons first, the written ids next, the Transforms and the
    /// markers last.
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
        let held_only = |&id: &u32| e.holds(id).then(|| e.moved(world, id, false)).flatten();
        let why = (w.rect.iter())
            .find_map(|&id| e.moved(world, id, true))
            .or_else(|| w.hierarchy.iter().find_map(held_only))
            .or_else(|| w.activity.iter().find_map(held_only))
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

/// Whether a root of `layout` is placed by more than the screen (a world or camera
/// canvas): its space moves with the camera or its Transform.
fn placed(layout: &UiLayout) -> bool {
    let root = |(id, r): (u32, &rusty_ui::UiRect)| r.canvas == id;
    layout
        .iter()
        .filter(|&e| root(e))
        .any(|(id, _)| layout.space(id) != CanvasSpace::Screen)
}

/// Every Canvas's [`CanvasKey`] on `screen`, in the World's order (the layout's
/// tie-break between equal sort orders).
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
