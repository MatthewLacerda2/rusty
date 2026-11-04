//! The probe step of Generate Lighting as a job taken a step at a time (#1602).
//!
//! Unity bakes in two ways: `Lightmapping.Bake` blocks, `BakeAsync` runs while the
//! editor stays responsive. [`LightingBake`] is the one bake both call. A
//! [`step`](LightingBake::step) is one frame's share — a few light probes of the
//! current bounce, or one reflection probe — so `Lighting.Generate()` runs steps
//! until it is done ([`super::lighting::bake_lighting`]) and the editor's Generate
//! Lighting button runs one per frame, keeping the editor and the MCP answering.
//!
//! Each step that starts a bounce pass or a reflection probe says so (`probe i of
//! n`), to the console and so to the MCP log, so a long bake reads as progress rather
//! than a hang.
//!
//! **A bake works on a plan and applies only when it completes** (Unity's shape).
//! Placement plans both probe sets when the job starts, into copies the scene does
//! not hold (#1737); the bakes read the copies, and their results are staged without
//! touching the scene: the light probes' SH in the solve, the cubemaps in
//! `<name>.baking` files in the scene's `<scene>.reflections/` folder.
//! [`LightingBake::commit`] moves the cubes into place, removes the folder's cubes
//! past the new count, then swaps the planned sets in, with their lighting, in one
//! step; until then the scene keeps its probes and their old lighting, so a bake that
//! is refused, dropped or fails changes nothing. If either of the scene's own probe
//! sets changed since the start (one added, removed, moved or marked manual, a
//! reflection probe's resolution, clip planes or shadows), a step or the commit
//! refuses with [`PROBES_CHANGED`] and nothing is applied; the staged files go with
//! the job. The captures still render the live scene's meshes and lights at each step.

use super::lighting::{LightingBakeReport, PlannedProbes};
use super::reflection::{out_dir, prune_stale, reflection_dir, UNSAVED};
use crate::render::ibl::probe::solve::BounceSolve;
pub use crate::render::ibl::probe::solve::PROBES_CHANGED;
use crate::render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use crate::scene::{ReflectionProbeSet, Scene};
use glam::Vec3;
use staged::{ReflectionSnapshot, StagedCubemaps};

mod staged;

/// Light probes captured per step: one light probe's six faces are a fraction of a
/// reflection probe's at its 512-texel default.
const LIGHT_PROBES_PER_STEP: usize = 4;

/// Where a probe bake is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Light probes: the bounce pass (from 1), probes captured in it, and the count.
    Light {
        bounce: u32,
        done: usize,
        total: usize,
    },
    /// Reflection probes: the ones baked, and the count.
    Reflection {
        done: usize,
        total: usize,
    },
    Done,
}

/// A probe bake in progress: the headless renderer it captures with (`None`
/// without an adapter, when both bakes skip and nothing is applied, or with nothing
/// to bake, when the empty planned sets are applied), the planned
/// light probes' solve, the planned reflection probes with the cubes staged so far,
/// and the scene's own sets as they were, to tell an edit made midway.
pub struct LightingBake {
    renderer: Option<Renderer>,
    /// There were probes to bake and no adapter to bake them: commit applies nothing.
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
}

/// The scene's probe sets when a bake started: an edit to them midway drops it.
struct Baseline {
    light: Vec<(Vec3, bool)>,
    reflections: ReflectionSnapshot,
}

impl Baseline {
    fn take(scene: &Scene) -> Self {
        let light = scene.probes.probes.iter();
        Self {
            light: light.map(|p| (p.position, p.manual)).collect(),
            reflections: ReflectionSnapshot::take(scene),
        }
    }

    fn changed(&self, scene: &Scene) -> bool {
        let light = scene.probes.probes.iter().map(|p| (p.position, p.manual));
        !light.eq(self.light.iter().copied()) || self.reflections.changed(scene)
    }
}

impl LightingBake {
    /// Start baking the `planned` probe sets of `scene` (`report` holds the
    /// placement), writing reflection cubemaps into `scene_path`'s own
    /// `<scene>.reflections/` folder, on a headless renderer. Refuses up front,
    /// before any probe is captured, when there are reflection probes and no saved
    /// scene to write them for.
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

    /// [`start`](Self::start) on `renderer`: `None` skips both bakes, and the
    /// commit then applies nothing, so the scene keeps its old lighting, unless
    /// nothing was planned: then the commit applies the empty sets.
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
            light: BounceSolve::of(planned.light, DEFAULT_BAKE_RESOLUTION),
            light_done: renderer.is_none(),
            renderer,
            scene_path: scene_path.map(str::to_owned),
            reflections: ReflectionSnapshot::of(&planned.reflections, default),
            planned: planned.reflections,
            kept: planned.kept,
            staged: StagedCubemaps::default(),
            was: Baseline::take(scene),
            report,
        })
    }

    /// One frame's share of the bake; `say` gets each progress line. Returns whether
    /// the bake is done and ready to [`commit`](Self::commit). Errors when a cubemap
    /// could not be written, or the scene's probes changed since the start ([`PROBES_CHANGED`]).
    pub fn step(&mut self, scene: &mut Scene, say: &mut dyn FnMut(String)) -> Result<bool, String> {
        self.refuse_changed(scene)?;
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

    /// Apply a finished bake to `scene` in one step: the planned light probes with
    /// their SH, and the planned reflection probes with their cubes, in place of the
    /// scene's sets. A reflection bake that ran first removes the cubes in the
    /// scene's folder past its probe count, all of them when it planned none.
    /// Errors, applying nothing to the scene, when the bake has not finished, the
    /// scene's probes changed since it started, or a cube could not be moved into
    /// place or a stale one removed (the cubes moved before it are on disk; a retry
    /// does the rest). Without an adapter nothing was baked and nothing is applied
    /// or removed; with nothing planned (no static geometry, no manual probe) the
    /// empty sets are.
    pub fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.refuse_changed(scene)?;
        if self.stage() != Stage::Done {
            return Err("the probe bake has not finished".into());
        }
        if self.skipped {
            return Ok(()); // nothing baked
        }
        let light = self
            .light
            .baked()
            .ok_or("the light-probe bake has not finished")?;
        let mut reflections = self.planned.clone();
        for (i, &from) in self.kept.iter().enumerate() {
            // The manual probe as the scene holds it now: an edit the cube is not
            // baked from (a box, an intensity) survives the commit.
            let live = scene.reflection_probes.probes.get(from).cloned();
            reflections.probes[i] = live.ok_or(PROBES_CHANGED)?;
        }
        self.staged.commit(&mut reflections)?;
        // A reflection bake that ran leaves only its own cubes in the scene's folder.
        if let (true, Some(path)) = (self.report.ran_reflection_bake, self.scene_path.as_deref()) {
            prune_stale(&reflection_dir(path), self.staged.len())?;
        }
        scene.probes = light.clone();
        scene.reflection_probes = reflections;
        Ok(())
    }

    /// [`PROBES_CHANGED`] when either of the scene's probe sets is not the one this
    /// bake started on.
    fn refuse_changed(&self, scene: &Scene) -> Result<(), String> {
        match self.was.changed(scene) {
            true => Err(PROBES_CHANGED.into()),
            false => Ok(()),
        }
    }

    /// Up to [`LIGHT_PROBES_PER_STEP`] light probes of the current pass.
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

    /// Where the bake is, for the editor's bar.
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

    /// What the bake did (its placement and whether each half ran).
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
