//! Editor gizmo for the selected Area light (#1674): the rectangle the light emits
//! from and an arrow out of its lit face. Area lights are baked only, so in the
//! editor viewport nothing else shows where one is or which way it shines; drawn
//! for the selected light alone, as Unity shows a light's shape on selection, so a
//! level of dozens of panels is not crossed by lines nobody asked for. Drawn as a
//! line list through the probe gizmos' path (`probes`), in the handles pass, never
//! in a shipped game render.

use glam::Vec3;

use rusty_components::mesh::Vertex;
use rusty_scene::lighting::area::AreaShape;
use rusty_scene::{LightType, Scene};

/// The selected Area light's gizmo tint: a pale yellow.
pub(super) const AREA_TINT: [f32; 4] = [1.0, 0.95, 0.5, 1.0];
/// Length of the facing arrow, in world units.
const ARROW: f32 = 0.5;

/// The selected entity's gizmo when it is an active Area light: its world-space
/// line list and tint. `None` for anything else.
pub(super) fn area_light_gizmo(scene: &Scene) -> Option<(Vec<Vertex>, [f32; 4])> {
    let id = scene.selected_entity_id?;
    let light = scene.world.light(id)?;
    if light.light_type != LightType::Area || !scene.world.is_active_in_hierarchy(id) {
        return None;
    }
    let area = AreaShape::of(scene, id, &light);
    let verts = area_lines(area.center, area.right, area.up, area.forward);
    Some((verts, AREA_TINT))
}

/// The line list of one Area light: the rectangle spanned by the full edges
/// `right` and `up` around `center`, and an [`ARROW`] out of its centre along
/// `forward` with a head. No line out to its range: the arrow says which way it
/// faces, and a reach line per panel crossed the whole view.
pub(super) fn area_lines(center: Vec3, right: Vec3, up: Vec3, forward: Vec3) -> Vec<Vertex> {
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
    fn the_rectangle_has_the_panels_size_and_the_arrow_ends_short() {
        let v = area_lines(Vec3::Y * 3.0, Vec3::X * 1.2, Vec3::Z * 0.6, Vec3::NEG_Y);
        // Four edges, the arrow and its head: seven segments, no reach line.
        assert_eq!(v.len(), 14);
        let edge =
            |k: usize| Vec3::from(v[2 * k].position).distance(Vec3::from(v[2 * k + 1].position));
        assert!((edge(0) - 1.2).abs() < 1e-5 && (edge(1) - 0.6).abs() < 1e-5);
        let ahead = |p: &Vertex| (Vec3::from(p.position) - Vec3::Y * 3.0).dot(Vec3::NEG_Y);
        let reach = v.iter().map(ahead).fold(0.0, f32::max);
        assert!(
            (reach - ARROW).abs() < 1e-5,
            "nothing past the arrow: {reach}"
        );
    }
}
