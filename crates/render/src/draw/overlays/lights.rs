use glam::Vec3;

use rusty_components::mesh::Vertex;
use rusty_scene::lighting::area::AreaShape;
use rusty_scene::{LightType, Scene};

pub(super) const AREA_TINT: [f32; 4] = [1.0, 0.95, 0.5, 1.0];
/// Metres: the facing arrow's length.
const ARROW: f32 = 0.5;

pub(super) fn area_light_gizmos(scene: &Scene) -> Vec<(Vec<Vertex>, [f32; 4])> {
    let Some(selected) = scene.selected_entity_id else {
        return Vec::new();
    };
    let world = &scene.world;
    let under = |id: u32| id == selected || world.ancestors(id).any(|a| a == selected);
    let mut out = Vec::new();
    for id in world.ids_with_light() {
        let Some(light) = world.light(id) else {
            continue;
        };
        if light.light_type != LightType::Area || !under(id) || !world.is_active_in_hierarchy(id) {
            continue;
        }
        let area = AreaShape::of(scene, id, &light);
        out.push((
            area_lines(area.center, area.right, area.up, area.forward),
            AREA_TINT,
        ));
    }
    out
}

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

    fn lamp() -> (Scene, [u32; 4]) {
        let mut scene = Scene::new();
        let [root, shade, panel, other] =
            ["Lamp", "Shade", "Panel", "Crate"].map(|n| scene.add_entity(n.into()));
        scene.set_parent(shade, Some(root)).unwrap();
        scene.set_parent(panel, Some(shade)).unwrap();
        let light = rusty_scene::LightComponent {
            light_type: LightType::Area,
            ..rusty_scene::create::default_light()
        };
        scene.world.set_light(panel, Some(light));
        (scene, [root, shade, panel, other])
    }

    #[test]
    fn an_area_light_draws_when_it_or_an_ancestor_is_selected() {
        let (mut scene, [root, shade, panel, other]) = lamp();
        for (selected, drawn) in [(root, 1), (shade, 1), (panel, 1), (other, 0)] {
            scene.selected_entity_id = Some(selected);
            assert_eq!(area_light_gizmos(&scene).len(), drawn, "{selected}");
        }
        scene.selected_entity_id = None;
        assert!(area_light_gizmos(&scene).is_empty(), "nothing selected");
    }
}
