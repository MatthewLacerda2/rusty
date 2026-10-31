//! src/scene/lighting/probe_fill.rs — Analytic SH fill for light probes (testing stand-in).
//!
//! This is the deterministic, render-free stand-in for the real probe bake (#241).
//! It projects the scene's environment — the drawn ambient (`Scene::drawn_ambient`,
//! #1647) plus the active directional light — into each probe's L2 SH, so the whole probe path
//! (placement → interpolation → shader sampling) is testable headlessly *before*
//! the GPU capture lands. It deliberately ignores occlusion and bounce: a probe
//! sees the same analytic sky+sun regardless of geometry. The real capture replaces
//! this with rendered cubemaps; the function signature is the stable target.
//!
//! Pure math over scene VALUES — no rendering, no RNG, no wall-clock. Reachable
//! from the sim layer, so it must stay deterministic.

use glam::Vec3;

use crate::scene::lighting::sh::Sh9;
use crate::scene::{AmbientRadiance, LightType, Scene};

/// The environment the analytic fill projects: the ambient's radiance by direction
/// plus at most one directional sun. Extracted from the scene so the projection is a pure
/// function of these values (and easy to unit-test in isolation).
#[derive(Clone, Copy, Debug)]
struct AnalyticEnv {
    /// The ambient radiance by direction (`DrawnAmbient::radiance`).
    pub sky: AmbientRadiance,
    /// Sun direction *toward* the light (normalized) and its radiance, if any.
    pub sun_dir: Option<Vec3>,
    pub sun_radiance: Vec3,
}

impl AnalyticEnv {
    /// Read the analytic environment out of a scene: the ambient term as the sky,
    /// and the first active directional light as the sun.
    pub fn from_scene(scene: &Scene) -> Self {
        let sky = scene.drawn_ambient().radiance();
        let mut sun_dir = None;
        let mut sun_radiance = Vec3::ZERO;
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Directional {
                // The light shines along -Z of its transform; the sample
                // direction *toward* the source is the negation.
                let forward = scene.world.transform_direction(id, Vec3::NEG_Z);
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

    /// Project this environment into an L2 SH probe. The ambient's radiance
    /// (`AmbientRadiance::incoming`) is integrated over the sphere: it is built so the
    /// probe's cosine-convolved read-back over π is the forward shader's flat ambient,
    /// exactly for the `Color` hemisphere (linear in `y`, carried by L0 and L1) and up
    /// to the L2 cut for the `Gradient` trilight (its horizon kink needs L2). The sun
    /// is a single bright directional sample.
    pub fn project(&self) -> Sh9 {
        let mut sh = Sh9::zero();
        // The ambient, integrated over the sphere. A Fibonacci sphere is
        // near-equal-area, so each sample carries the same solid-angle weight.
        let dirs = fibonacci_sphere(512);
        let w = 4.0 * std::f32::consts::PI / dirs.len() as f32;
        for d in &dirs {
            sh.add_sample(*d, self.sky.incoming(*d), w);
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
        local.sky = env.sky.scaled(h.max(0.0));
        probe.sh = local.project();
    }
}

/// `n` near-uniformly distributed unit directions on the sphere (the Fibonacci
/// lattice). Deterministic and roughly equal-area, so each sample carries the same
/// solid-angle weight `4π / n`: the quadrature an SH projection of a smooth field
/// over the sphere takes.
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
#[path = "probe_fill_tests.rs"]
mod tests;
