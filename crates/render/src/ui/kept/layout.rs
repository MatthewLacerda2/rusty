//! crates/render/src/ui/kept/layout.rs — each view's UI layout, kept until what
//! `UiLayout::compute_in` reads changes (#2412). GPU-free.
//!
//! The layout's rects are a function of the screen size, the texture-size table,
//! what each Canvas gives its root (presence, `sort_order`, world space or not, its
//! size and scale factor on this screen), the hierarchy and active flags, the
//! elements' `RectTransform`, `LayoutGroup` and `LayoutElement`, the rotation and
//! scale of every non-root element's `Transform`, the `Image` and `Text` of an
//! element its content sizes (it has a `LayoutElement` or its parent a
//! `LayoutGroup`), and, while a `WorldAnchor` marker exists under a camera, the
//! camera's projection and its targets' world poses. The values are compared and the
//! writes read from the World's change log ([`Watch`]): a write to an element the
//! layout laid out, or to a child of one (it may join), dirties it.
//!
//! **What the layout does not read never recomputes it**: the Canvas's placement
//! (`sway` and its lag, `tilt`, `plane_distance`, `pixels_per_unit`), a root's own
//! Transform, and the camera while no marker exists only move where the finished
//! rects land, the canvas *spaces*, which are placed again every frame
//! ([`UiLayout::respace`]). So a visor HUD swaying with a turning camera, or a world
//! canvas carried through the level, lays out nothing; a marker HUD lays out on every
//! frame the camera or a target moves, since that moves the marker's rect.
//!
//! **Whole recomputes**: the first frame, `Changes::Rebuild` on any channel (another
//! World, Play and Stop, a load, an Undo or Redo, a dropped log), or any compared
//! value moved.

use std::rc::Rc;

use glam::{Mat4, Vec2, Vec3};

use rusty_components::{
    ImageComponent, LayoutElementComponent, LayoutGroupComponent, RectTransformComponent,
    TextComponent, TransformComponent,
};
use rusty_ecs::{Activity, Changes, Cursor, World, WorldPose};
use rusty_scene::Camera;
use rusty_ui::{TextureSizes, UiLayout, UiView};

/// What a root's Canvas gives its rects: id, `sort_order`, world space, size and
/// scale factor on this screen.
type CanvasKey = (u32, i32, bool, Vec2, f32);

/// What a marker reads of the camera: its view-projection on this screen, and the
/// eye and forward axis its behind test reads.
type Eye = (Mat4, Vec3, Vec3);

/// The cursors of every column the layout reads (the module docs).
struct Watch {
    rect: Cursor<RectTransformComponent>,
    transform: Cursor<TransformComponent>,
    activity: Cursor<Activity>,
    group: Cursor<LayoutGroupComponent>,
    element: Cursor<LayoutElementComponent>,
    image: Cursor<ImageComponent>,
    text: Cursor<TextComponent>,
    /// Open only while a marker exists: a target's world pose moves it.
    pose: Option<Cursor<WorldPose>>,
}

/// The ids each channel wrote since the last read, `None` for a whole recompute.
struct Written {
    /// RectTransform, activity and hierarchy, LayoutGroup and LayoutElement.
    structure: Vec<u32>,
    transform: Vec<u32>,
    content: Vec<u32>,
    pose: Vec<u32>,
}

impl Watch {
    fn open(w: &World) -> Self {
        Self {
            rect: w.change_cursor(),
            transform: w.change_cursor(),
            activity: w.change_cursor(),
            group: w.change_cursor(),
            element: w.change_cursor(),
            image: w.change_cursor(),
            text: w.change_cursor(),
            pose: None,
        }
    }

    /// Every cursor read (each current after), or `None` when one cannot say.
    fn read(&mut self, w: &World) -> Option<Written> {
        let ids = |c: Changes| match c {
            Changes::Rebuild => None,
            Changes::Ids(ids) => Some(ids),
        };
        let all = [
            ids(w.changes(&mut self.rect)),
            ids(w.changes(&mut self.activity)),
            ids(w.changes(&mut self.group)),
            ids(w.changes(&mut self.element)),
            ids(w.changes(&mut self.transform)),
            ids(w.changes(&mut self.image)),
            ids(w.changes(&mut self.text)),
            self.pose
                .as_mut()
                .map_or(Some(Vec::new()), |p| ids(w.changes(p))),
        ];
        let [rect, activity, group, element, transform, image, text, pose] = all;
        let mut structure = rect?;
        structure.extend(activity?.into_iter().chain(group?).chain(element?));
        let mut content = image?;
        content.extend(text?);
        let (transform, pose) = (transform?, pose?);
        Some(Written {
            structure,
            transform,
            content,
            pose,
        })
    }
}

