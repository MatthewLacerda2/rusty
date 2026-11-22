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

/// One run per canvas, merging only neighbours: the layout walks one root canvas at a
/// time, so a canvas's elements are contiguous.
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
