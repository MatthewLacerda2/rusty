//! The collider and debug-line gizmos (#1882): every collider's real shape
//! (`physics::collider_wire`) in Unity's green, a trigger's paler, and the
//! `Debug.DrawLine` / `DrawRay` lines in their own colours, drawn through the same
//! line pipeline and group 1 as the probe gizmos.
//!
//! **What draws.** In the Scene view, the selected object's colliders always, every
//! collider under the `Colliders` gizmo, and the debug lines always, as Unity's
//! Scene view does. Over a game frame (the Game view's **Gizmos** toggle,
//! `Harness.Screenshot`'s `gizmos` option), every collider and the debug lines.
//!
//! **Bounded per frame.** Colliders draw nearest the camera first (the selection
//! before any), until [`MAX_COLLIDER_SEGMENTS`] segments: past it the farther ones,
//! or the rest of a level's collision mesh, are left out of that frame. Unity draws
//! a mesh collider in full; at Dust2 scale that is hundreds of thousands of edges,
//! uploaded every frame, so the cap keeps the gizmo a view and not a stall.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::OverlayDraws;
use crate::components::mesh::Vertex;
use crate::render::draw::resources::ProbeResource;
use crate::render::Renderer;
use crate::scene::Scene;

/// Unity's collider gizmo green (145, 244, 139, 210).
pub(crate) const COLLIDER: [f32; 4] = [0.569, 0.957, 0.545, 0.824];
/// A trigger's: the same green, paler, so a solid wall and a trigger volume read apart.
pub(crate) const TRIGGER: [f32; 4] = [0.569, 0.957, 0.545, 0.35];
/// The most collider segments one frame draws (see the module docs).
pub(crate) const MAX_COLLIDER_SEGMENTS: usize = 32_768;

/// Which colliders a gizmo pass draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColliderScope {
    /// The selected object's only (the Scene view with `Colliders` off).
    Selected,
    /// Every one, the selection first.
    All,
}

/// The ids whose colliders draw, in draw order: the selection, then the rest
/// nearest `eye` first (ties by id, so a frame is the same however the ECS iterates).
pub(crate) fn collider_order(scene: &Scene, scope: ColliderScope, eye: Vec3) -> Vec<u32> {
    let world = &scene.world;
    let selected = scene
        .selected_entity_id
        .filter(|&id| world.is_active_in_hierarchy(id));
    let mut ids: Vec<u32> = selected.into_iter().collect();
    if scope == ColliderScope::All {
        let mut rest: Vec<(f32, u32)> = world
            .ids()
            .iter()
            .copied()
            .filter(|&id| Some(id) != selected && world.is_active_in_hierarchy(id))
            .filter_map(|id| Some((world.world_pose(id)?.pos.distance_squared(eye), id)))
            .collect();
        rest.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        ids.extend(rest.into_iter().map(|(_, id)| id));
    }
    ids
}

/// Every segment to draw, grouped by tint (one draw per distinct colour), each
/// group's ends in order. Colliders keep their budget; debug lines are bounded by
/// their store (`scene::runtime::debug_lines`).
pub(crate) fn wire_segments(
    scene: &Scene,
    scope: ColliderScope,
    eye: Vec3,
) -> Vec<([f32; 4], Vec<Vec3>)> {
    let mut groups: BTreeMap<[u32; 4], ([f32; 4], Vec<Vec3>)> = BTreeMap::new();
    let mut add = |tint: [f32; 4], a: Vec3, b: Vec3| {
        let key = tint.map(f32::to_bits);
        let group = groups.entry(key).or_insert_with(|| (tint, Vec::new()));
        group.1.extend([a, b]);
    };
    let mut budget = MAX_COLLIDER_SEGMENTS;
    for id in collider_order(scene, scope, eye) {
        if budget == 0 {
            break;
        }
        let Some(wire) = crate::physics::collider_wire(scene, id, budget) else {
            continue;
        };
        budget -= wire.segments.len();
        let tint = if wire.trigger { TRIGGER } else { COLLIDER };
        for [a, b] in wire.segments {
            add(tint, a, b);
        }
    }
    for line in scene.debug_lines.lines() {
        add(line.color, line.from, line.to);
    }
    groups.into_values().collect()
}

impl Renderer {
    /// The collider and debug-line gizmos' line draws, each at its slot of `draws`.
    pub(crate) fn precreate_wires(
        &self,
        scene: &Scene,
        (scope, eye): (ColliderScope, Vec3),
        draws: &mut OverlayDraws,
    ) -> Vec<ProbeResource> {
        let mut out = Vec::new();
        for (tint, ends) in wire_segments(scene, scope, eye) {
            let verts: Vec<Vertex> = ends
                .iter()
                .map(|&p| Vertex::new(p, Vec3::Y, [0.0, 0.0]))
                .collect();
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Collider & Debug Line Gizmo Vertices"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let slot = draws.push(Mat4::IDENTITY, tint);
            out.push((buffer, slot, verts.len() as u32));
        }
        out
    }
}

#[cfg(test)]
#[path = "wires_tests.rs"]
mod tests;
