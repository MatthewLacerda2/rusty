use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use glam::Vec2;

use super::super::mesh::{build_canvas, canvas_frame, CanvasMesh, Sources, TexSize};
use super::{canvas_runs, ids};
use rusty_components::{
    BackdropFilterComponent, CanvasGroupComponent, ImageComponent, MaskComponent,
    RectMaskComponent, ShapeComponent, TextComponent,
};
use rusty_ecs::{Activity, Cursor, World};
use rusty_ui::{UiLayout, UiRect};

#[derive(Debug, PartialEq)]
struct CanvasInputs {
    frame: Vec2,
    overlay: bool,
    rects: Vec<(u32, UiRect)>,
    atlases: u64,
    textures: Vec<(String, Option<Vec2>)>,
}

impl CanvasInputs {
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

type Lookups<'a> = (TexSize<'a>, u64);

struct Watch {
    image: Cursor<ImageComponent>,
    shape: Cursor<ShapeComponent>,
    text: Cursor<TextComponent>,
    mask: Cursor<MaskComponent>,
    rect_mask: Cursor<RectMaskComponent>,
    group: Cursor<CanvasGroupComponent>,
    backdrop: Cursor<BackdropFilterComponent>,
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
            activity: w.change_cursor(),
        }
    }

    fn read(&mut self, w: &World) -> Option<BTreeSet<u32>> {
        let all = [
            ids(w, &mut self.image),
            ids(w, &mut self.shape),
            ids(w, &mut self.text),
            ids(w, &mut self.mask),
            ids(w, &mut self.rect_mask),
            ids(w, &mut self.group),
            ids(w, &mut self.backdrop),
            ids(w, &mut self.activity),
        ];
        let mut written = BTreeSet::new();
        for changed in all {
            written.extend(changed?);
        }
        Some(written)
    }
}

#[derive(Default)]
pub(crate) struct KeptMeshes {
    watch: Option<Watch>,
    held: HashMap<u32, (CanvasInputs, Rc<CanvasMesh>)>,
    built: BTreeSet<u32>,
}

impl KeptMeshes {
    pub(crate) fn update(
        &mut self,
        (world, layout): (&World, &UiLayout),
        screen: Vec2,
        (tex_size, atlases): Sources,
        wanted: &dyn Fn(u32) -> bool,
    ) -> Vec<Rc<CanvasMesh>> {
        self.built.clear();
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
            dirty.clear();
        }
    }

    pub(crate) fn built(&self) -> usize {
        self.built.len()
    }

    fn pass(
        &mut self,
        entries: &[(u32, &UiRect)],
        (world, layout, screen): (&World, &UiLayout, Vec2),
        (tex_size, atlases): Sources,
        wanted: &dyn Fn(u32) -> bool,
        dirty: &BTreeSet<u32>,
    ) -> Vec<Rc<CanvasMesh>> {
        let mut drawn = BTreeSet::new();
        let mut meshes = Vec::new();
        for run in canvas_runs(entries) {
            let run = &entries[run];
            let canvas = run[0].1.canvas;
            if !wanted(canvas) {
                continue;
            }
            let first = drawn.insert(canvas);
            debug_assert!(first, "canvas {canvas}: a canvas's run is contiguous");
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

    fn build(
        &mut self,
        world: &World,
        layout: &UiLayout,
        run: &[(u32, &UiRect)],
        screen: Vec2,
        (tex_size, atlases): Sources,
    ) -> Rc<CanvasMesh> {
        self.built.insert(run[0].1.canvas);
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
