//! The probe family's one line list: each probe's tint on both ends of each of its
//! segments, each family only under its gizmo, a reflection probe's volume and an
//! Area light's shape only when selected, and one buffer and one draw for a
//! thousand probes on a GPU.

use super::*;
use crate::draw::overlays::OverlayDraws;

/// A scene of `n` light probes, each with its own SH tint, and two reflection probes,
/// the second inspected.
fn probed(n: usize) -> Scene {
    let mut scene = Scene::new();
    for i in 0..n {
        scene.probes.add_probe(Vec3::new(i as f32, 1.0, 0.0));
    }
    for (i, probe) in scene.probes.probes_mut().iter_mut().enumerate() {
        probe.sh.coeffs[0] = [i as f32 * 0.01, 1.0, 2.0];
    }
    scene.reflection_probes.add(Vec3::ZERO, Vec3::ONE);
    scene.reflection_probes.add(Vec3::X * 5.0, Vec3::ONE);
    scene.inspected_reflection_probe = Some(1);
    scene
}

/// Two Area lights at `x`, the second selected; returns their ids.
fn lit(scene: &mut Scene) -> Vec<u32> {
    let mut ids = Vec::new();
    for x in [10.0, 20.0] {
        let id = scene.add_entity(format!("panel{x}"));
        scene.world.transform_mut(id).expect("fresh").position = Vec3::X * x;
        let light = rusty_scene::LightComponent {
            light_type: rusty_scene::LightType::Area,
            mode: rusty_components::LightMode::Mixed,
            ..rusty_scene::create::default_light()
        };
        scene.world.set_light(id, Some(light));
        scene.selected_entity_id = Some(id);
        ids.push(id);
    }
    ids
}

/// Each run of `got` matches `want`'s positions, every vertex in `tint`.
fn holds(got: &[Vertex], want: &[Vertex], tint: [f32; 4]) {
    assert_eq!(got.len(), want.len());
    for (v, a) in got.iter().zip(want) {
        assert_eq!((v.tangent, v.position), (tint, a.position));
    }
}

#[test]
fn every_probe_segment_carries_its_probes_tint_on_both_ends() {
    use super::super::lights::{area_light_gizmo, AREA_TINT};
    let mut scene = probed(3);
    lit(&mut scene);
    let lines = probe_lines(&scene, &SceneGizmos::default());
    let crosses = 3 * 6;
    let (panel, _) = area_light_gizmo(&scene).expect("the selected panel draws");
    assert_eq!(lines.len(), crosses + 24 + panel.len());
    for (i, probe) in scene.probes.probes().iter().enumerate() {
        let at = probe_marker_verts(probe.position, MARKER_HALF);
        holds(&lines[i * 6..i * 6 + 6], &at, sh_marker_tint(&probe.sh));
    }
    let inspected = reflection_volume_verts(&scene.reflection_probes.probes[1]);
    holds(&lines[crosses..crosses + 24], &inspected, REFLECTION_TINT);
    holds(&lines[crosses + 24..], &panel, AREA_TINT);
}

/// An unselected Area light and an unselected reflection probe add no line; a
/// selected one adds its shape, and an Area light no line out to its range.
#[test]
fn only_the_selected_area_light_and_inspected_probe_draw() {
    let mut scene = probed(0);
    let ids = lit(&mut scene);
    let (mut none, gizmos) = (scene, SceneGizmos::default());
    none.selected_entity_id = None;
    none.inspected_reflection_probe = None;
    assert!(probe_lines(&none, &gizmos).is_empty(), "nothing selected");
    none.selected_entity_id = Some(ids[0]);
    let lines = probe_lines(&none, &gizmos);
    assert_eq!(lines.len(), 14, "a rectangle and an arrow, no reach line");
    let x = |v: &Vertex| v.position[0];
    assert!(
        lines.iter().all(|v| (x(v) - 10.0).abs() < 1.0),
        "the first panel's"
    );
    none.selected_entity_id = None;
    none.inspected_reflection_probe = Some(0);
    let volume = reflection_volume_verts(&none.reflection_probes.probes[0]);
    holds(&probe_lines(&none, &gizmos), &volume, REFLECTION_TINT);
    none.inspected_reflection_probe = Some(7);
    assert!(
        probe_lines(&none, &gizmos).is_empty(),
        "a stale index draws nothing"
    );
}

#[test]
fn each_family_hides_under_its_own_gizmo() {
    let scene = probed(4);
    let mut set = SceneGizmos::default();
    set.set(Gizmo::LightProbes, false);
    let lines = probe_lines(&scene, &set);
    assert_eq!(lines.len(), 24, "only the inspected reflection volume");
    set.set(Gizmo::LightProbes, true);
    set.set(Gizmo::ReflectionProbes, false);
    assert_eq!(probe_lines(&scene, &set).len(), 4 * 6, "only the crosses");
    set.set(Gizmo::LightProbes, false);
    assert!(probe_lines(&scene, &set).is_empty());
}

/// A thousand light probes and an inspected reflection probe: the whole family is one
/// vertex buffer at one overlay slot, drawn once.
#[test]
fn gpu_a_thousand_probes_draw_in_one_draw_from_one_buffer() {
    let Some(r) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let scene = probed(1000);
    let mut draws = OverlayDraws::default();
    let probes = r.precreate_probes(&scene, &SceneGizmos::default(), &mut draws);
    let (buffer, slot, count) = probes.expect("the probes draw");
    assert_eq!((slot, count), (0, 1000 * 6 + 24));
    let vertex = std::mem::size_of::<Vertex>() as u64;
    assert_eq!(buffer.size(), u64::from(count) * vertex);
    assert_eq!(draws.len(), 1, "one slot for the whole family");
    let mut none = SceneGizmos::default();
    none.set(Gizmo::LightProbes, false);
    none.set(Gizmo::ReflectionProbes, false);
    let mut draws = OverlayDraws::default();
    assert!(r.precreate_probes(&scene, &none, &mut draws).is_none());
    assert_eq!(draws.len(), 0, "nothing on, no slot taken");
}
