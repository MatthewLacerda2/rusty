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
//! than a hang. Placement happens when the job starts; the bakes read the scene as it
//! is at each step.

use std::path::{Path, PathBuf};

use super::lighting::LightingBakeReport;
use crate::render::ibl::probe::solve::BounceSolve;
use crate::render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use crate::scene::Scene;

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
/// light-probe solve and the next reflection probe.
pub struct LightingBake {
    renderer: Option<Renderer>,
    light: BounceSolve,
    light_done: bool,
    scene_path: Option<String>,
    next_reflection: usize,
    report: LightingBakeReport,
}

impl LightingBake {
    /// Start baking `scene`'s placed probes (`report` holds the placement), writing
    /// reflection cubemaps beside `scene_path`.
    pub(super) fn start(
        scene: &Scene,
        scene_path: Option<&str>,
        mut report: LightingBakeReport,
    ) -> Self {
        let any = !scene.probes.is_empty() || !scene.reflection_probes.is_empty();
        let renderer = any
            .then(|| pollster::block_on(Renderer::new_headless(64, 64)))
            .flatten();
        if any && renderer.is_none() {
            log::warn!("[LightingBake] no GPU/software adapter available — skipping bake");
        }
        report.ran_light_bake = scene.probes.is_empty() || renderer.is_some();
        report.ran_reflection_bake = scene.reflection_probes.is_empty() || renderer.is_some();
        Self {
            light: BounceSolve::new(scene, DEFAULT_BAKE_RESOLUTION),
            light_done: renderer.is_none(),
            renderer,
            scene_path: scene_path.map(str::to_owned),
            next_reflection: 0,
            report,
        }
    }

    /// One frame's share of the bake; `say` gets each progress line. Returns whether
    /// the bake is done. Errors when reflection probes have no saved scene to write
    /// beside, or a cubemap could not be written.
    pub fn step(&mut self, scene: &mut Scene, say: &mut dyn FnMut(String)) -> Result<bool, String> {
        if !self.light_done {
            self.step_light(scene, say);
            return Ok(false);
        }
        let total = scene.reflection_probes.len();
        if self.next_reflection >= total {
            return Ok(true);
        }
        let out_dir = out_dir(self.scene_path.as_deref())?;
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(true);
        };
        let i = self.next_reflection;
        say(format!("Baking reflection probe {} of {total}", i + 1));
        renderer.bake_reflection_into(scene, i, &out_dir)?;
        self.next_reflection += 1;
        Ok(self.next_reflection >= total)
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
    pub fn stage(&self, scene: &Scene) -> Stage {
        let total = scene.reflection_probes.len();
        if !self.light_done {
            let (bounce, done, total) = self.light.progress();
            Stage::Light {
                bounce,
                done,
                total,
            }
        } else if self.next_reflection < total && self.renderer.is_some() {
            Stage::Reflection {
                done: self.next_reflection,
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
#[path = "steps_tests.rs"]
mod tests;
