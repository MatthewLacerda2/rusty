//! src/components/mesh/runs.rs — which material slot draws which index run (#1285).

use std::ops::Range;

/// Which material slot draws which of a mesh's index `runs` (#1285), Unity's way
/// (`Renderer.sharedMaterials`): slot `i` draws run `i`; a slot past the last run
/// draws the last run again, so an extra material layers over it; a run past the
/// last slot is not drawn. `slots` is `None` for an entity without a Material, whose
/// every run draws with the engine default. The renderer and the lightmap bake
/// (#1290) both pair slots with runs through this one rule.
pub fn material_runs(runs: Vec<Range<u32>>, slots: Option<usize>) -> Vec<(usize, Range<u32>)> {
    let Some(slots) = slots else {
        return runs.into_iter().enumerate().collect();
    };
    let Some(last) = runs.last() else {
        return Vec::new();
    };
    (0..slots)
        .map(|slot| (slot, runs.get(slot).unwrap_or(last).clone()))
        .collect()
}

/// `runs` clipped to the first `count` indices, empty runs dropped (#1285). A mesh's
/// runs always cover its own indices; the clip only matters when its geometry changed
/// ahead of a GPU upload. The renderer and the lightmap bake both clip through this
/// before [`material_runs`], so the two never pair slots with different runs.
pub fn clip_runs(runs: impl Iterator<Item = Range<u32>>, count: u32) -> Vec<Range<u32>> {
    runs.map(|r| r.start.min(count)..r.end.min(count))
        .filter(|r| !r.is_empty())
        .collect()
}
