use std::fmt::Write;

use super::super::editor;
use super::{Row, SceneReport, PROFILING_NOTE, SCRIPT_NOTE};

impl SceneReport {
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
        let notes = notes.into_iter().chain(self.clock_notes());
        let notes = notes.chain(self.part_notes());
        let notes = notes.chain(self.load_notes());
        let notes = notes.chain(self.editor_notes());
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
    fn clock_notes(&self) -> [(bool, &'static str); 1] {
        [(
            self.row("wall_ms").is_some(),
            "(wall_ms: the frame end to end on the wall, its sim step and its draw until the \
             GPU finished, before the bench's readbacks, as measured: it moves with \
             cpu_clock_pct and gpu_clock_pct; not the windowed game's frame time, \
             docs/testing.md)",
        )]
    }

    fn part_notes(&self) -> [(bool, &'static str); 3] {
        [
            (
                self.row("render.ui.layout").is_some(),
                "(render.ui.<part>: the UI stage's CPU parts, inside its row, in no sum)",
            ),
            (
                self.row("render.occlusion.test").is_some(),
                "(render.occlusion.<part>: the Hi-Z stage's CPU parts, inside its row, in no sum)",
            ),
            (
                self.row("gpu.occlusion.select").is_some(),
                "(gpu.occlusion.<part>: its GPU parts, inside its row, in no sum; a timed frame \
                 records each in a pass of its own, so 3 compute_passes (6 with a clustered \
                 batch) against 2 untimed)",
            ),
        ]
    }
}

impl SceneReport {
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

impl SceneReport {
    fn editor_notes(&self) -> [(bool, &'static str); 3] {
        let round = self.row("editor.ui_ms").is_some();
        let rendered = self.row("editor.renderer_ms").is_some();
        [
            (
                round,
                "(editor.<row>: the editor round, in a process of its own: the Scene view with \
                 its default gizmos and the scene's first camera selected; editor.edit_ms the \
                 world's edit-mode frame and editor.ui_ms the editor's UI draw and egui's \
                 tessellation, at the reference CPU clock, editor.wall_ms the whole editor \
                 frame on the wall until the GPU finished, as measured, editor.render.<stage> and \
                 editor.gpu.<pass> its viewport frame; play_enter_ms edit mode to the first Play \
                 tick, without the texture refresh the editor runs before Play (it compresses \
                 into the project), play_stop_ms Stop to the first edit frame drawn, save_ms a \
                 save beside the reports that keeps the bake files as loaded, wall clock, one a \
                 run; play_rss_mb the resident memory the first run's Play entry added)",
            ),
            (
                self.row(editor::VIEW_ROWS[0]).is_some(),
                "(editor.view_w, editor.view_h: the Scene view's size in pixels, what the \
                 editor's panels leave of the window; every editor.gpu row shades it, so a \
                 panel's width moves them)",
            ),
            (
                rendered && self.row("editor.gpu_ms").is_none(),
                "(editor round: this adapter has no timestamp queries: no editor.gpu rows)",
            ),
        ]
    }
}

impl SceneReport {
    pub(super) fn run_notes(&self) -> String {
        let mut out = String::new();
        if !self.set.is_empty() {
            let _ = writeln!(
                out,
                "overridden for the run (--set): {}",
                self.set.join(", ")
            );
        }
        if self.script {
            out.push_str(SCRIPT_NOTE);
        }
        out
    }

    pub(super) fn compared_with(&self) -> (&'static str, &'static str) {
        if self.baseline.is_some() {
            ("before", "the baseline")
        } else {
            ("last", "the last run")
        }
    }

    fn view_size(&self) -> Option<(f64, f64)> {
        let [w, h] = editor::VIEW_ROWS.map(|m| self.row(m).map(|row| row.avg.round()));
        w.zip(h)
    }

    pub(super) fn view_size_moved(&self, previous: &SceneReport) -> Option<String> {
        let (now, before) = (self.view_size()?, previous.view_size()?);
        (now != before).then(|| {
            let px = |(w, h): (f64, f64)| format!("{w}x{h}");
            format!(
                "{}the Scene view is {} against {} in {}: the editor's GPU rows \
                 are not comparable pixel for pixel",
                editor::SAID,
                px(now),
                px(before),
                self.compared_with().1
            )
        })
    }

    pub(super) fn view_size_line(&self, comparable: Option<&SceneReport>) -> String {
        let moved = comparable.and_then(|p| self.view_size_moved(p));
        moved.map_or_else(String::new, |m| format!("warning: {m}\n"))
    }
}

pub(super) fn view_size_delta(before: &Row, now: &Row) -> Option<String> {
    if !editor::VIEW_ROWS.contains(&now.metric.as_str()) {
        return None;
    }
    let was = before.avg.round();
    Some(if was == now.avg.round() {
        "=".into()
    } else {
        format!("was {was}")
    })
}