/// The layout a view last computed and what it was computed from.
struct Held {
    layout: Rc<UiLayout>,
    screen: Vec2,
    camera: bool,
    canvases: Vec<CanvasKey>,
    textures: Rc<TextureSizes>,
    /// The markers' targets (sorted) and the camera they were placed through, when
    /// a marker exists under a camera.
    markers: Option<(Vec<u32>, Eye)>,
}

/// A view's kept UI layout (the module docs).
#[derive(Default)]
pub(crate) struct KeptLayout {
    watch: Option<Watch>,
    held: Option<Held>,
    /// Whether the last [`Self::update`] computed the layout (else it kept it).
    computed: bool,
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
        let canvases = canvas_keys(world, view.screen);
        let kept = match (&mut self.held, written) {
            (Some(held), Some(written)) if held.holds(world, view, &canvases, &written) => {
                Rc::make_mut(&mut held.layout).respace(world, view);
                Some(Rc::clone(&held.layout))
            }
            _ => None,
        };
        self.computed = kept.is_none();
        kept.unwrap_or_else(|| self.compute(world, view, canvases))
    }

    /// Whether the last [`Self::update`] computed the layout.
    pub(crate) fn computed(&self) -> bool {
        self.computed
    }

    fn compute(&mut self, world: &World, view: &UiView, canvases: Vec<CanvasKey>) -> Rc<UiLayout> {
        let layout = Rc::new(UiLayout::compute_in(world, view));
        let markers = view.camera.as_ref().and_then(|cam| {
            let targets = marker_targets(world)?;
            Some((targets, eye(cam, view.screen)))
        });
        if let Some(watch) = &mut self.watch {
            // Opened here, it reads from now on: this layout read the poses.
            match &markers {
                Some(_) => _ = watch.pose.get_or_insert_with(|| world.change_cursor()),
                None => watch.pose = None,
            }
        }
        self.held = Some(Held {
            layout: Rc::clone(&layout),
            screen: view.screen,
            camera: view.camera.is_some(),
            canvases,
            textures: Rc::clone(&view.textures),
            markers,
        });
        layout
    }
}

impl Held {
    /// Whether the held rects are what `compute_in` lays out now.
    fn holds(&self, world: &World, view: &UiView, canvases: &[CanvasKey], w: &Written) -> bool {
        let laid = |id: u32| self.layout.get(id).is_some();
        // A write to an element, or to a child that may join (or leave) its parent.
        let touches = |id: u32| laid(id) || world.parent_id(id).is_some_and(laid);
        let sized_by_content = |id: u32| {
            world.has_layout_element(id)
                || world
                    .parent_id(id)
                    .is_some_and(|p| world.has_layout_group(p))
        };
        let non_root = |id: u32| self.layout.get(id).is_some_and(|r| r.canvas != id);
        let markers_still = match (&self.markers, &view.camera) {
            (None, _) => true,
            (Some((targets, at)), Some(cam)) => {
                *at == eye(cam, view.screen)
                    && !w.pose.iter().any(|id| targets.binary_search(id).is_ok())
            }
            (Some(_), None) => false,
        };
        self.screen == view.screen
            && self.camera == view.camera.is_some()
            && self.canvases == canvases
            && (Rc::ptr_eq(&self.textures, &view.textures) || *self.textures == *view.textures)
            && markers_still
            && !w.structure.iter().any(|&id| touches(id))
            && !w.transform.iter().any(|&id| non_root(id))
            && !w
                .content
                .iter()
                .any(|&id| touches(id) && sized_by_content(id))
    }
}

/// Every Canvas's [`CanvasKey`] on `screen`, in the World's order (the layout's
/// tie-break between equal sort orders).
fn canvas_keys(world: &World, screen: Vec2) -> Vec<CanvasKey> {
    let ids = world.ids_with_canvas().into_iter();
    let key = |id: u32| {
        let c = world.canvas(id)?;
        let s = c.is_world_space();
        Some((id, c.sort_order, s, c.size(screen), c.scale_factor(screen)))
    };
    ids.filter_map(key).collect()
}

/// The sorted targets of every `WorldAnchor` in the World, or `None` when there is
/// no marker (a marker hidden this frame still counts: a move can show it).
fn marker_targets(world: &World) -> Option<Vec<u32>> {
    let mut any = false;
    let mut targets = Vec::new();
    for id in world.ids_with_rect_transform() {
        let Some(rt) = world.rect_transform(id) else {
            continue;
        };
        if let Some(anchor) = &rt.world_anchor {
            any = true;
            targets.extend(anchor.target);
        }
    }
    targets.sort_unstable();
    targets.dedup();
    any.then_some(targets)
}

fn eye(cam: &Camera, screen: Vec2) -> Eye {
    let aspect = screen.x / screen.y;
    (
        cam.build_view_projection(aspect),
        cam.position,
        cam.forward(),
    )
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod layout_tests;
