//! The occlusion bake's words on the console line Generate Lighting and Bake
//! Occlusion end with: its cells (and of them the ones sampled, #2328), its objects,
//! its size and time, or why nothing is culled; and the warning a bake gives
//! when the scene file lacks a material the PVS fingerprints.

use rusty_lighting::occlusion::OcclusionBakeReport;
use rusty_scene::lighting::occlusion::FloorSource;
use rusty_scene::Scene;

/// What the bake says when it sampled every cell for want of a navmesh to read
/// where a camera can be, and what to do.
pub const EVERYWHERE: &str = "no fresh navmesh: every cell sampled; bake the navmesh to \
                              sample only where a camera can be";

/// What the bake says when a current navmesh marked no cell of its grid where a
/// camera can be, so it sampled every cell, and what to do.
pub const MISSED: &str = "the navmesh marks no cell where a camera can be: every cell \
                          sampled; re-bake the navmesh over the level";

/// The occlusion bake's part of the summary line: empty when nothing occludes.
pub(crate) fn occlusion_summary(o: &OcclusionBakeReport, ms: u64) -> String {
    let never = match o.never_culled {
        0 => String::new(),
        n => format!(", {n} never culled (too large to sample)"),
    };
    if o.too_large {
        let max = rusty_scene::lighting::occlusion::MAX_CELL;
        return format!(", no occlusion culling: the level needs cells over {max} m");
    }
    match o.cells {
        0 => String::new(),
        cells => format!(
            ", occlusion over {cells} cell(s) of {:.1} m{}, {} object(s){never} ({} KiB, {ms} ms)",
            o.cell_size,
            sampled(o, cells),
            o.objects,
            o.bytes.div_ceil(1024),
        ),
    }
}

/// Which of the `cells` were sampled: where a camera can be over the navmesh (and
/// the share of the level it covers), or every cell and why.
fn sampled(o: &OcclusionBakeReport, cells: usize) -> String {
    match o.floor {
        FloorSource::Everywhere if o.navmesh_missed => format!(" ({MISSED})"),
        FloorSource::Everywhere => format!(" ({EVERYWHERE})"),
        FloorSource::Navmesh => format!(
            " ({} sampled, where a camera can be over a navmesh covering {:.0}% of the level)",
            o.baked_cells.min(cells),
            o.floor_coverage.unwrap_or(1.0) * 100.0
        ),
    }
}

/// The warning a bake gives when the scene file at `path` lacks, or holds
/// otherwise, a material the baked meshes draw (#1958,
/// [`unsaved_materials`](rusty_lighting::occlusion::unsaved_materials)):
/// the PVS fingerprints the live library, so the next load of the file would read
/// it as stale. `None` when the file agrees, or cannot be read.
pub fn unsaved_materials_warning(scene: &Scene, path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let saved = rusty_scene::parse_scene_text(&text, path).ok()?.materials;
    let keys = rusty_lighting::occlusion::unsaved_materials(scene, &saved);
    if keys.is_empty() {
        return None;
    }
    let keys: Vec<_> = keys.iter().map(|k| format!("{k:?}")).collect();
    Some(format!(
        "[Occlusion] material(s) {} are defined in memory but not as the scene file holds \
         them: save the scene so its materials are written, or the bake won't match the \
         next load",
        keys.join(", ")
    ))
}
