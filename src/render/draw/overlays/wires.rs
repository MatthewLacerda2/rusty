//! The collider and debug-line gizmos (#1882): every collider's real shape
//! (`physics::ShapeWire`) in Unity's green, a trigger's paler, and the
//! `Debug.DrawLine` / `DrawRay` lines in their own colours. All of them are one
//! vertex buffer and one draw: each vertex carries its colour (in its `tangent`,
//! which the `vs_gizmo_vertex` line pipeline passes through), so a frame of rays
//! in a hundred colours still costs one upload.
//!
//! **What draws.** In the Scene view, the selected object's colliders always, every
//! collider under the `Colliders` gizmo, and the debug lines always, as Unity's
//! Scene view does. Over a game frame (the `GameGizmos` toggle,
//! `Harness.Screenshot`'s `gizmos` option), every collider and the debug lines.
//!
//! **Built once.** A collider's shape is kept in the renderer's
//! [`WireCache`](crate::physics::WireCache) and built again only when what physics
//! builds it from changes; a frame only places it, at the pose the frame draws its
//! entity at (`DrawPoses::pose`, blended between two ticks as its mesh is, #1877).
//!
//! **Bounded per frame.** Colliders draw nearest the camera first (the selection
//! before any), until [`MAX_COLLIDER_SEGMENTS`] segments: past it the farther ones,
//! or the rest of a level's collision mesh, are left out of that frame. Unity draws
//! a mesh collider in full; at Dust2 scale that is hundreds of thousands of edges,
//! uploaded every frame, so the cap keeps the gizmo a view and not a stall.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::OverlayDraws;
use crate::components::mesh::Vertex;
use crate::physics::WireCache;
use crate::render::draw::resources::ProbeResource;
use crate::render::interpolate::DrawPoses;
use crate::render::Renderer;
use crate::scene::Scene;

/// Unity's collider gizmo green (145, 244, 139, 210).
pub(crate) const COLLIDER: [f32; 4] = [0.569, 0.957, 0.545, 0.824];
/// A trigger's: the same green, paler, so a solid wall and a trigger volume read apart.
pub(crate) const TRIGGER: [f32; 4] = [0.569, 0.957, 0.545, 0.35];
/// The most collider segments one frame draws (see the module docs).
pub(crate) const MAX_COLLIDER_SEGMENTS: usize = crate::physics::MAX_WIRE_SEGMENTS;

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
/// Only the entities carrying a Collider or a CharacterController are walked.
pub(crate) fn collider_order(scene: &Scene, scope: ColliderScope, eye: Vec3) -> Vec<u32> {
    let world = &scene.world;
    let selected = scene
        .selected_entity_id
        .filter(|&id| world.is_active_in_hierarchy(id));
    let mut ids: Vec<u32> = selected.into_iter().collect();
    if scope == ColliderScope::All {
        let mut carriers = world.ids_with_collider();
        carriers.extend(world.ids_with_character_controller());
        carriers.sort_unstable();
        carriers.dedup();
        let mut rest: Vec<(f32, u32)> = carriers
            .into_iter()
            .filter(|&id| Some(id) != selected && world.is_active_in_hierarchy(id))
            .filter_map(|id| Some((world.world_pose(id)?.pos.distance_squared(eye), id)))
            .collect();
        rest.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        ids.extend(rest.into_iter().map(|(_, id)| id));
    }
    ids
}

/// One coloured line end, as the vertex-colour line pipeline reads it.
fn end(p: Vec3, color: [f32; 4]) -> Vertex {
    Vertex {
        tangent: color,
        ..Vertex::new(p, Vec3::Y, [0.0, 0.0])
    }
}

/// Every segment to draw, both ends in order, each end carrying its colour. The
/// colliders keep their budget; the debug lines are bounded by their store
/// (`scene::runtime::debug_lines`).
pub(crate) fn wire_vertices(
    scene: &Scene,
    (cache, poses): (&mut WireCache, &DrawPoses),
    (scope, eye): (ColliderScope, Vec3),
) -> Vec<Vertex> {
    cache.refresh(scene);
    let mut out = Vec::new();
    let mut budget = MAX_COLLIDER_SEGMENTS;
    for id in collider_order(scene, scope, eye) {
        if budget == 0 {
            break;
        }
        let Some(wire) = cache.wire(scene, id) else {
            continue;
        };
        let tint = if wire.trigger { TRIGGER } else { COLLIDER };
        let Some(pose) = poses.pose(scene, id) else {
            continue;
        };
        let placed = wire.placed_at(pose);
        for [a, b] in placed.take(budget) {
            out.extend([end(a, tint), end(b, tint)]);
            budget -= 1;
        }
    }
    for line in scene.debug_lines.lines() {
        out.extend([end(line.from, line.color), end(line.to, line.color)]);
    }
    out
}

impl Renderer {
    /// The collider and debug-line gizmos' one draw, at its slot of `draws`; `None`
    /// when there is nothing to draw.
    pub(crate) fn precreate_wires(
        &self,
        scene: &Scene,
        view: (ColliderScope, Vec3),
        draws: &mut OverlayDraws,
    ) -> Option<ProbeResource> {
        let wires = (&mut *self.wire_cache.borrow_mut(), &self.draw_poses);
        let verts = wire_vertices(scene, wires, view);
        if verts.is_empty() {
            return None;
        }
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Collider & Debug Line Gizmo Vertices"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        // White: the colour is each vertex's own.
        let slot = draws.push(Mat4::IDENTITY, [1.0; 4]);
        Some((buffer, slot, verts.len() as u32))
    }
}

#[cfg(test)]
#[path = "wires_tests.rs"]
mod tests;
