//! Which lights the realtime path uploads, by mode (#809): a frame drops `Baked`
//! lights, a bake capture drops `Realtime` ones.

use glam::Vec3;

use super::{apply_scene_lights, default_lighting_uniform};
use crate::components::{LightComponent, LightMode, LightType};
use crate::render::clusters::local_lights;
use crate::scene::Scene;

/// One light of `light_type` per mode, in `LightMode::ALL` order, each a different
/// intensity (1, 2, 3) so the upload says which survived.
fn one_per_mode(light_type: LightType) -> Scene {
    let mut scene = Scene::new();
    for (k, mode) in LightMode::ALL.into_iter().enumerate() {
        let id = scene.add_entity(mode.name().to_string());
        let light = LightComponent {
            light_type: light_type.clone(),
            color: Vec3::ONE,
            intensity: (k + 1) as f32,
            range: 10.0,
            inner_cone: 20.0,
            outer_cone: 30.0,
            cast_shadows: false,
            mode,
        };
        scene.world.set_light(id, Some(light));
    }
    scene
}

fn directional_intensities(scene: &Scene, capture: bool) -> Vec<f32> {
    let mut uniform = default_lighting_uniform(scene);
    apply_scene_lights(&mut uniform, scene, capture);
    let n = uniform.num_dir_lights as usize;
    let mut out: Vec<f32> = uniform.dir_lights[..n]
        .iter()
        .map(|l| l.intensity)
        .collect();
    out.sort_by(f32::total_cmp);
    out
}

fn local_intensities(scene: &Scene, capture: bool) -> Vec<f32> {
    local_lights(scene, capture)
        .iter()
        .map(|(l, _)| l.intensity)
        .collect()
}

#[test]
fn a_frame_draws_realtime_and_mixed_lights_only() {
    let suns = one_per_mode(LightType::Directional);
    assert_eq!(directional_intensities(&suns, false), vec![1.0, 2.0]);
    for local in [LightType::Point, LightType::Spotlight] {
        assert_eq!(
            local_intensities(&one_per_mode(local), false),
            vec![1.0, 2.0]
        );
    }
}

#[test]
fn a_bake_capture_draws_mixed_and_baked_lights_only() {
    let suns = one_per_mode(LightType::Directional);
    assert_eq!(directional_intensities(&suns, true), vec![2.0, 3.0]);
    let points = one_per_mode(LightType::Point);
    let baked: Vec<f32> = local_lights(&points, true)
        .iter()
        .map(|(l, _)| l.baked)
        .collect();
    assert_eq!(
        baked,
        vec![0.0, 1.0],
        "a lightmapped surface still skips Baked"
    );
}

#[test]
fn an_ambient_light_ignores_its_mode() {
    let scene = one_per_mode(LightType::Ambient);
    let mut uniform = default_lighting_uniform(&scene);
    apply_scene_lights(&mut uniform, &scene, false);
    assert_eq!(
        uniform.ambient.intensity, 3.0,
        "the last ambient, though Baked"
    );
}
