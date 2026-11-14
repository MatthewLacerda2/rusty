//! The running bake's bar on the scene card (#808, #1602, #1714): the lightmaps'
//! texels (a progressive bake's passes done, samples and time left above it,
//! #1970, in the log's words: `job::pass_line`) or
//! pages with a Cancel button, the probes' stage, the occlusion's cells.

use egui_phosphor::regular as icon;

use super::{ActiveBake, Step};
use crate::EditorUi;
use rusty_dev::bake::lightmap::job::pass_line;
use rusty_dev::bake::steps::Stage;
use rusty_scene::lighting::bake_status::BakeProgress;

impl ActiveBake {
    /// How far the bake is, in the bar's own counts (#2042): its step's share, and
    /// whether the card offers Cancel.
    pub(super) fn progress(&self) -> BakeProgress {
        let share = |(done, total): (u64, u64)| {
            // At most 100, so the cast keeps every value.
            (total > 0).then(|| (done.min(total) * 100 / total) as u8)
        };
        let cells = |(done, total): (usize, usize)| share((done as u64, total as u64));
        let (what, percent, cancellable) = match &self.step {
            Step::Lightmaps(job) => {
                // The trace's texels, then the encode's pages, as the bar counts.
                let (what, counts) = match job.pages() {
                    Some((done, n)) => ("lighting (lightmap pages)", (done, n)),
                    None => ("lighting", job.progress()),
                };
                (what, share(counts), !job.is_cancelled())
            }
            Step::Probes { job, .. } => {
                let counts = match job.stage() {
                    Stage::Light { done, total, .. } | Stage::Reflection { done, total } => {
                        (done, total)
                    }
                    Stage::Done => (1, 1),
                };
                ("lighting (probes)", cells(counts), false)
            }
            Step::Occlusion { job, .. } => ("lighting (occlusion)", cells(job.cells()), false),
            Step::OcclusionOnly { job, .. } => ("occlusion", cells(job.cells()), false),
        };
        BakeProgress {
            what,
            percent,
            cancellable,
        }
    }
}

/// The running bake's bar, and its Cancel button during the lightmaps.
pub(super) fn draw_progress(ui: &mut egui::Ui, editor: &mut EditorUi) {
    let Some(active) = &editor.dev.lightmap_bake else {
        return;
    };
    let job = match &active.step {
        Step::Lightmaps(job) => job,
        Step::Probes { job, .. } => return draw_probe_progress(ui, job.stage()),
        Step::Occlusion { job, .. } | Step::OcclusionOnly { job, .. } => {
            return draw_occlusion_progress(ui, job.cells())
        }
    };
    let (done, total) = job.progress();
    let text = match (job.is_cancelled(), job.pages(), total) {
        (true, ..) => "Cancelling…".to_string(),
        // The page being encoded now, of them all (#1694).
        (false, Some((done, n)), _) => format!("Encoding page {} of {n}", (done + 1).min(n)),
        (false, None, 0) => "Preparing…".to_string(),
        (false, None, _) => format!("Lightmaps: {done} / {total} texels"),
    };
    // A progressive bake's pass, its samples so far and the time left (#1970).
    if let Some(pass) = job.passes().filter(|_| job.pages().is_none()) {
        ui.label(format!("Lightmaps {}", pass_line(pass, job.eta())));
    }
    ui.horizontal(|ui| {
        let bar = egui::ProgressBar::new(job.fraction()).text(text);
        ui.add_sized([180.0, 20.0], bar);
        let cancel = egui::Button::new(format!("{}  Cancel", icon::X));
        if ui.add_enabled(!job.is_cancelled(), cancel).clicked() {
            job.cancel();
        }
    });
}

/// The probe step's bar: which pass or set on a line above, `i / n` in the bar, so
/// the text fits the inspector's default width.
fn draw_probe_progress(ui: &mut egui::Ui, stage: Stage) {
    let (done, total, title) = match stage {
        Stage::Light {
            bounce,
            done,
            total,
        } => (done, total, format!("Light probes, bounce {bounce}")),
        Stage::Reflection { done, total } => (done, total, "Reflection probes".to_string()),
        Stage::Done => (1, 1, "Saving…".to_string()),
    };
    ui.label(title);
    let bar =
        egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(format!("{done} / {total}"));
    ui.add_sized([180.0, 20.0], bar);
}

/// The occlusion step's bar: cells baked of the grid's.
fn draw_occlusion_progress(ui: &mut egui::Ui, (done, total): (usize, usize)) {
    ui.label("Occlusion culling");
    let text = match total {
        0 => "Preparing…".to_string(),
        _ => format!("{done} / {total} cells"),
    };
    let bar = egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(text);
    ui.add_sized([180.0, 20.0], bar);
}
