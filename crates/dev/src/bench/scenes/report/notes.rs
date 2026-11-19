//! The lines under a scene's table: what the run could not measure, what a
//! row in parts or the load rows are, and what it was compared with.

use std::fmt::Write;

use super::{SceneReport, PROFILING_NOTE};

impl SceneReport {
    /// The lines under the table that say what the run could not measure.
    pub(super) fn notes(&self, first: bool) -> String {
        let mut out = String::new();
        let notes = [
            (
                self.row("renderer_ms").is_none(),
                "(no GPU or software adapter: nothing was rendered)",
            ),
            (
                self.row("renderer_ms").is_some() && self.row("gpu_ms").is_none(),
                "(this adapter has no timestamp queries: no GPU times)",
            ),
            (
                self.row("gpu_ms").is_some(),
                "(gpu_ms, gpu.<pass>, gpu.other: ms at the reference clock, gpu_ms the passes' \
                 busy sum and gpu.other untimed work inside a submit; gpu_between_submits_ms \
                 the GPU waiting on the CPU, as measured; gpu_raw_ms as measured, at \
                 gpu_clock_pct of the reference)",
            ),
            (
                self.row("cpu_ms").is_some(),
                "(cpu_ms, cpu.<system>: ms at the reference CPU clock; frame_ms as measured, \
                 at cpu_clock_pct of it)",
            ),
            (
                self.row("gpu_mb").is_none(),
                "(this backend reports no GPU memory: no gpu_mb)",
            ),
            (
                self.row("renderer_ms").is_some(),
                "(render.<stage>: the renderer's CPU time by stage, summing to renderer_ms; \
                 render.other is what no stage timed)",
            ),
        ];
        let notes = notes.into_iter().chain(self.part_notes());
        let notes = notes.chain(self.load_notes());
        let notes = notes.chain(self.comparison_notes(first));
        for (_, note) in notes.filter(|(on, _)| *on) {
            let _ = writeln!(out, "{note}");
        }
        if let Some(source) = &self.gpu_mb_source {
            let _ = writeln!(out, "(gpu_mb: {source})");
        }
        if let Some(dir) = &self.baseline {
            let _ = writeln!(out, "(compared with the kept report in {dir})");
        }
        if super::super::profile::on_profiling_build() {
            let _ = writeln!(out, "{PROFILING_NOTE}");
        }
        out
    }
}

impl SceneReport {
    /// The notes on a stage timed in parts (#2401): the Hi-Z stage's CPU parts
    /// wherever it ran, its GPU parts only with GPU timing (an untimed frame
    /// records the stage in 2 compute passes).
    fn part_notes(&self) -> [(bool, &'static str); 2] {
        [
            (
                self.row("render.occlusion.test").is_some(),
                "(render.occlusion.<part>: the Hi-Z stage's CPU parts, inside its row, in no sum)",
            ),
            (
                self.row("gpu.occlusion.select").is_some(),
                "(gpu.occlusion.<part>: its GPU parts, inside its row, in no sum; a timed frame \
                 records each in a pass of its own, so 1 or 2 more compute_passes than untimed)",
            ),
        ]
    }
}

impl SceneReport {
    /// What the load rows are: the warm loads' figure with the cold one apart,
    /// or a run's one load, which is cold.
    fn load_notes(&self) -> [(bool, &'static str); 3] {
        let timed = self.row("load.document").is_some();
        [
            (
                timed && self.row("load_cold_ms").is_some(),
                "(load_ms: the warm loads' average, each in a fresh process, its spread theirs; \
                 load_cold_ms the scene's first load in this run, apart)",
            ),
            (
                timed && self.row("load_cold_ms").is_none(),
                "(load_ms: one load (LOAD_RUNS=1), the first, so cold, with no spread)",
            ),
            (
                timed,
                "(load.<step>: the load's steps, summing to load_ms; load.count.<what>: what the \
                 load did: model files the scene uses, sources converted over the run's loads, \
                 textures resident after the first frame compressed and decoded, MB that \
                 frame uploaded)",
            ),
        ]
    }
}
