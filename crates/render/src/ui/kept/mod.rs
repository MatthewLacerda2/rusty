//! crates/render/src/ui/kept/ — each view's UI layout (`layout`) and canvas meshes
//! (`meshes`), kept until what they are built from changes (#2416). GPU-free.
//!
//! What the layout keeps and when it lays out again is `layout`'s; the rest of
//! these docs are the meshes'.
//!
//! A canvas's mesh is a function of its `meshes::CanvasInputs`: its run of the layout
//! (every element's id and laid-out rect, in draw order), the pixel frame it is
//! meshed into and whether it is an overlay, the source size of every texture it
//! looked up, and the glyph atlases' generation (a grown atlas moves every uv); and
//! of its elements' UI components. The inputs are compared, not hashed: equality is
//! exact. The components are not copied to be compared: the World's change log says
//! which ones were written (`meshes::Watch`: one cursor per UI component column the mesh
//! reads, and `Activity` for the active flags and re-parenting, descendants
//! included), and a written element dirties the canvas it lays out under.
//!
//! **The Canvas component is not watched**: what the mesh reads of it is compared
//! instead. `render_mode` reaches the mesh only through the frame and the overlay
//! flag, `reference_resolution` and `match_width_or_height` through the rects'
//! `scale_factor` and size and a `WorldSpace` canvas's frame, and `sort_order` only
//! orders the runs; `pixels_per_unit`, `plane_distance`, `tilt`, `sway` and the
//! `sway_state` place the finished mesh (`ui::space`). Watching it would rebuild a
//! `ScreenSpaceCamera` canvas on every tick the camera turns (`ui::sway`). A canvas whose inputs
//! equal the held ones and which no write touched keeps its mesh, by `Rc`, so the
//! sync sees the same mesh and uploads nothing.
//!
//! **Whole rebuilds.** The first frame, a World the cursors were not opened on (a
//! load, Play and Stop, another scene drawn into this view), an Undo or Redo, and a
//! dropped log each read `Changes::Rebuild`, and every held mesh is dropped. A
//! canvas this frame does not draw (gone from the layout, or filtered out of an
//! editor-mode view) loses its held mesh too, so a write missed while it was not
//! drawn cannot leave it stale.

mod layout;
mod meshes;
pub(crate) use layout::{KeptLayout, Why};
pub(crate) use meshes::KeptMeshes;

#[cfg(test)]
use super::mesh::{build_canvas, CanvasMesh};
#[cfg(test)]
use super::text::atlas::FontAtlases;
#[cfg(test)]
use glam::Vec2;
use rusty_ecs::{Changes, Channel, Cursor, World};
#[cfg(test)]
use rusty_ui::UiLayout;
use rusty_ui::UiRect;

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

/// `cursor`'s ids since its last read, nothing collected when it is quiet; `None`
/// when the log cannot say.
fn ids<T: Channel>(w: &World, cursor: &mut Cursor<T>) -> Option<Vec<u32>> {
    if w.quiet(cursor) {
        return Some(Vec::new());
    }
    match w.changes(cursor) {
        Changes::Rebuild => None,
        Changes::Ids(ids) => Some(ids),
    }
}

#[cfg(test)]
mod tests;
