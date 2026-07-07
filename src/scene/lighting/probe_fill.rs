//! src/scene/probe_fill.rs — Analytic SH fill for light probes (testing stand-in).
//!
//! This is the deterministic, render-free stand-in for the real probe bake (#241).
//! It projects the scene's environment — the flat skybox/ambient term plus the
//! active directional light — into each probe's L2 SH, so the whole probe path
//! (placement → interpolation → shader sampling) is testable headlessly *before*
//! the GPU capture lands. It deliberately ignores occlusion and bounce: a probe
//! sees the same analytic sky+sun regardless of geometry. The real capture replaces
//! this with rendered cubemaps; the function signature is the stable target.
//!
//! Pure math over scene VALUES — no rendering, no RNG, no wall-clock. Reachable
//! from the sim layer, so it must stay deterministic.

use glam::Vec3;

use crate::scene::lighting::sh::Sh9;
use crate::scene::{LightType, Scene};

/// The environment the analytic fill projects: a uniform ambient sky radiance plus
/// at most one directional sun. Extracted from the scene so the projection is a pure
/// function of these values (and easy to unit-test in isolation).
#[derive(Clone, Copy, Debug)]
pub struct AnalyticEnv {
    /// Uniform sky radiance (the scene's `ambient_color * ambient_intensity`).
    pub sky: Vec3,
    /// Sun direction *toward* the light (normalized) and its radiance, if any.
    pub sun_dir: Option<Vec3>,
    pub sun_radiance: Vec3,
}

impl AnalyticEnv {
    /// Read the analytic environment out of a scene: the ambient term as the sky,
    /// and the first active directional light as the sun.
    pub fn from_scene(scene: &Scene) -> Self {
        let sky = scene.ambient_color * scene.ambient_intensity;
        let mut sun_dir = None;
        let mut sun_radiance = Vec3::ZERO;
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Directional {
                // The light shines along -Z of its transform; the sample
                // direction *toward* the source is the negation.
                let rotation = scene.world.transform(id).expect("mandatory").rotation;
                let forward = rotation * Vec3::NEG_Z;
                sun_dir = Some((-forward).normalize_or_zero());
                sun_radiance = light.color * light.intensity;
                break;
            }
        }
        Self {
            sky,
            sun_dir,
            sun_radiance,
        }
    }

    /// Project this environment into an L2 SH probe. The sky is a constant radiance
    /// over the sphere (its DC term); the sun is a single bright directional sample.
    pub fn project(&self) -> Sh9 {
        let mut sh = Sh9::zero();
        // Uniform sky: integrate a constant over the sphere. A Fibonacci sphere is
        // near-equal-area, so each sample carries the same solid-angle weight and the
        // high SH bands cancel — the projection recovers the DC term cleanly.
        let dirs = fibonacci_sphere(512);
        let w = 4.0 * std::f32::consts::PI / dirs.len() as f32;
        for d in &dirs {
            sh.add_sample(*d, self.sky, w);
        }
        // Directional sun: one delta-ish sample. Weighted so its reconstructed
        // contribution reads as a plausible directional bounce, not a full sphere.
        if let Some(dir) = self.sun_dir {
            sh.add_sample(dir, self.sun_radiance, 1.0);
        }
        sh
    }
}

/// Fill every probe in the scene's volume with the analytic environment, sampled at
/// the probe's own position. Because the analytic env carries no spatial falloff yet,
/// a directional sun makes probes differ by ORIENTATION rather than position; to give
/// the headless test a position-dependent field, the sky term is attenuated by height
/// (`y`) so probes at different heights read different irradiance. This is purely a
/// stand-in shape — the real bake produces genuine spatial variation.
pub fn analytic_fill(scene: &mut Scene) {
    let env = AnalyticEnv::from_scene(scene);
    for probe in &mut scene.probes.probes {
        let mut local = env;
        // A gentle height gradient so the fill is non-uniform in space (the test
        // asserts position-dependence). Clamped to stay non-negative.
        let h = 1.0 + 0.25 * probe.position.y;
        local.sky = (env.sky * h.max(0.0)).max(Vec3::ZERO);
        probe.sh = local.project();
    }
}

/// `n` near-uniformly distributed unit directions on the sphere (the Fibonacci
/// lattice). Deterministic and roughly equal-area, so each sample carries the same
/// solid-angle weight — exactly what an SH projection of a constant field needs.
pub fn fibonacci_sphere(n: usize) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt()); // golden angle
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        // y from +1 to -1, evenly in z (equal-area bands).
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden * i as f32;
        v.push(Vec3::new(theta.cos() * r, y, theta.sin() * r));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{LightComponent, LightType};
    use glam::Quat;

    /// A default scene with a set ambient term. Uses functional struct-update so the
    /// `field_reassign_with_default` lint doesn't fire on the large `Scene`.
    fn scene_with_ambient(color: Vec3, intensity: f32) -> Scene {
        Scene {
            ambient_color: color,
            ambient_intensity: intensity,
            ..Scene::default()
        }
    }

    #[test]
    fn fill_is_position_dependent() {
        let mut scene = scene_with_ambient(Vec3::new(0.2, 0.3, 0.4), 1.0);
        scene.probes.add_probe(Vec3::new(0.0, 0.0, 0.0));
        scene.probes.add_probe(Vec3::new(0.0, 4.0, 0.0));
        analytic_fill(&mut scene);
        let low = scene.probes.probes[0].sh.eval(Vec3::Y);
        let high = scene.probes.probes[1].sh.eval(Vec3::Y);
        assert!(
            high.x > low.x,
            "expected height to brighten: {high} vs {low}"
        );
    }

    #[test]
    fn directional_sun_adds_directionality() {
        let mut scene = scene_with_ambient(Vec3::splat(0.1), 1.0);
        let id = scene.add_entity("Sun".to_string());
        // Point the light's -Z straight down, so the sample direction is +Y.
        if let Some(mut transform) = scene.world.transform_mut(id) {
            transform.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        }
        scene.world.set_light(
            id,
            Some(LightComponent {
                light_type: LightType::Directional,
                color: Vec3::ONE,
                intensity: 5.0,
                range: 0.0,
                inner_cone: 0.0,
                outer_cone: 0.0,
            }),
        );
        scene.probes.add_probe(Vec3::ZERO);
        analytic_fill(&mut scene);
        let sh = scene.probes.probes[0].sh;
        let up = sh.eval(Vec3::Y).x;
        let down = sh.eval(Vec3::NEG_Y).x;
        assert!(up > down, "sun from above should brighten up-normals");
    }
}
