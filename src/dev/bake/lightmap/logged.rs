//! The lightmap bake with a [`BakeLog`] watching it (#1667): the bake and its
//! page encode (#1694) run on a scoped worker while the calling thread reads its
//! `BakeProgress` and logs, so a script's `Lighting.BakeLightmaps()` / `Generate()`
//! reports as it goes, a line per encoded page included. The worker is the same
//! `bake_with_progress` and `encode_pages` the editor's job runs, so the pages are
//! byte-identical with or without a log.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{encode_pages, EncodedPages};
use crate::dev::bake::progress::{BakeLog, Progress, Step};
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::lighting::lightmap::{bake_with_progress, BakeProgress, BakeScene};

/// How often the calling thread reads the counters (the log decides what prints).
const POLL: Duration = Duration::from_millis(200);

/// Bake and encode `input`, reporting into `log`. An off log runs in line, with
/// no worker.
pub(super) fn bake_logged(
    input: &BakeScene,
    settings: &BakeSettings,
    log: &mut BakeLog,
) -> Result<EncodedPages, String> {
    let progress = BakeProgress::default();
    let run = |progress: &BakeProgress| {
        // Nobody cancels this one, so it always runs to the end.
        let maps = bake_with_progress(input, settings, progress).unwrap_or_default();
        encode_pages(&maps, settings.page(), progress)?.ok_or_else(|| "never cancelled".to_string())
    };
    if !log.is_on() {
        return run(&progress);
    }
    let watched = &progress;
    let (sent, result) = mpsc::channel();
    let encoded = thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let _ = sent.send(run(watched));
        });
        loop {
            log.observe(read(&progress));
            match result.recv_timeout(POLL) {
                Ok(encoded) => break encoded,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                // The worker panicked before sending: re-raise it here.
                Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
                    Ok(()) => break Err("the bake worker stopped".into()),
                    Err(panic) => std::panic::resume_unwind(panic),
                },
            }
        }
    });
    log.observe(read(&progress));
    encoded
}

/// The bake's counters as the log reads them: the trace's, then the encode's.
fn read(progress: &BakeProgress) -> Progress {
    if let Some((done, total)) = progress.pages() {
        return Progress {
            step: Step::LightmapPages,
            done,
            total,
            meshes: None,
        };
    }
    Progress {
        step: Step::Lightmaps,
        done: progress.done(),
        total: progress.total(),
        meshes: Some((progress.meshes_done(), progress.meshes_total())),
    }
}
