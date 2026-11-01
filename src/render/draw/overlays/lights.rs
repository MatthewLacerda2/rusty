//! Editor gizmos for Area lights (#1674): the rectangle the light emits from, an
//! arrow out of its lit face, and a line along that facing out to its `range`.
//! Area lights are baked only, so in the editor viewport nothing else shows where
//! one is or which way it shines. Drawn as line lists through the probe gizmos'
//! path (`probes`), in the handles pass, never in a shipped game render.

use glam::Vec3;

use crate::components::mesh::Vertex;
use crate::scene::lighting::lightmap::AreaShape;
use crate::scene::{LightType, Scene};

/// An Area light's gizmo tint: a pale yellow, brighter when selected.
const AREA_TINT: [f32; 4] = [1.0, 0.95, 0.5, 0.6];
const AREA_SELECTED_TINT: [f32; 4] = [1.0, 0.95, 0.5, 1.0];
/// Length of the facing arrow, in world units.
const ARROW: f32 = 0.5;

/// One gizmo per active Area light: its world-space line list and tint.
pub(super) fn area_light_gizmos(scene: &Scene) -> Vec<(Vec<Vertex>, [f32; 4])> {
    let mut out = Vec::new();
    for id in scene.world.ids_with_light() {
        let Some(light) = scene.world.light(id) else {
            continue;
        };
        if light.light_type != LightType::Area || !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let area = AreaShape::of(scene, id, &light);
        let verts = area_lines(area.center, area.right, area.up, area.forward, area.range);
        let selected = scene.selected_entity_id == Some(id);
        out.push((
            verts,
            if selected {
                AREA_SELECTED_TINT
            } else {
                AREA_TINT
            },
        ));
    }
    out
}

/// The line list of one Area light: the rectangle spanned by the full edges
/// `right` and `up` around `center`, an [`ARROW`] out of its centre along
/// `forward` with a head, and the line on to `range`.
pub(super) fn area_lines(
    center: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    range: f32,
) -> Vec<Vertex> {
    let corner = |sx: f32, sy: f32| center + right * (0.5 * sx) + up * (0.5 * sy);
    let corners = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];
    let tip = center + forward * ARROW;
    let head = right.normalize_or_zero() * (ARROW * 0.2);
    let mut segments: Vec<(Vec3, Vec3)> =
        (0..4).map(|k| (corners[k], corners[(k + 1) % 4])).collect();
    segments.push((center, tip));
    segments.push((tip, tip - forward * (ARROW * 0.3) + head));
    segments.push((tip, tip - forward * (ARROW * 0.3) - head));
    if range > ARROW {
        segments.push((tip, center + forward * range));
    }
    segments
        .into_iter()
        .flat_map(|(a, b)| [a, b])
        .map(|p| Vertex::new(p, forward, [0.0, 0.0]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rectangle_has_the_panels_size_and_the_reach_its_range() {
        let v = area_lines(
            Vec3::Y * 3.0,
            Vec3::X * 1.2,
            Vec3::Z * 0.6,
            Vec3::NEG_Y,
            10.0,
        );
        // Four edges, the arrow and its head, the reach: eight segments.
        assert_eq!(v.len(), 16);
        let edge =
            |k: usize| Vec3::from(v[2 * k].position).distance(Vec3::from(v[2 * k + 1].position));
        assert!((edge(0) - 1.2).abs() < 1e-5 && (edge(1) - 0.6).abs() < 1e-5);
        let reach = Vec3::from(v[15].position);
        assert!(
            reach.abs_diff_eq(Vec3::new(0.0, -7.0, 0.0), 1e-5),
            "{reach}"
        );
    }

    #[test]
    fn a_short_range_draws_no_reach_line() {
        let v = area_lines(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::NEG_Z, 0.2);
        assert_eq!(v.len(), 14);
    }
}
