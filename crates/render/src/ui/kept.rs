//! crates/render/src/ui/kept.rs — each view's canvas meshes, kept until what they
//! are built from changes (#2416). GPU-free.
//!
//! A canvas's mesh is a function of its [`CanvasInputs`]: its run of the layout
//! (every element's id and laid-out rect, in draw order), the pixel frame it is
//! meshed into and whether it is an overlay, the source size of every texture it
//! looked up, and the glyph atlases' generation (a grown atlas moves every uv); and
//! of its elements' UI components. The inputs are compared, not hashed: equality is
//! exact. The components are not copied to be compared: the World's change log says
//! which ones were written ([`Watch`]: one cursor per UI component column, and
//! `Activity` for the active flags and re-parenting, descendants included), and a
//! written element dirties the canvas it lays out under. A canvas whose inputs
//! equal the held ones and which no write touched keeps its mesh, by `Rc`, so the
//! sync sees the same mesh and uploads nothing.
//!
//! **Whole rebuilds.** The first frame, a World the cursors were not opened on (a
//! load, Play and Stop, another scene drawn into this view), an Undo or Redo, and a
//! dropped log each read `Changes::Rebuild`, and every held mesh is dropped. A
//! canvas this frame does not draw (gone from the layout, or filtered out of an
//! editor-mode view) loses its held mesh too, so a write missed while it was not
//! drawn cannot leave it stale.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use glam::Vec2;

use super::mesh::{build_canvas, canvas_frame, CanvasMesh};
use super::text::atlas::FontAtlases;
use rusty_components::{
    BackdropFilterComponent, CanvasComponent, CanvasGroupComponent, ImageComponent, MaskComponent,
    RectMaskComponent, ShapeComponent, TextComponent,
};
use rusty_ecs::{Activity, Changes, Cursor, World};
use rusty_ui::{UiLayout, UiRect};

/// Build every canvas's mesh on a `screen`-pixel target, from scratch. `tex_size`
/// reports a texture's size in texels (`None` when not loaded); text glyphs are
/// packed into `atlases` as they are first drawn. What a kept mesh must equal: the
/// tests' reference.
#[cfg(test)]
pub(crate) fn build_canvas_meshes(
    world: &World,
    layout: &UiLayout,
    screen: Vec2,
    tex_size: &dyn Fn(&str) -> Option<Vec2>,
    atlases: &mut FontAtlases,
) -> Vec<CanvasMesh> {
    let entries: Vec<_> = layout.iter().collect();
    let runs = canvas_runs(&entries).into_iter();
    let meshes =
        runs.map(|r| build_canvas(world, layout, &entries[r], screen, (tex_size, atlases)));
    meshes.filter(|m| !m.batches.is_empty()).collect()
}

/// `entries` (the layout's, in draw order) cut into one run per canvas: the layout
/// walks one root canvas at a time, so a canvas's elements are contiguous.
fn canvas_runs(entries: &[(u32, &UiRect)]) -> Vec<std::ops::Range<usize>> {
    let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
    for (i, (_, rect)) in entries.iter().enumerate() {
        match runs.last_mut() {
            Some(r) if entries[r.start].1.canvas == rect.canvas => r.end = i + 1,
            _ => runs.push(i..i + 1),
        }
    }
    runs
}

/// What one canvas's mesh was built from, besides its elements' components (the
/// module docs).
#[derive(Debug, PartialEq)]
struct CanvasInputs {
    frame: Vec2,
    overlay: bool,
    rects: Vec<(u32, UiRect)>,
    atlases: u64,
    /// Every texture the build looked up, and the size it read.
    textures: Vec<(String, Option<Vec2>)>,
}

impl CanvasInputs {
    /// Whether a mesh built from these is the one `run` builds now (no component
    /// under it written since).
    fn hold(&self, now: (Vec2, bool), run: &[(u32, &UiRect)], kept: Lookups) -> bool {
        let (tex_size, atlases) = kept;
        (self.frame, self.overlay) == now
            && self.atlases == atlases
            && self.rects.len() == run.len()
            && self
                .rects
                .iter()
                .zip(run)
                .all(|(a, b)| a.0 == b.0 && a.1 == *b.1)
            && self.textures.iter().all(|(p, s)| tex_size(p) == *s)
    }
}

/// The texture-size lookup and the atlases' generation.
type Lookups<'a> = (&'a dyn Fn(&str) -> Option<Vec2>, u64);

/// One cursor per channel a canvas's mesh reads from the World.
struct Watch {
    image: Cursor<ImageComponent>,
    shape: Cursor<ShapeComponent>,
    text: Cursor<TextComponent>,
    mask: Cursor<MaskComponent>,
    rect_mask: Cursor<RectMaskComponent>,
    group: Cursor<CanvasGroupComponent>,
    backdrop: Cursor<BackdropFilterComponent>,
    canvas: Cursor<CanvasComponent>,
    activity: Cursor<Activity>,
}

impl Watch {
    fn open(w: &World) -> Self {
        Self {
            image: w.change_cursor(),
            shape: w.change_cursor(),
            text: w.change_cursor(),
            mask: w.change_cursor(),
            rect_mask: w.change_cursor(),
            group: w.change_cursor(),
            backdrop: w.change_cursor(),
            canvas: w.change_cursor(),
            activity: w.change_cursor(),
        }
    }

