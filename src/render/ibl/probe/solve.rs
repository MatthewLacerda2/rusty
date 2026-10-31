//! The multi-bounce light-probe solve, one probe at a time (#1602).
//!
//! [`BounceSolve`] is `bounce.rs`'s iteration as a state machine: each
//! [`step`](BounceSolve::step) captures and projects one probe of the current pass,
//! and a finished pass writes its field and decides whether to stop, exactly as the
//! loop did. `Renderer::bake_probes_multibounce` runs it to the end in one call (the
//! script's `Lighting.Generate()`, Unity's `Lightmapping.Bake`); the editor's Generate
//! Lighting steps it across frames (Unity's `BakeAsync`), so the editor keeps answering.
//! One path, so the two never bake differently.

use glam::Vec3;

use super::bake::project_cubemap;
use super::bounce::{add_baked_direct, pass_converged, BounceReport, CONVERGENCE_EPSILON};
use super::bounce::{write_field, MAX_BOUNCES};
use crate::render::Renderer;
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// A light-probe bake in progress: the probe positions taken when it started, the
/// pass it is in, and the field that pass has projected so far.
pub struct BounceSolve {
    positions: Vec<Vec3>,
    resolution: u32,
    /// The pass being captured, from 1 (direct light only).
    bounce: u32,
    /// The last finished pass's field; empty before the first.
    field: Vec<Sh9>,
    /// The current pass's projections, one per probe captured so far.
    pass: Vec<Sh9>,
    report: Option<BounceReport>,
}

impl BounceSolve {
    /// A solve of `scene`'s probes at `resolution` per face. With no probes it has
    /// already finished.
    pub fn new(scene: &Scene, resolution: u32) -> Self {
        let positions: Vec<Vec3> = scene.probes.probes.iter().map(|p| p.position).collect();
        let report = positions.is_empty().then_some(BounceReport {
            bounces: 0,
            converged: true,
        });
        Self {
            pass: Vec::with_capacity(positions.len()),
            positions,
            resolution,
            bounce: 1,
            field: Vec::new(),
            report,
        }
    }

    /// Capture and project the next probe; at a pass's end, write its field onto
    /// `scene` and stop or start the next. Returns the report once the solve has
    /// stopped and the `Baked` lights' direct light is added (#809).
    pub fn step(&mut self, renderer: &mut Renderer, scene: &mut Scene) -> Option<BounceReport> {
        if self.report.is_some() {
            return self.report;
        }
        let p = self.positions[self.pass.len()];
        let capture = match self.bounce {
            1 => renderer.capture_static_cubemap(scene, p, self.resolution),
            _ => renderer.capture_static_cubemap_lit(scene, p, self.resolution),
        };
        self.pass.push(project_cubemap(&capture));
        if self.pass.len() < self.positions.len() {
            return None;
        }
        let next = std::mem::take(&mut self.pass);
        let converged = self.bounce > 1 && pass_converged(&self.field, &next, CONVERGENCE_EPSILON);
        write_field(scene, &next);
        self.field = next;
        if converged || self.bounce == MAX_BOUNCES {
            add_baked_direct(scene);
            self.report = Some(BounceReport {
                bounces: self.bounce,
                converged,
            });
        } else {
            self.bounce += 1;
        }
        self.report
    }

    /// Where it is: the pass (from 1), the probes that pass has captured, and the
    /// probe count. The editor's bar reads it, a dev-only stepper's.
    #[cfg(feature = "dev")]
    pub fn progress(&self) -> (u32, usize, usize) {
        (self.bounce, self.pass.len(), self.positions.len())
    }
}
