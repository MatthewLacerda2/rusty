//! The navmesh bake's report (#1980): the grid, the walkable area per nav area,
//! the islands the spans fall into, the generated off-mesh links, and the
//! colliders the bake read, heaviest first. A render-density mesh collider on a
//! chair is one line at the top of that list, before anyone walks into it.
//!
//! An **island** is a walking area: spans joined by grid steps alone
//! (`NavigationGraph::walk_areas`). One is **reachable** when an actor can get to
//! it (`reachable_spans`: the main floor, wherever an actor starts, and what an
//! off-mesh link leads to). A wall top is an unreachable island too; a small
//! unreachable floor is usually a bug.

use serde_json::{json, Value};

use super::{entity_name, point, Kind, Report, Stages, TOP};
use crate::navigation::{bake_collider_ids, baked_fingerprint, NavigationGraph};
use crate::physics::collider_world_triangles;
use crate::scene::Scene;

/// The most islands a report lists, largest first (the count holds them all).
pub const ISLANDS: usize = 256;

/// The report of the bake that left `nav` and wrote `bytes` (`output_bytes` on
/// disk) for `scene`.
pub fn report(
    scene: &Scene,
    nav: &NavigationGraph,
    stages: &Stages,
    bytes: &[u8],
    output_bytes: u64,
) -> Report {
    let fingerprint = baked_fingerprint(bytes).unwrap_or(0);
    let settings = &scene.nav_settings;
    Report::new(Kind::Navmesh, stages, settings, fingerprint, output_bytes).with(json!({
        "grid": {
            "min": [nav.min_x, nav.min_z],
            "max": [nav.max_x, nav.max_z],
            "cellSize": nav.grid_spacing,
            "width": nav.width,
            "height": nav.height,
        },
        "spans": nav.spans.len(),
        "areas": areas(scene, nav),
        "islands": islands(scene, nav),
        "offMeshLinks": links(nav),
        "colliders": colliders(scene),
    }))
}

/// Per nav area with any span: its name, spans and walkable square metres.
fn areas(scene: &Scene, nav: &NavigationGraph) -> Value {
    let mut spans = [0usize; crate::navigation::MAX_AREAS];
    for s in &nav.spans {
        spans[(s.area as usize).min(spans.len() - 1)] += 1;
    }
    let cell = nav.grid_spacing * nav.grid_spacing;
    let names = &scene.nav_settings.areas;
    let area = |(id, &n): (usize, &usize)| {
        let name = names.get(id).map_or(format!("#{id}"), |a| a.name.clone());
        json!({ "area": id, "name": name, "spans": n, "squareMetres": n as f32 * cell })
    };
    let used = spans.iter().enumerate().filter(|(_, &n)| n > 0);
    Value::Array(used.map(area).collect())
}

/// One island: its spans, area, bounds and centre, and whether it is reachable.
struct Island {
    spans: usize,
    min: glam::Vec3,
    max: glam::Vec3,
    sum: glam::Vec3,
    reachable: bool,
}

/// The islands, largest first (ties to scan order), at most [`ISLANDS`] listed.
pub(super) fn islands(scene: &Scene, nav: &NavigationGraph) -> Value {
    let (area, sizes) = nav.walk_areas();
    let reach = nav.reachable_spans(&crate::dev::bake::lighting::actor_starts(scene));
    let empty = |&spans: &usize| Island {
        spans,
        min: glam::Vec3::INFINITY,
        max: glam::Vec3::NEG_INFINITY,
        sum: glam::Vec3::ZERO,
        reachable: false,
    };
    let mut all: Vec<Island> = sizes.iter().map(empty).collect();
    for s in nav.span_refs() {
        let (at, island) = (nav.span_world(s), &mut all[area[s.index as usize] as usize]);
        (island.min, island.max) = (island.min.min(at), island.max.max(at));
        island.sum += at;
        island.reachable |= reach[s.index as usize];
    }
    let unreachable = all.iter().filter(|i| !i.reachable).count();
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(all[i].spans));
    let cell = nav.grid_spacing * nav.grid_spacing;
    let listed = order.iter().take(ISLANDS).map(|&i| {
        let island = &all[i];
        json!({
            "spans": island.spans,
            "squareMetres": island.spans as f32 * cell,
            "min": point(island.min),
            "max": point(island.max),
            "centre": point(island.sum / island.spans.max(1) as f32),
            "reachable": island.reachable,
        })
    });
    json!({ "count": all.len(), "unreachable": unreachable, "list": listed.collect::<Vec<_>>() })
}

/// The off-mesh links: the generated ones by kind, and the authored count.
fn links(nav: &NavigationGraph) -> Value {
    let (mut drop, mut jump, mut authored) = (0, 0, 0);
    for link in nav.offmesh_links() {
        use crate::navigation::OffMeshLinkKind as K;
        match (link.owner, link.kind) {
            (Some(_), _) | (None, K::Manual) => authored += 1,
            (None, K::Drop) => drop += 1,
            (None, K::Jump) => jump += 1,
        }
    }
    json!({ "generated": { "drop": drop, "jump": jump }, "authored": authored })
}

/// The colliders the bake read with their world triangles, heaviest first (ties
/// to the lower id), at most [`TOP`] listed.
fn colliders(scene: &Scene) -> Value {
    let ids = bake_collider_ids(scene);
    let triangles = |id: u32| collider_world_triangles(scene, id).map_or(0, |t| t.triangles.len());
    let mut weighed: Vec<(u32, usize)> = ids.iter().map(|&id| (id, triangles(id))).collect();
    let total: usize = weighed.iter().map(|w| w.1).sum();
    weighed.sort_by_key(|&(id, n)| (std::cmp::Reverse(n), id));
    let heaviest = weighed
        .iter()
        .take(TOP)
        .map(|&(id, n)| json!({ "entity": id, "name": entity_name(scene, id), "triangles": n }));
    json!({
        "count": ids.len(),
        "triangles": total,
        "heaviest": heaviest.collect::<Vec<_>>(),
    })
}
