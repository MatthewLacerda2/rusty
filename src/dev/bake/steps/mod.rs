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
//! **A bake works on a snapshot and applies only when it completes** (Unity's shape).
//! Placement happens when the job starts; then both probe sets are copied, the bakes
//! read the copies, and their results are staged without touching the scene: the light
//! probes' SH in the solve, the cubemaps in `<name>.baking` files.
//! [`LightingBake::commit`] applies both in one step. If either probe set changed since
//! the start (one added, removed or moved, a reflection probe's resolution, clip
//! planes or shadows), a step or the commit refuses with [`PROBES_CHANGED`] and
//! nothing is applied; the staged files go with the job. The captures still render the
//! live scene's meshes and lights at each step.

use std::path::{Path, PathBuf};

use super::lighting::LightingBakeReport;
use crate::render::ibl::probe::solve::BounceSolve;
pub use crate::render::ibl::probe::solve::PROBES_CHANGED;
use crate::render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use crate::scene::Scene;
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
/// without an adapter, when both bakes skip, as `bake_scene_probes` does), the
/// light-probe solve, and the reflection probes it bakes with the cubes staged so far.
pub struct LightingBake {
    renderer: Option<Renderer>,
    light: BounceSolve,
    light_done: bool,
    scene_path: Option<String>,
    reflections: ReflectionSnapshot,
    staged: StagedCubemaps,
    report: LightingBakeReport,
}

impl LightingBake {
    /// Start baking `scene`'s placed probes (`report` holds the placement), writing
    /// reflection cubemaps beside `scene_path`. Refuses up front, before any probe is
    /// captured, when there are reflection probes and no saved scene to write beside.
    pub(super) fn start(
        scene: &Scene,
        scene_path: Option<&str>,
        mut report: LightingBakeReport,
    ) -> Result<Self, String> {
        if !scene.reflection_probes.is_empty() {
            out_dir(scene_path)?;
        }
        let any = !scene.probes.is_empty() || !scene.reflection_probes.is_empty();
        let renderer = any
            .then(|| pollster::block_on(Renderer::new_headless(64, 64)))
            .flatten();
        if any && renderer.is_none() {
            log::warn!("[LightingBake] no GPU/software adapter available — skipping bake");
        }
        report.ran_light_bake = scene.probes.is_empty() || renderer.is_some();
        report.ran_reflection_bake = scene.reflection_probes.is_empty() || renderer.is_some();
        Ok(Self {
            light: BounceSolve::new(scene, DEFAULT_BAKE_RESOLUTION),
            light_done: renderer.is_none(),
            renderer,
            scene_path: scene_path.map(str::to_owned),
            reflections: ReflectionSnapshot::take(scene),
            staged: StagedCubemaps::default(),
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
        let out_dir = out_dir(self.scene_path.as_deref())?;
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(true);
        };
        say(format!("Baking reflection probe {} of {total}", i + 1));
        let bytes = renderer.bake_reflection(scene, probe, *resolution);
        self.staged.write(&out_dir, &bytes)?;
        Ok(self.staged.len() >= total)
    }

    /// Apply a finished bake to `scene` in one step: the light probes' SH and every
    /// reflection probe's cube. Errors, applying nothing, when the bake has not
    /// finished or the scene's probes changed since it started.
    pub fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.refuse_changed(scene)?;
        if self.stage() != Stage::Done {
            return Err("the probe bake has not finished".into());
        }
        if self.renderer.is_none() {
            return Ok(()); // nothing baked
        }
        self.light.commit(scene)?;
        self.staged.commit(scene)
    }

    /// [`PROBES_CHANGED`] when either probe set is not the one this bake started on.
    fn refuse_changed(&self, scene: &Scene) -> Result<(), String> {
        match self.light.changed(scene) || self.reflections.changed(scene) {
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

/// The directory reflection cubemaps are written to: the saved scene's.
fn out_dir(scene_path: Option<&str>) -> Result<PathBuf, String> {
    let path = scene_path.ok_or_else(|| "save the scene before baking reflections".to_string())?;
    Ok(Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf())
}

#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
