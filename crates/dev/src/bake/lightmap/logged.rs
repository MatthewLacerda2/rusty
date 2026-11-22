use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use super::{encode_pages, lend_denoiser, EncodedPages};
use crate::bake::progress::{BakeLog, MeshClocks, Progress, Step};
use crate::bake::report::lightmaps::Measured;
use rusty_lighting::lightmap::{bake_with_progress, BakeProgress, BakeScene};
use rusty_scene::lighting::settings::BakeSettings;

const POLL: Duration = Duration::from_millis(200);

pub(super) struct Run {
    pub pages: EncodedPages,
    pub trace: Duration,
    pub encode: Duration,
    pub measured: Measured,
}

pub(super) fn bake_logged(
    input: &BakeScene,
    settings: &BakeSettings,
    log: &mut BakeLog,
    scene_path: &str,
) -> Result<Run, String> {
    let run = |progress: &BakeProgress| {
        timed((input, settings), progress, scene_path)?.ok_or_else(|| "never cancelled".to_string())
    };
    let clocks = Arc::new(MeshClocks::default());
    let (progress, oidn) = lend_denoiser(BakeProgress::observed(clocks.clone()), settings);
    let (pages, trace, encode) = match log.is_on() {
        true => watch(&progress, read, log, run)?,
        false => run(&progress)?,
    };
    let denoised = oidn.map(|o| o.spent());
    if let Some((charts, busy)) = denoised {
        log.denoised(charts, busy);
    }
    let peak_memory = crate::memory::rss_peak_bytes();
    if let Some(bytes) = peak_memory {
        log.peak_memory(bytes);
    }
    let measured = Measured {
        times: clocks.times(),
        denoised,
        peak_memory,
    };
    Ok(Run {
        pages,
        trace,
        encode,
        measured,
    })
}

pub(super) fn timed(
    (input, settings): (&BakeScene, &BakeSettings),
    progress: &BakeProgress,
    scene_path: &str,
) -> Result<Option<(EncodedPages, Duration, Duration)>, String> {
    let started = Instant::now();
    let Some(maps) = bake_with_progress(input, settings, progress) else {
        return Ok(None);
    };
    let traced = Instant::now();
    let pages = encode_pages(maps, settings.page(), progress, scene_path)?;
    Ok(pages.map(|p| (p, traced - started, traced.elapsed())))
}

pub(in crate::bake) fn watch<P: Sync, T: Send>(
    progress: &P,
    read: fn(&P) -> Progress,
    log: &mut BakeLog,
    work: impl FnOnce(&P) -> Result<T, String> + Send,
) -> Result<T, String> {
    log.observe(read(progress));
    let (sent, result) = mpsc::channel();
    let done = thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let _ = sent.send(work(progress));
        });
        loop {
            match result.recv_timeout(POLL) {
                Ok(done) => break done,
                Err(mpsc::RecvTimeoutError::Timeout) => log.observe(read(progress)),
                Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
                    Ok(()) => break Err("the bake worker stopped".into()),
                    Err(panic) => std::panic::resume_unwind(panic),
                },
            }
        }
    });
    log.observe(read(progress));
    done
}

pub(super) fn read(progress: &BakeProgress) -> Progress {
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
