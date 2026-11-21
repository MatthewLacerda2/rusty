use rusty_lighting::occlusion::OcclusionBakeReport;
use rusty_scene::lighting::occlusion::FloorSource;
use rusty_scene::Scene;

pub const EVERYWHERE: &str = "no fresh navmesh: every cell sampled; bake the navmesh to \
                              sample only where a camera can be";

pub const MISSED: &str = "the navmesh marks no cell where a camera can be: every cell \
                          sampled; re-bake the navmesh over the level";

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
