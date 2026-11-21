use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::OverlayDraws;
use crate::draw::resources::LineResource;
use crate::interpolate::DrawPoses;
use crate::Renderer;
use rusty_components::mesh::Vertex;
use rusty_physics::WireCache;
use rusty_scene::Scene;

pub(crate) const COLLIDER: [f32; 4] = [0.569, 0.957, 0.545, 0.824];
pub(crate) const TRIGGER: [f32; 4] = [0.569, 0.957, 0.545, 0.35];
pub(crate) const MAX_COLLIDER_SEGMENTS: usize = rusty_physics::MAX_WIRE_SEGMENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColliderScope {
    Selected,
    All,
}

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

fn end(p: Vec3, color: [f32; 4]) -> Vertex {
    Vertex {
        tangent: color,
        ..Vertex::new(p, Vec3::Y, [0.0, 0.0])
    }
}

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
    pub(crate) fn precreate_wires(
        &self,
        scene: &Scene,
        view: (ColliderScope, Vec3),
        draws: &mut OverlayDraws,
    ) -> Option<LineResource> {
        let wires = (&mut *self.wire_cache.borrow_mut(), &self.draw_poses);
        let verts = wire_vertices(scene, wires, view);
        self.vertex_lines(draws, &verts, "Collider & Debug Line Gizmo Vertices")
    }

    pub(super) fn vertex_lines(
        &self,
        draws: &mut OverlayDraws,
        verts: &[Vertex],
        label: &str,
    ) -> Option<LineResource> {
        if verts.is_empty() {
            return None;
        }
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let slot = draws.push(Mat4::IDENTITY, [1.0; 4]);
        Some((buffer, slot, verts.len() as u32))
    }
}

#[cfg(test)]
#[path = "wires_tests.rs"]
mod tests;
