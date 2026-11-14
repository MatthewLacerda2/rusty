//! A shadowmasked sun at its shadow distance (#1695): a static wall's shadow on a
//! long floor holds past the cascades in both modes, with no step at the edge.

use glam::Vec3;

use super::fixture::{bake, block, patch, renderer, scene, settled};
use crate::components::{LightMode, LightType};
use crate::core::settings::quality::ShadowmaskMode;
use crate::scene::lighting::MixedLightingMode::Shadowmask;

/// The largest step, as a share of the lit floor, between two neighbouring floor
/// points in the wall's shadow, either side of the shadow distance.
const NO_POP: f32 = 0.15;

#[test]
fn gpu_a_shadowmasked_suns_static_shadow_holds_past_the_shadow_distance() {
    let Some(mut r) = renderer() else { return };
    let (mut s, sun) = scene(LightMode::Mixed, Shadowmask, 1.0);
    // The sun low from -X, a long static wall along -Z shading the floor beside it
    let mut light = s.world.light(sun).unwrap().clone();
    (light.light_type, light.intensity) = (LightType::Directional, 3.0);
    s.world.set_light(sun, Some(light));
    let mut t = s.world.transform_mut(sun).unwrap();
    t.rotation = glam::Quat::from_rotation_z(-0.6)
        * glam::Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    drop(t);
    let wall = (Vec3::new(-1.5, 1.0, -6.0), Vec3::new(0.2, 2.0, 14.0));
    block(&mut s, "wall", wall, 1.0, true);
    for id in s.world.ids_with_post_processing() {
        s.world.post_processing_mut(id).unwrap().shadows.distance = 6.0;
    }
    bake(&mut s, "edge");
    assert!(s.mask_channel(sun).is_some());
    for mode in [
        ShadowmaskMode::Shadowmask,
        ShadowmaskMode::DistanceShadowmask,
    ] {
        r.quality.shadowmask_mode = mode;
        let frame = settled(&mut r, &s);
        let lit = patch(&frame, Vec3::new(2.5, 0.0, -6.0));
        let shade: Vec<f32> = (0..9)
            .map(|k| patch(&frame, Vec3::new(-0.5, 0.0, -3.0 - k as f32)))
            .collect();
        eprintln!("[shadowmask] {mode:?}: lit {lit}, shade {shade:?}");
        assert!(
            shade.iter().all(|&v| v < lit * 0.3),
            "{mode:?}: {shade:?} vs {lit}"
        );
        let step = shade
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        assert!(step <= lit * NO_POP, "{mode:?}: a {step} step vs {lit}");
    }
}
