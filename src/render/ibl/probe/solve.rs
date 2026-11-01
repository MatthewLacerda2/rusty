//! The multi-bounce light-probe solve, one probe at a time (#1602).
//!
//! [`BounceSolve`] is `bounce.rs`'s iteration as a state machine: each
//! [`step`](BounceSolve::step) captures and projects one probe of the current pass,
//! and a finished pass writes its field and decides whether to stop, exactly as the
//! loop did. `Renderer::bake_probes_multibounce` runs it to the end in one call (the
//! script's `Lighting.Generate()`, Unity's `Lightmapping.Bake`); the editor's Generate
//! Lighting steps it across frames (Unity's `BakeAsync`), so the editor keeps answering.
//! One path, so the two never bake differently.
//!
//! The solve works on a snapshot of the probe set taken when it starts (Unity's
//! bake works on a snapshot, too): passes write their field into that copy, never
//! into the scene, so the scene can be edited while a stepped bake runs. A lit capture
//! reads the field from `scene.probes`, so the copy is swapped in for the length of
//! that one capture. [`commit`](BounceSolve::commit) writes the result, once it is
//! done and [`changed`](BounceSolve::changed) says the scene's probes are the ones it
//! baked.

use glam::Vec3;

use super::bake::project_cubemap;
use super::bounce::{add_baked_direct, pass_converged, BounceReport, CONVERGENCE_EPSILON};
use super::bounce::{write_field, MAX_BOUNCES};
use crate::render::Renderer;
use crate::scene::lighting::probe::ProbeVolume;
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Why a finished bake was not applied: the scene's probes are not the ones it baked.
pub const PROBES_CHANGED: &str = "the probes changed during the bake, so nothing was applied";

/// A light-probe bake in progress: the probe set as it was when it started (its SH
/// the passes' field), the pass it is in, and the field that pass has projected so far.
pub struct BounceSolve {
    volume: ProbeVolume,
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
            volume: scene.probes.clone(),
            pass: Vec::with_capacity(positions.len()),
            positions,
            resolution,
            bounce: 1,
            field: Vec::new(),
            report,
        }
    }

    /// Capture and project the next probe; at a pass's end, write its field into the
    /// snapshot and stop or start the next. Returns the report once the solve has
    /// stopped and the `Baked` lights' direct light is added (#809). The scene's own
    /// probes are left as they were: [`commit`](Self::commit) writes them.
    pub fn step(&mut self, renderer: &mut Renderer, scene: &mut Scene) -> Option<BounceReport> {
        if self.report.is_some() {
            return self.report;
        }
        let p = self.positions[self.pass.len()];
        let capture = match self.bounce {
            1 => renderer.capture_static_cubemap(scene, p, self.resolution),
            _ => {
                // The lit capture reads the last pass's field off the scene.
                std::mem::swap(&mut scene.probes, &mut self.volume);
                let capture = renderer.capture_static_cubemap_lit(scene, p, self.resolution);
                std::mem::swap(&mut scene.probes, &mut self.volume);
                capture
            }
        };
        self.pass.push(project_cubemap(&capture));
        if self.pass.len() < self.positions.len() {
            return None;
        }
        let next = std::mem::take(&mut self.pass);
        let converged = self.bounce > 1 && pass_converged(&self.field, &next, CONVERGENCE_EPSILON);
        write_field(&mut self.volume, &next);
        self.field = next;
        if converged || self.bounce == MAX_BOUNCES {
            add_baked_direct(scene, &mut self.volume);
            self.report = Some(BounceReport {
                bounces: self.bounce,
                converged,
            });
        } else {
            self.bounce += 1;
        }
        self.report
    }

    /// Whether `scene`'s light probes are no longer the ones this solve bakes: one
    /// added, removed or moved since it started (a `FillGrid` among them).
    pub fn changed(&self, scene: &Scene) -> bool {
        let live = &scene.probes.probes;
        live.len() != self.positions.len()
            || live
                .iter()
                .zip(&self.positions)
                .any(|(p, &at)| p.position != at)
    }

    /// Write the finished solve's SH onto `scene`'s probes. Errors, writing nothing,
    /// when the solve has not finished or the scene's probes [`changed`](Self::changed).
    pub fn commit(&self, scene: &mut Scene) -> Result<(), String> {
        if self.report.is_none() {
            return Err("the light-probe bake has not finished".into());
        }
        if self.changed(scene) {
            return Err(PROBES_CHANGED.into());
        }
        let field = self.volume.probes.iter().map(|p| p.sh);
        for (probe, sh) in scene.probes.probes.iter_mut().zip(field) {
            probe.sh = sh;
        }
        Ok(())
    }

    /// Where it is: the pass (from 1), the probes that pass has captured, and the
    /// probe count. The editor's bar and the headless bake's log read it.
    pub fn progress(&self) -> (u32, usize, usize) {
        (self.bounce, self.pass.len(), self.positions.len())
    }
}
