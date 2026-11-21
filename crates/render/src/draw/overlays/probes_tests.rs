//! The probe family's one line list: each probe's tint on both ends of each of its
//! segments, each family only under its gizmo, and one buffer and one draw for a
//! thousand probes on a GPU.

use super::*;
use crate::draw::overlays::OverlayDraws;

/// A scene of `n` light probes, each with its own SH tint, and two reflection probes.
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
    scene
}

/// Two Area lights at `x`, the second selected.
fn lit(scene: &mut Scene) {
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
    }
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
    use super::super::lights::{area_light_gizmos, AREA_SELECTED_TINT, AREA_TINT};
    let mut scene = probed(3);
    lit(&mut scene);
    let lines = probe_lines(&scene, &SceneGizmos::default());
    let crosses = 3 * 6;
    let volumes = 2 * 24;
    let areas = area_light_gizmos(&scene);
    assert_eq!(areas.len(), 2, "both panels draw");
    let panels: usize = areas.iter().map(|(v, _)| v.len()).sum();
    assert_eq!(lines.len(), crosses + volumes + panels);
    for (i, probe) in scene.probes.probes().iter().enumerate() {
        let at = probe_marker_verts(probe.position, MARKER_HALF);
        holds(&lines[i * 6..i * 6 + 6], &at, sh_marker_tint(&probe.sh));
    }
    for (i, probe) in scene.reflection_probes.probes.iter().enumerate() {
        let from = crosses + i * 24;
        let want = reflection_volume_verts(probe);
        holds(&lines[from..from + 24], &want, REFLECTION_TINT);
    }
    let mut from = crosses + volumes;
    for ((verts, _), tint) in areas.iter().zip([AREA_TINT, AREA_SELECTED_TINT]) {
        holds(&lines[from..from + verts.len()], verts, tint);
        from += verts.len();
    }
}

#[test]
fn each_family_hides_under_its_own_gizmo() {
    let scene = probed(4);
    let mut set = SceneGizmos::default();
    set.set(Gizmo::LightProbes, false);
    let lines = probe_lines(&scene, &set);
    assert_eq!(lines.len(), 2 * 24, "only the reflection volumes");
    set.set(Gizmo::LightProbes, true);
    set.set(Gizmo::ReflectionProbes, false);
    assert_eq!(probe_lines(&scene, &set).len(), 4 * 6, "only the crosses");
    set.set(Gizmo::LightProbes, false);
    assert!(probe_lines(&scene, &set).is_empty());
}

/// A thousand light probes and two reflection probes: the whole family is one
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
    assert_eq!((slot, count), (0, 1000 * 6 + 2 * 24));
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
