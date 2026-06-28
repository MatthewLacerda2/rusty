//! src/render/ibl/probe_bounce.rs — Multi-bounce light-probe solve (#285).
//!
//! The iteration that turns the single-capture probe bake (`probe_bake.rs`) into a
//! true multi-bounce GI bake. Each pass re-captures every probe and projects the
//! captured radiance into SH; from bounce 2 on, the capture re-lights the STATIC
//! scene with the *previous* pass's probe field (`capture_static_cubemap_lit`), so
//! every iteration folds in one more bounce of indirect light. The passes are
//! accumulated in place on `scene.probes`, then a `<scene>.lighting.json` sidecar
//! persists the converged result exactly as the single-bounce bake did.
//!
//! Two stops, whichever fires first:
//!   * a fixed cap of [`MAX_BOUNCES`] passes, and
//!   * an early-out when a pass adds less than [`CONVERGENCE_EPSILON`] energy
//!     (max per-coefficient SH delta over the whole field) — the next bounce is too
//!     dim to matter.
//!
//! The loop is a pure function of (static scene, probe positions, resolution); the
//! convergence decision (`pass_converged`) is GPU-free and unit-tested directly, so
//! the early-out logic is covered even on an adapter-less host. Render layer: exempt
//! from the determinism guard, but the SH output is byte-identical across runs.

use glam::Vec3;

use crate::render::Renderer;
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Hard cap on bounce iterations. Five passes capture direct light plus four
/// indirect bounces — well past the point diffuse GI stops visibly changing for the
/// LDR/SH precision the bake stores; the convergence clamp usually stops it sooner.
pub const MAX_BOUNCES: u32 = 5;

/// Early-out threshold on the energy a pass adds, measured as the max absolute
/// per-coefficient SH delta across every probe (see [`field_delta`]). A fully white
/// hemisphere projects to a DC coefficient on the order of a few units, so a pass
/// that nudges no coefficient by even 1e-3 is below the LDR/8-bit capture's own
/// noise floor — the next bounce cannot matter, so we stop.
pub const CONVERGENCE_EPSILON: f32 = 1.0e-3;

/// Outcome of a multi-bounce bake — the converged SH per probe is written in place on
/// the scene; this reports *how* the solve terminated, for tests and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BounceReport {
    /// Number of capture passes actually run (1..=[`MAX_BOUNCES`]).
    pub bounces: u32,
    /// `true` if the early-out (energy < epsilon) stopped it before the cap.
    pub converged: bool,
}

impl Renderer {
    /// Bake every probe with a true multi-bounce solve (#285), writing the converged
    /// SH in place on `scene.probes`. Bounce 1 captures direct light only; each later
    /// bounce re-lights the static scene with the previous pass's probe field, adding
    /// one indirect bounce. Stops at [`MAX_BOUNCES`] or when a pass adds less than
    /// [`CONVERGENCE_EPSILON`] energy, whichever comes first. With no probes it is a
    /// no-op. Returns a [`BounceReport`] describing the termination.
    pub fn bake_probes_multibounce(&mut self, scene: &mut Scene, resolution: u32) -> BounceReport {
        let positions: Vec<Vec3> = scene.probes.probes.iter().map(|p| p.position).collect();
        if positions.is_empty() {
            return BounceReport {
                bounces: 0,
                converged: true,
            };
        }

        // Bounce 1: direct-only capture, exactly today's single-capture result.
        let mut field = self.bake_pass(scene, &positions, resolution, false);
        write_field(scene, &field);

        for bounce in 2..=MAX_BOUNCES {
            // Re-light the static scene from the field just written (the previous
            // bounce's SH), then project the added indirect light into a new field.
            let next = self.bake_pass(scene, &positions, resolution, true);
            let converged = pass_converged(&field, &next, CONVERGENCE_EPSILON);
            field = next;
            write_field(scene, &field);
            if converged {
                return BounceReport {
                    bounces: bounce,
                    converged: true,
                };
            }
        }

        BounceReport {
            bounces: MAX_BOUNCES,
            converged: false,
        }
    }

    /// One full pass: capture and project every probe at `positions`. `with_probes`
    /// selects the lit capture (static surfaces sample the current probe field, for
    /// bounce ≥2) versus the direct-only capture (bounce 1).
    fn bake_pass(
        &mut self,
        scene: &Scene,
        positions: &[Vec3],
        resolution: u32,
        with_probes: bool,
    ) -> Vec<Sh9> {
        positions
            .iter()
            .map(|&p| {
                let capture = if with_probes {
                    self.capture_static_cubemap_lit(scene, p, resolution)
                } else {
                    self.capture_static_cubemap(scene, p, resolution)
                };
                crate::render::ibl::probe_bake::project_cubemap(&capture)
            })
            .collect()
    }
}

/// Write a freshly-solved SH field onto the scene's probes (one SH per probe, in
/// order). The next bounce's lit capture reads these back through the probe field.
fn write_field(scene: &mut Scene, field: &[Sh9]) {
    for (probe, &sh) in scene.probes.probes.iter_mut().zip(field) {
        probe.sh = sh;
    }
}

/// The energy a pass added: the max absolute per-coefficient difference between two
/// SH fields, over every probe and every RGB channel of all 9 coefficients. A pure
/// scalar metric (no GPU) that drives the early-out, so the convergence behaviour is
/// unit-testable on any host. Mismatched lengths compare the common prefix.
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

/// Convergence test for one pass: did it add less than `epsilon` energy? Stops the
/// loop when `true`. Factored out (pure, GPU-free) so the early-out decision is
/// covered without a capture.
pub fn pass_converged(prev: &[Sh9], next: &[Sh9], epsilon: f32) -> bool {
    field_delta(prev, next) < epsilon
}

#[cfg(test)]
#[path = "probe_bounce_tests.rs"]
mod probe_bounce_tests;
