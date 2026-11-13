//! A loaded scene's reflection values are held once, on load (#1584).

use glam::Vec3;

use crate::{apply_scene_data, to_scene_data, Scene};

/// What a scene file wrote out of range comes back held, for every reader alike.
#[test]
fn a_loaded_scene_holds_its_probes_and_reflection_settings() {
    let mut scene = Scene::new();
    let i = scene.reflection_probes.add(Vec3::ZERO, Vec3::ONE);
    let probe = &mut scene.reflection_probes.probes[i];
    (probe.intensity, probe.resolution, probe.far_clip) = (-3.0, Some(300), -1.0);
    (probe.box_min, probe.box_max) = (Vec3::ONE, -Vec3::ONE);
    let reflections = &mut scene.lighting_settings.reflections;
    (reflections.budget_mb, reflections.max_probes) = (0, 0);

    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, to_scene_data(&scene));
    let mut held = scene.reflection_probes.probes[i].clone();
    held.clamp_settings();
    (held.box_min, held.box_max) = (-Vec3::ONE, Vec3::ONE);
    assert_eq!(loaded.reflection_probes.probes[i], held);
    let reflections = &loaded.lighting_settings.reflections;
    assert_eq!(reflections.budget_mb, 8);
    assert_eq!(reflections.max_probes, 1);
}

/// One fallback at most (#1697): a file naming two keeps the first, a named one is
/// manual so placement keeps it, removing it leaves none, and a damaged stored
/// irradiance is dropped on load rather than shaded as NaN.
#[test]
fn a_set_holds_one_fallback_and_drops_a_damaged_irradiance() {
    use crate::lighting::sh::Sh9;
    let mut set = crate::ReflectionProbeSet::new();
    for x in 0..3 {
        set.add(Vec3::X * x as f32, Vec3::ONE);
    }
    (set.probes[1].fallback, set.probes[2].fallback) = (true, true);
    let mut bad = Sh9::zero();
    bad.coeffs[4][1] = f32::NAN;
    set.probes[0].irradiance = Some(bad);
    set.hold();
    assert_eq!(set.fallback(), Some(1));
    assert!(set.probes[1].manual && !set.probes[2].fallback);
    assert_eq!(set.probes[0].irradiance, None);
    set.remove(1);
    assert_eq!(set.fallback(), None);
}
