use std::time::Duration;

use super::format::{count, duration};
use super::Step;

pub const SLOWEST: usize = 15;

#[derive(Clone, Debug, PartialEq)]
pub struct MeshCost {
    pub name: String,
    pub texels: u64,
    pub busy: Duration,
}

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

pub(super) fn lines(
    steps: &[StepRun],
    end: Duration,
    meshes: &mut [MeshCost],
    denoised: Option<(u64, Duration)>,
) -> Vec<String> {
    let ends = steps.iter().skip(1).map(|s| s.from).chain([end]);
    let mut out: Vec<String> = steps
        .iter()
        .zip(ends)
        .map(|(run, to)| {
            let took = duration(to.saturating_sub(run.from));
            format!("[Lighting] {} took {took}{}", step(run.step), size(run))
        })
        .collect();
    out.extend(denoised.map(|(charts, busy)| {
        let (n, took) = (count(charts), duration(busy));
        format!(
            "[Lighting] denoise (OIDN): {took} of worker time over {n} lightmaps, within the lightmaps step"
        )
    }));
    meshes.sort_by_key(|m| std::cmp::Reverse(m.busy));
    if !meshes.is_empty() {
        out.push("[Lighting] slowest lightmap meshes (worker time over every thread):".into());
    }
    out.extend(meshes.iter().take(SLOWEST).map(mesh));
    out
}

fn step(step: Step) -> String {
    match step {
        Step::LightProbes { bounce, .. } => format!("light probes (bounce {bounce})"),
        other => other.name().to_string(),
    }
}

fn size(run: &StepRun) -> String {
    let unit = match run.step {
        Step::Lightmaps => "texels",
        Step::LightmapPages => "pages",
        Step::Occlusion => "cells",
        Step::LightProbes { .. } | Step::ReflectionProbes => "probes",
        Step::WritingLightmaps | Step::ProbePlacement | Step::Saving => return String::new(),
    };
    match run.total {
        0 => String::new(),
        n => format!(" ({} {unit})", count(n)),
    }
}

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
