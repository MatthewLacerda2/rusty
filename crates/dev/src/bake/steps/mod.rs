//! Generate Lighting's probe step, stepped once a frame by the editor and run to the end by
//! a script. It bakes planned copies and applies them only at `commit`, so a dropped or
//! failed bake changes nothing.

use super::lighting::{LightingBakeReport, PlannedProbes};
use super::reflection::{out_dir, prune_stale, reflection_dir, UNSAVED};
use super::report::probes::Timeline;
use glam::Vec3;
use rusty_lighting::lightmap::probe_direct;
use rusty_render::ibl::probe::solve::BounceSolve;
pub use rusty_render::ibl::probe::solve::PROBES_CHANGED;
use rusty_render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use rusty_scene::{ReflectionProbeSet, Scene};
use staged::{ReflectionSnapshot, StagedCubemaps};

mod staged;

/// A light probe's 32-texel faces cost a sixteenth of a default 128-texel reflection probe's,
/// the one probe a reflection step bakes.
const LIGHT_PROBES_PER_STEP: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Light {
        bounce: u32,
        done: usize,
        total: usize,
    },
    Reflection {
        done: usize,
        total: usize,
    },
    Done,
}

pub struct LightingBake {
    renderer: Option<Renderer>,
    // Probes to bake and no adapter: commit applies nothing, unlike an empty plan.
    skipped: bool,
    light: BounceSolve,
    light_done: bool,
    scene_path: Option<String>,
    planned: ReflectionProbeSet,
    kept: Vec<usize>,
    reflections: ReflectionSnapshot,
    staged: StagedCubemaps,
    was: Baseline,
    report: LightingBakeReport,
    times: Timeline,
}

struct Baseline {
    light: Vec<(Vec3, bool)>,
    reflections: ReflectionSnapshot,
}

impl Baseline {
    fn take(scene: &Scene) -> Self {
        let light = scene.probes.probes().iter();
        Self {
            light: light.map(|p| (p.position, p.manual)).collect(),
            reflections: ReflectionSnapshot::take(scene),
        }
    }

    fn changed(&self, scene: &Scene) -> bool {
        let light = scene.probes.probes().iter().map(|p| (p.position, p.manual));
        !light.eq(self.light.iter().copied()) || self.reflections.changed(scene)
    }
}

impl LightingBake {
    pub(super) fn start(
        scene: &Scene,
        scene_path: Option<&str>,
        planned: PlannedProbes,
        report: LightingBakeReport,
    ) -> Result<Self, String> {
        if !planned.reflections.is_empty() {
            scene_path.ok_or(UNSAVED)?;
        }
        let any = !planned.light.is_empty() || !planned.reflections.is_empty();
        let renderer = any
            .then(|| pollster::block_on(Renderer::new_headless(64, 64)))
            .flatten();
        if any && renderer.is_none() {
            log::warn!("[LightingBake] no GPU/software adapter available — skipping bake");
        }
        Self::on(renderer, scene, scene_path, planned, report)
    }

    fn on(
        renderer: Option<Renderer>,
        scene: &Scene,
        scene_path: Option<&str>,
        planned: PlannedProbes,
        mut report: LightingBakeReport,
    ) -> Result<Self, String> {
        if !planned.reflections.is_empty() {
            scene_path.ok_or(UNSAVED)?;
        }
        report.ran_light_bake = planned.light.is_empty() || renderer.is_some();
        report.ran_reflection_bake = planned.reflections.is_empty() || renderer.is_some();
        let default = scene.lighting_settings.reflections.resolution;
        let any = !planned.light.is_empty() || !planned.reflections.is_empty();
        Ok(Self {
            skipped: any && renderer.is_none(),
            light: BounceSolve::of(planned.light, DEFAULT_BAKE_RESOLUTION, probe_direct),
            light_done: renderer.is_none(),
            renderer,
            scene_path: scene_path.map(str::to_owned),
            reflections: ReflectionSnapshot::of(&planned.reflections, default),
            planned: planned.reflections,
            kept: planned.kept,
            staged: StagedCubemaps::default(),
            was: Baseline::take(scene),
            report,
            times: Timeline::default(),
        })
    }

    pub fn step(&mut self, scene: &mut Scene, say: &mut dyn FnMut(String)) -> Result<bool, String> {
        self.refuse_changed(scene)?;
        self.times.note(self.stage());
        if !self.light_done {
            self.step_light(scene, say);
            return Ok(false);
        }
        let total = self.reflections.len();
        let i = self.staged.len();
        let Some((probe, resolution)) = self.reflections.get(i) else {
            return Ok(true);
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(true);
        };
        let out_dir = out_dir(self.scene_path.as_deref())?;
        say(format!("Baking reflection probe {} of {total}", i + 1));
        let baked = renderer.bake_reflection(scene, probe, *resolution);
        self.staged.write(&out_dir, &baked)?;
        Ok(self.staged.len() >= total)
    }

    pub fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.refuse_changed(scene)?;
        if self.stage() != Stage::Done {
            return Err("the probe bake has not finished".into());
        }
        if self.skipped {
            return Ok(());
        }
        let light = self
            .light
            .baked()
            .ok_or("the light-probe bake has not finished")?;
        let mut reflections = self.planned.clone();
        for (i, &from) in self.kept.iter().enumerate() {
            // The live probe, so an edit the cube is not baked from (a box) survives.
            let live = scene.reflection_probes.probes.get(from).cloned();
            reflections.probes[i] = live.ok_or(PROBES_CHANGED)?;
        }
        self.staged.commit(&mut reflections)?;
        if let (true, Some(path)) = (self.report.ran_reflection_bake, self.scene_path.as_deref()) {
            prune_stale(&reflection_dir(path), self.staged.len())?;
        }
        scene.probes = light.clone();
        scene.replace_reflection_probes(reflections);
        self.times.note(Stage::Done);
        if let Some(path) = self.scene_path.as_deref() {
            super::report::probes::keep_both(scene, path, &self.times);
        }
        Ok(())
    }

    fn refuse_changed(&self, scene: &Scene) -> Result<(), String> {
        match self.was.changed(scene) {
            true => Err(PROBES_CHANGED.into()),
            false => Ok(()),
        }
    }

    fn step_light(&mut self, scene: &mut Scene, say: &mut dyn FnMut(String)) {
        let Some(renderer) = self.renderer.as_mut() else {
            self.light_done = true;
            return;
        };
        for _ in 0..LIGHT_PROBES_PER_STEP {
            let (bounce, done, total) = self.light.progress();
            if done == 0 && total > 0 {
                say(format!(
                    "Baking light probes: bounce {bounce}, {total} probe(s)"
                ));
            }
            if self.light.step(renderer, scene).is_some() {
                self.light_done = true;
                return;
            }
        }
    }

    pub fn stage(&self) -> Stage {
        let total = self.reflections.len();
        if !self.light_done {
            let (bounce, done, total) = self.light.progress();
            Stage::Light {
                bounce,
                done,
                total,
            }
        } else if self.staged.len() < total && self.renderer.is_some() {
            Stage::Reflection {
                done: self.staged.len(),
                total,
            }
        } else {
            Stage::Done
        }
    }

    pub fn report(&self) -> LightingBakeReport {
        self.report
    }
}

#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod staged_tests;
#[cfg(test)]
mod staging_gpu_tests;
#[cfg(test)]
mod staging_tests;
#[cfg(test)]
mod tests;
