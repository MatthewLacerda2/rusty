use glam::{Vec3, Vec4};

use super::solve::BounceSolve;
use crate::Renderer;
use rusty_scene::lighting::probe::ProbeVolume;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::Scene;

/// The `Baked` lights' direct light, added once the bounces stop so no capture counts it twice.
pub type BakedDirect = fn(&Scene, &[Vec3]) -> (Vec<Sh9>, Vec<Vec4>);

/// In capture passes: direct light plus four indirect bounces.
pub const MAX_BOUNCES: u32 = 5;

/// A pass changing no SH coefficient by this much stops the solve; absolute, as a white
/// hemisphere's DC coefficient is a few units.
pub const CONVERGENCE_EPSILON: f32 = 1.0e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BounceReport {
    pub bounces: u32,
    pub converged: bool,
}

impl Renderer {
    /// A test hook: `bounce_tests`; the dev bake runs `bake_probes_observed`.
    pub fn bake_probes_multibounce(
        &mut self,
        scene: &mut Scene,
        resolution: u32,
        direct: BakedDirect,
    ) -> BounceReport {
        self.bake_probes_observed(scene, resolution, direct, &mut |_, _, _| {})
    }

    pub fn bake_probes_observed(
        &mut self,
        scene: &mut Scene,
        resolution: u32,
        direct: BakedDirect,
        observe: &mut dyn FnMut(u32, usize, usize),
    ) -> BounceReport {
        let mut solve = BounceSolve::new(scene, resolution, direct);
        loop {
            let (bounce, done, total) = solve.progress();
            observe(bounce, done, total);
            if let Some(report) = solve.step(self, scene) {
                // Nothing edits the scene inside this call, so the commit cannot fail.
                let committed = solve.commit(scene);
                debug_assert!(committed.is_ok(), "{committed:?}");
                return report;
            }
        }
    }
}

pub(super) fn write_field(volume: &mut ProbeVolume, field: &[Sh9]) {
    for (probe, &sh) in volume.probes_mut().iter_mut().zip(field) {
        probe.sh = sh;
    }
}

pub(super) fn add_baked_direct(scene: &Scene, volume: &mut ProbeVolume, direct: BakedDirect) {
    let positions: Vec<Vec3> = volume.probes().iter().map(|p| p.position).collect();
    if positions.is_empty() {
        return;
    }
    let (direct, occlusion) = direct(scene, &positions);
    for ((probe, direct), occlusion) in volume.probes_mut().iter_mut().zip(direct).zip(occlusion) {
        probe.sh.accumulate(&direct);
        probe.occlusion = occlusion;
    }
}

pub fn field_delta(prev: &[Sh9], next: &[Sh9]) -> f32 {
    let mut max = 0.0f32;
    for (a, b) in prev.iter().zip(next) {
        for (ca, cb) in a.coeffs.iter().zip(b.coeffs.iter()) {
            for c in 0..3 {
                max = max.max((ca[c] - cb[c]).abs());
            }
        }
    }
    max
}

pub fn pass_converged(prev: &[Sh9], next: &[Sh9], epsilon: f32) -> bool {
    field_delta(prev, next) < epsilon
}

#[cfg(test)]
#[path = "bounce_tests.rs"]
mod probe_bounce_tests;

#[cfg(test)]
#[path = "modes_tests.rs"]
mod probe_modes_tests;

#[cfg(test)]
#[path = "hdr_tests.rs"]
mod probe_hdr_tests;

#[cfg(test)]
#[path = "hdr_finite_tests.rs"]
mod probe_hdr_finite_tests;
