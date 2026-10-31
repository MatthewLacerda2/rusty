//! The probe bake a step at a time (#1602): no step bakes more than one reflection
//! probe or a few light probes, each says where it is, and stepping lands where the
//! bake run in one call does. Skips its GPU half without an adapter.

use glam::Vec3;

use super::super::lighting::{start_lighting, LightingBakeParams};
use super::{Stage, LIGHT_PROBES_PER_STEP};
use crate::scene::Scene;

/// A scene with `light` manual light probes and `reflection` manual reflection
/// probes at a small resolution, saved under `name`; returns it and its path.
pub(super) fn probed(name: &str, light: usize, reflection: usize) -> (Scene, String) {
    let dir = crate::test_temp::dir().join(format!("rusty_1602_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut scene = Scene::new();
    for i in 0..light {
        scene.probes.add_probe(Vec3::new(i as f32, 1.0, 0.0));
    }
    for i in 0..reflection {
        let at = Vec3::new(i as f32 * 4.0, 1.0, 0.0);
        scene.reflection_probes.add(at, Vec3::splat(2.0));
    }
    scene.lighting_settings.reflections.resolution = 16;
    (scene, path)
}

#[test]
fn gpu_each_step_bakes_a_reflection_probe_or_a_few_light_probes() {
    let (mut scene, path) = probed("steps", 6, 3);
    let params = LightingBakeParams::default();
    let mut job = start_lighting(&mut scene, Some(&path), None, params).unwrap();
    if !job.report().ran_reflection_bake {
        return; // no adapter
    }
    let mut lines = Vec::new();
    let mut last = job.stage();
    while !job.step(&mut scene, &mut |l| lines.push(l)).unwrap() {
        let now = job.stage();
        match (last, now) {
            (Stage::Light { done: a, .. }, Stage::Light { done: b, .. }) => {
                assert!(b <= a + LIGHT_PROBES_PER_STEP, "{last:?} → {now:?}")
            }
            (Stage::Reflection { done: a, .. }, Stage::Reflection { done: b, .. }) => {
                assert_eq!(b, a + 1, "one reflection probe a step")
            }
            _ => {}
        }
        last = now;
    }
    job.commit(&mut scene).unwrap();
    for i in 1..=3 {
        let line = format!("Baking reflection probe {i} of 3");
        assert!(lines.contains(&line), "{line} in {lines:?}");
    }
    assert!(
        lines.iter().any(|l| l.contains("bounce 1, 6 probe(s)")),
        "{lines:?}"
    );
    let baked = &scene.reflection_probes.probes;
    assert!(
        baked.iter().all(|p| !p.cubemap_path.is_empty()),
        "every probe baked"
    );
    assert_eq!(job.stage(), Stage::Done);
}

/// Stepped and in one call, the same probes: the sync path is the steps run out.
#[test]
fn gpu_stepping_bakes_what_one_call_bakes() {
    let (mut stepped, path) = probed("same", 3, 0);
    let (mut whole, _) = probed("same", 3, 0);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    r.bake_probes(&mut whole, crate::render::DEFAULT_BAKE_RESOLUTION);
    drop(r);
    let params = LightingBakeParams::default();
    let mut job = start_lighting(&mut stepped, Some(&path), None, params).unwrap();
    while !job.step(&mut stepped, &mut |_| {}).unwrap() {}
    job.commit(&mut stepped).unwrap();
    let sh = |s: &Scene| s.probes.probes.iter().map(|p| p.sh).collect::<Vec<_>>();
    assert_eq!(sh(&stepped), sh(&whole));
}

/// Reflection probes with nowhere to write: the bake refuses before it starts, not
/// after the light probes' whole solve.
#[test]
fn unsaved_reflections_refuse_up_front() {
    let (mut scene, _) = probed("unsaved", 2, 1);
    let params = LightingBakeParams::default();
    let Err(err) = start_lighting(&mut scene, None, None, params) else {
        panic!("started with no scene path");
    };
    assert!(err.contains("save the scene"), "{err}");
}
