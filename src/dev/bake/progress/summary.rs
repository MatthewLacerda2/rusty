//! The end of a bake's log (#1891): what each step took, and the lightmap meshes
//! that cost the most, so a long bake can say where its time went.
//!
//! A step runs from its first line to the next step's (or the end), so the steps add
//! up to the total: the writes between two steps count in the one before. A mesh's
//! cost is its worker time: the bake's pool (#1656) traces one mesh's batches on
//! every core at once, so a wall-clock span per mesh would overlap its neighbours,
//! while the time its batches took, summed over the threads, is what it cost.

use std::time::Duration;

use super::format::{count, duration};
use super::Step;

/// How many of the slowest lightmap meshes the log names.
pub const SLOWEST: usize = 15;

/// One lightmap mesh's cost: its entity's name, its texels and its worker time.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshCost {
    pub name: String,
    pub texels: u64,
    pub busy: Duration,
}

/// One step as it ran: when it began, and the total of work it last reported.
pub(super) struct StepRun {
    step: Step,
    from: Duration,
    pub(super) total: u64,
}

impl StepRun {
    pub(super) fn new(step: Step, from: Duration) -> Self {
        Self {
            step,
            from,
            total: 0,
        }
    }
}

/// The summary's lines, before the total: a line per step, each ending where the
/// next began or at `end`, then the [`SLOWEST`] meshes (sorted in place).
pub(super) fn lines(steps: &[StepRun], end: Duration, meshes: &mut [MeshCost]) -> Vec<String> {
    let ends = steps.iter().skip(1).map(|s| s.from).chain([end]);
    let mut out: Vec<String> = steps
        .iter()
        .zip(ends)
        .map(|(run, to)| {
            let took = duration(to.saturating_sub(run.from));
            format!("[Lighting] {} took {took}{}", step(run.step), size(run))
        })
        .collect();
    meshes.sort_by_key(|m| std::cmp::Reverse(m.busy));
    if !meshes.is_empty() {
        out.push("[Lighting] slowest lightmap meshes (worker time over every thread):".into());
    }
    out.extend(meshes.iter().take(SLOWEST).map(mesh));
    out
}

/// A step's name in the summary: a light-probe bounce says which.
fn step(step: Step) -> String {
    match step {
        Step::LightProbes { bounce, .. } => format!("light probes (bounce {bounce})"),
        other => other.name().to_string(),
    }
}

/// The step's size, e.g. ` (7.6M texels)`; nothing while its total was unknown.
fn size(run: &StepRun) -> String {
    let unit = match run.step {
        Step::Lightmaps => "texels",
        Step::LightmapPages => "pages",
        Step::Occlusion => "cells",
        Step::LightProbes { .. } | Step::ReflectionProbes => "probes",
    };
    match run.total {
        0 => String::new(),
        n => format!(" ({} {unit})", count(n)),
    }
}

/// One mesh's line: `  Hall_Ceiling: 210.4k texels, 312.5 s, 673 texels/s`.
fn mesh(m: &MeshCost) -> String {
    let secs = m.busy.as_secs_f64();
    let rate = match secs > 0.0 {
        true => format!(", {} texels/s", count((m.texels as f64 / secs) as u64)),
        false => String::new(),
    };
    let texels = count(m.texels);
    format!(
        "[Lighting]   {}: {texels} texels, {secs:.1} s{rate}",
        m.name
    )
}
