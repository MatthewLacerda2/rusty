//! The lightmap bake with a [`BakeLog`] watching it (#1667): the bake runs on a
//! scoped worker while the calling thread reads its `BakeProgress` and logs, so a
//! script's `Lighting.BakeLightmaps()` / `Generate()` reports as it goes. The worker
//! is the same `bake_with_progress` the editor's job runs, so the lightmaps are
//! byte-identical with or without a log.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::dev::bake::progress::{BakeLog, Progress, Step};
use crate::scene::lighting::lightmap::{bake, bake_with_progress, BakeProgress, BakeScene};
use crate::scene::lighting::lightmap::{BakeSettings, Lightmap};

/// How often the calling thread reads the counters (the log decides what prints).
const POLL: Duration = Duration::from_millis(200);

/// Bake `input`, reporting into `log`. An off log bakes in line, with no worker.
pub(super) fn bake_logged(
    input: &BakeScene,
    settings: &BakeSettings,
    log: &mut BakeLog,
) -> Vec<Lightmap> {
    if !log.is_on() {
        return bake(input, settings);
    }
    let progress = BakeProgress::default();
    let watched = &progress;
    let (sent, result) = mpsc::channel();
    let maps = thread::scope(|scope| {
        let worker = scope.spawn(move || {
            // Nobody cancels this one, so it always runs to the end.
            let _ = sent.send(bake_with_progress(input, settings, watched));
        });
        loop {
            log.observe(read(&progress));
            match result.recv_timeout(POLL) {
                Ok(maps) => break maps,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                // The worker panicked before sending: re-raise it here.
                Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
                    Ok(()) => break None,
                    Err(panic) => std::panic::resume_unwind(panic),
                },
            }
        }
    });
    log.observe(read(&progress));
    maps.unwrap_or_default()
}

/// The bake's counters as the log reads them.
fn read(progress: &BakeProgress) -> Progress {
    Progress {
        step: Step::Lightmaps,
        done: progress.done(),
        total: progress.total(),
        meshes: Some((progress.meshes_done(), progress.meshes_total())),
    }
}