    /// Every id written on any channel since the last read, or `None` for a whole
    /// rebuild. Reads every cursor, so each is current after.
    fn read(&mut self, w: &World) -> Option<BTreeSet<u32>> {
        let all = [
            w.changes(&mut self.image),
            w.changes(&mut self.shape),
            w.changes(&mut self.text),
            w.changes(&mut self.mask),
            w.changes(&mut self.rect_mask),
            w.changes(&mut self.group),
            w.changes(&mut self.backdrop),
            w.changes(&mut self.canvas),
            w.changes(&mut self.activity),
        ];
        let mut ids = BTreeSet::new();
        for changes in all {
            match changes {
                Changes::Rebuild => return None,
                Changes::Ids(changed) => ids.extend(changed),
            }
        }
        Some(ids)
    }
}

/// A view's kept canvas meshes (the module docs).
#[derive(Default)]
pub(crate) struct KeptMeshes {
    watch: Option<Watch>,
    held: HashMap<u32, (CanvasInputs, Rc<CanvasMesh>)>,
    /// Canvases the last [`Self::update`] meshed (built rather than kept).
    pub(crate) built: usize,
}

impl KeptMeshes {
    /// This frame's mesh of every canvas `wanted` draws, in draw order, empty ones
    /// left out: kept where nothing it is built from changed, else built. A glyph
    /// atlas that grows while one canvas is meshed moves the uvs of every canvas
    /// meshed before it, so a pass that grew one is run again (only the canvases
    /// built before the growth are rebuilt; an atlas grows a bounded few times).
    pub(crate) fn update(
        &mut self,
        (world, layout): (&World, &UiLayout),
        screen: Vec2,
        (tex_size, atlases): (&dyn Fn(&str) -> Option<Vec2>, &mut FontAtlases),
        wanted: &dyn Fn(u32) -> bool,
    ) -> Vec<Rc<CanvasMesh>> {
        self.built = 0;
        let mut dirty = self.dirty(world, layout);
        let entries: Vec<_> = layout.iter().collect();
        loop {
            let generation = atlases.generation;
            let meshes = self.pass(
                &entries,
                (world, layout, screen),
                (tex_size, atlases),
                wanted,
                &dirty,
            );
            if atlases.generation == generation {
                return meshes;
            }
            dirty.clear(); // every written canvas was rebuilt and held by this pass
        }
    }

    /// One pass of [`Self::update`] over the layout's `entries`.
    fn pass(
        &mut self,
        entries: &[(u32, &UiRect)],
        (world, layout, screen): (&World, &UiLayout, Vec2),
        (tex_size, atlases): (&dyn Fn(&str) -> Option<Vec2>, &mut FontAtlases),
        wanted: &dyn Fn(u32) -> bool,
        dirty: &BTreeSet<u32>,
    ) -> Vec<Rc<CanvasMesh>> {
        let mut drawn = BTreeSet::new();
        let mut meshes = Vec::new();
        for run in canvas_runs(entries) {
            let run = &entries[run];
            let canvas = run[0].1.canvas;
            // A canvas's run is contiguous (`canvas_runs`), so `drawn` never refuses one.
            if !wanted(canvas) || !drawn.insert(canvas) {
                continue;
            }
            let now = canvas_frame(world, layout, canvas, screen);
            let keep = (tex_size, atlases.generation);
            let held = self
                .held
                .get(&canvas)
                .filter(|(i, m)| !dirty.contains(&m.canvas) && i.hold(now, run, keep));
            let mesh = match held {
                Some((_, mesh)) => Rc::clone(mesh),
                None => self.build(world, layout, run, screen, (tex_size, atlases)),
            };
            if !mesh.batches.is_empty() {
                meshes.push(mesh);
            }
        }
        self.held.retain(|c, _| drawn.contains(c));
        meshes
    }

    /// The canvases a write since the last frame touched; every canvas (the held
    /// meshes dropped) on a whole rebuild.
    fn dirty(&mut self, world: &World, layout: &UiLayout) -> BTreeSet<u32> {
        let ids = match &mut self.watch {
            Some(watch) => watch.read(world),
            None => {
                self.watch = Some(Watch::open(world));
                None
            }
        };
        let Some(ids) = ids else {
            self.held.clear();
            return BTreeSet::new();
        };
        ids.iter()
            .filter_map(|&id| Some(layout.get(id)?.canvas))
            .collect()
    }

    /// Build `run`'s mesh and hold it with what it was built from.
    fn build(
        &mut self,
        world: &World,
        layout: &UiLayout,
        run: &[(u32, &UiRect)],
        screen: Vec2,
        (tex_size, atlases): (&dyn Fn(&str) -> Option<Vec2>, &mut FontAtlases),
    ) -> Rc<CanvasMesh> {
        self.built += 1;
        let generation = atlases.generation;
        let seen = RefCell::new(Vec::<(String, Option<Vec2>)>::new());
        let looked = |p: &str| {
            let size = tex_size(p);
            let mut seen = seen.borrow_mut();
            if !seen.iter().any(|(q, _)| q == p) {
                seen.push((p.to_string(), size));
            }
            size
        };
        let mesh = Rc::new(build_canvas(world, layout, run, screen, (&looked, atlases)));
        let inputs = CanvasInputs {
            frame: mesh.frame,
            overlay: canvas_frame(world, layout, mesh.canvas, screen).1,
            rects: run.iter().map(|&(id, r)| (id, *r)).collect(),
            atlases: generation,
            textures: seen.into_inner(),
        };
        self.held.insert(mesh.canvas, (inputs, Rc::clone(&mesh)));
        mesh
    }
}

#[cfg(test)]
#[path = "kept_tests.rs"]
mod kept_tests;
