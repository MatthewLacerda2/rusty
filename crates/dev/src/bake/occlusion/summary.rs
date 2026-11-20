//! The occlusion bake's words on the console line Generate Lighting and Bake
//! Occlusion end with: its cells (and of them the ones sampled, #2328), its objects,
//! its size and time, or why nothing is culled.

use rusty_lighting::occlusion::OcclusionBakeReport;

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
            match o.baked_cells {
                n if n < cells => format!(" ({n} sampled, where a camera can be)"),
                _ => String::new(),
            },
            o.objects,
            o.bytes.div_ceil(1024),
        ),
    }
}
