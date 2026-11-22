use glam::Vec3;

use super::bake::project_light_probe;
use super::bounce::{add_baked_direct, pass_converged, BounceReport, CONVERGENCE_EPSILON};
use super::bounce::{write_field, BakedDirect, MAX_BOUNCES};
use crate::Renderer;
use rusty_scene::lighting::probe::ProbeVolume;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::Scene;

/// `commit`'s error when the scene's probes changed; the editor matches it.
pub const PROBES_CHANGED: &str = "the probes changed during the bake, so nothing was applied";

pub struct BounceSolve {
    volume: ProbeVolume,
    positions: Vec<Vec3>,
    resolution: u32,
    direct: BakedDirect,
    bounce: u32,
    field: Vec<Sh9>,
    pass: Vec<Sh9>,
    report: Option<BounceReport>,
}

impl BounceSolve {
    pub fn new(scene: &Scene, resolution: u32, direct: BakedDirect) -> Self {
        Self::of(scene.probes.clone(), resolution, direct)
    }

    /// A solve of a set the scene does not hold yet: `changed` and `commit` compare against the
    /// scene's own set, so they are only for a solve made by `new`.
    pub fn of(volume: ProbeVolume, resolution: u32, direct: BakedDirect) -> Self {
        let positions: Vec<Vec3> = volume.probes().iter().map(|p| p.position).collect();
        let report = positions.is_empty().then_some(BounceReport {
            bounces: 0,
            converged: true,
        });
        Self {
            volume,
            pass: Vec::with_capacity(positions.len()),
            positions,
            resolution,
            direct,
            bounce: 1,
            field: Vec::new(),
            report,
        }
    }

    pub fn step(&mut self, renderer: &mut Renderer, scene: &mut Scene) -> Option<BounceReport> {
        if self.report.is_some() {
            return self.report;
        }
        let p = self.positions[self.pass.len()];
        let capture = match self.bounce {
            1 => renderer.capture_light_probe(scene, p, self.resolution, false),
            _ => {
                // The lit capture reads the field off `scene.probes`, so the snapshot stands in.
                std::mem::swap(&mut scene.probes, &mut self.volume);
                let capture = renderer.capture_light_probe(scene, p, self.resolution, true);
                std::mem::swap(&mut scene.probes, &mut self.volume);
                capture
            }
        };
        let ambient = scene.drawn_ambient().radiance();
        self.pass.push(project_light_probe(&capture, ambient));
        if self.pass.len() < self.positions.len() {
            return None;
        }
        let next = std::mem::take(&mut self.pass);
        let converged = self.bounce > 1 && pass_converged(&self.field, &next, CONVERGENCE_EPSILON);
        write_field(&mut self.volume, &next);
        self.field = next;
        if converged || self.bounce == MAX_BOUNCES {
            add_baked_direct(scene, &mut self.volume, self.direct);
            self.report = Some(BounceReport {
                bounces: self.bounce,
                converged,
            });
        } else {
            self.bounce += 1;
        }
        self.report
    }

    pub fn changed(&self, scene: &Scene) -> bool {
        let live = &scene.probes.probes();
        live.len() != self.positions.len()
            || live
                .iter()
                .zip(&self.positions)
                .any(|(p, &at)| p.position != at)
    }

    pub fn commit(&self, scene: &mut Scene) -> Result<(), String> {
        if self.report.is_none() {
            return Err("the light-probe bake has not finished".into());
        }
        if self.changed(scene) {
            return Err(PROBES_CHANGED.into());
        }
        let field = self.volume.probes().iter().map(|p| (p.sh, p.occlusion));
        for (probe, (sh, occlusion)) in scene.probes.probes_mut().iter_mut().zip(field) {
            probe.sh = sh;
            probe.occlusion = occlusion;
        }
        Ok(())
    }

    #[cfg(feature = "dev")]
    pub fn baked(&self) -> Option<&ProbeVolume> {
        self.report.map(|_| &self.volume)
    }

    pub fn progress(&self) -> (u32, usize, usize) {
        (self.bounce, self.pass.len(), self.positions.len())
    }
}
