//! Test hooks: the box colliders and span lookups other crates' navigation tests bake with.

use super::{NavBounds, NavigationGraph, SpanRef, ALL_AREAS};
use glam::{Quat, Vec3};
use rusty_scene::{ColliderComponent, ColliderShape, Scene};

pub fn add_box_with(scene: &mut Scene, min: Vec3, max: Vec3, is_static: bool) -> u32 {
    add_rotated_box(
        scene,
        (min + max) * 0.5,
        max - min,
        Quat::IDENTITY,
        is_static,
    )
}

pub fn add_box(scene: &mut Scene, min: Vec3, max: Vec3) -> u32 {
    add_box_with(scene, min, max, true)
}

pub fn add_rotated_box(
    scene: &mut Scene,
    center: Vec3,
    size: Vec3,
    rot: Quat,
    is_static: bool,
) -> u32 {
    let id = scene.add_entity("box".to_string());
    scene.world.set_static(id, is_static);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = center;
        t.rotation = rot;
    }
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box { size };
    scene.world.set_collider(id, Some(col));
    id
}

/// A 0.1-thick slab whose top is `y = 0`.
pub fn add_floor(scene: &mut Scene, x0: f32, x1: f32, z0: f32, z1: f32) -> u32 {
    add_box(scene, Vec3::new(x0, -0.1, z0), Vec3::new(x1, 0.0, z1))
}

/// 1 m cells, not the 0.25 m default.
pub fn bake_pinned(scene: &mut Scene) -> NavigationGraph {
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.max_step = 0.5;
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(scene);
    g
}

/// Panics when the cell has none.
pub fn ground(g: &NavigationGraph, gx: i32, gz: i32) -> SpanRef {
    let index = g.span_range(gx, gz).start;
    assert!(g.is_walkable(gx, gz), "cell ({gx},{gz}) has no span");
    SpanRef {
        gx,
        gz,
        index: index as u32,
    }
}

pub fn floors(g: &NavigationGraph, gx: i32, gz: i32) -> Vec<f32> {
    g.spans_at(gx, gz).iter().map(|s| s.y).collect()
}

/// A box `NavMeshObstacle` of `size` centred at `center`; `carve` turns carving
/// on, carving at once rather than after standing still.
pub fn add_obstacle(scene: &mut Scene, center: Vec3, size: Vec3, carve: bool) -> u32 {
    let id = scene.add_entity("obstacle".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = center;
    }
    let o = rusty_components::NavMeshObstacleComponent {
        size,
        carve,
        carve_only_stationary: false,
        ..Default::default()
    };
    scene.world.set_nav_obstacle(id, Some(o));
    id
}

pub fn add_modifier(scene: &mut Scene, center: Vec3, size: Vec3, area: u8) -> u32 {
    let id = scene.add_entity("modifier".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = center;
    }
    let v = rusty_components::NavMeshModifierVolumeComponent {
        center: Vec3::ZERO,
        size,
        area,
        ..Default::default()
    };
    scene.world.set_nav_modifier(id, Some(v));
    id
}

/// Its collider bounds follow on their own.
pub fn move_to(scene: &mut Scene, id: u32, position: Vec3) {
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = position;
    }
}

/// Every cell's spans, row by row: the mesh, whatever ids its tiles gave them.
pub fn mesh_cells(g: &NavigationGraph) -> Vec<Vec<crate::NavSpan>> {
    let cells = (0..g.height).flat_map(|gz| (0..g.width).map(move |gx| (gx, gz)));
    cells.map(|(gx, gz)| g.spans_at(gx, gz).to_vec()).collect()
}

/// Cell by cell: where each tile stores its spans is no part of the mesh.
pub fn assert_same_mesh(a: &NavigationGraph, b: &NavigationGraph) {
    assert_eq!((a.width, a.height), (b.width, b.height), "the grids differ");
    let bits = |g: &NavigationGraph| -> Vec<Vec<(u32, u32, u8)>> {
        let cells = (0..g.height).flat_map(|gz| (0..g.width).map(move |gx| (gx, gz)));
        cells
            .map(|(gx, gz)| {
                let spans = g.spans_at(gx, gz).iter();
                spans
                    .map(|s| (s.y.to_bits(), s.ceiling.to_bits(), s.area))
                    .collect()
            })
            .collect()
    };
    let counts = |c: &Vec<Vec<_>>| c.iter().map(Vec::len).collect::<Vec<_>>();
    let (a, b) = (bits(a), bits(b));
    assert_eq!(counts(&a), counts(&b), "per-cell span counts differ");
    assert_eq!(a, b, "span heights or areas differ");
}

pub fn arrives(g: &NavigationGraph, from: SpanRef, to: SpanRef) -> bool {
    g.find_path_masked(from, to, ALL_AREAS).1
}

pub fn full_path(g: &NavigationGraph, from: (i32, i32), to: (i32, i32)) -> Vec<SpanRef> {
    let (path, complete) =
        g.find_path_masked(ground(g, from.0, from.1), ground(g, to.0, to.1), ALL_AREAS);
    assert!(complete, "{from:?} -> {to:?} should arrive");
    path
}
