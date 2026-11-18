//! `PROFILE=1`: which function inside a stage costs the time. A scene is loaded
//! and each viewpoint warmed up and measured as a timed run does, while the
//! platform's sampling profiler records this process, and only the samples of
//! the measured frames are kept: [`perf`] on Linux records the whole scene once
//! and is cut to the measured windows by timestamp ([`window`]); [`sample`] on
//! macOS, whose samples carry no time, is attached for each viewpoint's measured
//! frames. The samples are folded together ([`folded`]). Kept beside the scene's
//! report: `<scene>.profile.txt`, the top functions by self and by total time,
//! and `<scene>.folded`, the raw folded stacks for a flame graph.
//!
//! A profile that did not record soundly (too few samples, or too few that name
//! a function of the bench's own binary) is refused ([`trust`]): its file and
//! the console say it is not to be trusted and why, and no tables are written.
//!
//! A run of its own, after the timed ones: the profiler's overhead never reaches
//! a timing. A release build with line tables and frame pointers (`make
//! bench-scenes PROFILE=1` builds the `profiling` profile), so the stacks name
//! functions and walk cheaply. No profiler on `PATH` is an error naming the
//! package to install, never a skip.

pub mod folded;
pub mod perf;
pub mod sample;
pub mod trust;
pub mod window;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusty_core::project::ProjectSettings;

use super::super::super::harness::Harness;
use super::measure;
use super::views::Viewpoint;
use super::{load, views, Options};
use folded::Folded;
use trust::Tally;
use window::Window;

/// The platform's sampling profiler, found on `PATH`.
#[derive(Clone, Debug, PartialEq)]
pub enum Profiler {
    Perf(PathBuf),
    Sample(PathBuf),
}

/// The profiler this platform profiles with, looked up on the `PATH` `path`
/// (`std::env::var_os("PATH")`); an error naming what to install without it.
pub fn find(path: Option<&OsStr>) -> Result<Profiler, String> {
    let on_path = |name: &str| {
        let dirs = path.map(std::env::split_paths).into_iter().flatten();
        dirs.map(|d| d.join(name)).find(|p| p.is_file())
    };
    if cfg!(target_os = "macos") {
        return on_path("sample").map(Profiler::Sample).ok_or_else(|| {
            "PROFILE=1 needs `sample`, part of macOS (/usr/bin/sample): it is not on PATH"
                .to_string()
        });
    }
    on_path("perf").map(Profiler::Perf).ok_or_else(|| {
        "PROFILE=1 needs `perf`, which is not on PATH: install it (Debian: linux-perf; \
         Ubuntu: linux-tools-generic and linux-tools-$(uname -r); Fedora and Arch: perf)"
            .to_string()
    })
}

/// Whether `profiler` may record this process here: started and stopped once,
/// before any scene, so a refusal (a stricter `kernel.perf_event_paranoid`)
/// ends the run with what to set instead of failing every scene's profile.
/// `scratch` holds the throwaway recording and is removed.
pub fn probe(profiler: &Profiler, scratch: &Path) -> Result<(), String> {
    std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
    // What a started recording folds is thrown away: the probe asks only whether it starts.
    let stopped = profiler.start(scratch).map(|r| drop(r.stop(&[], "")));
    let _ = std::fs::remove_dir_all(scratch);
    stopped
}

/// A profiler recording this process.
enum Recording {
    Perf(perf::Recording),
    Sample(sample::Recording),
}

impl Profiler {
    fn start(&self, scratch: &Path) -> Result<Recording, String> {
        match self {
            Self::Perf(bin) => perf::start(bin, &scratch.join("perf.data")).map(Recording::Perf),
            Self::Sample(bin) => {
                let report = scratch.join("sample.txt");
                sample::start(bin, &report).map(Recording::Sample)
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Perf(_) => "perf",
            Self::Sample(_) => "sample",
        }
    }

    /// Whether one recording covers the whole scene, cut to the measured
    /// windows by its samples' timestamps; else one is made per window.
    fn cuts_by_time(&self) -> bool {
        matches!(self, Self::Perf(_))
    }

    /// Samples a second, for the count a sound recording should reach.
    fn hz(&self) -> u32 {
        match self {
            Self::Perf(_) => perf::HZ,
            Self::Sample(_) => sample::HZ,
        }
    }
}

impl Recording {
    /// Stop and fold the samples inside `windows` (a per-window recording keeps
    /// all of its own), a leaf in the image named `own` counted as the bench's.
    fn stop(self, windows: &[Window], own: &str) -> Result<(Folded, Tally), String> {
        match self {
            Self::Perf(r) => r.stop(windows, own),
            Self::Sample(r) => r.stop(own),
        }
    }
}

/// Profile `scene` of the project at `root` with `profiler`: each viewpoint's
/// measured frames, folded, written beside its report in `out_dir`, or refused
/// when the recording is not sound. Returns the path of the profile's text.
pub fn run(
    profiler: &Profiler,
    (root, scene): (&Path, &str),
    settings: ProjectSettings,
    out_dir: &Path,
    opts: Options,
) -> Result<PathBuf, String> {
    let (mut harness, _) = load(out_dir, root, scene, settings, opts.size)?;
    let picked = {
        let world = harness.world.borrow();
        let scene = world.scene().borrow();
        let nav = world.resources.nav.borrow();
        views::pick(&scene, &nav)
    };
    let scratch = out_dir.join(format!("{scene}.profiling"));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let recorded = record(profiler, &mut harness, &picked.views, &scratch, opts);
    let _ = std::fs::remove_dir_all(&scratch);
    let Recorded {
        stacks,
        tally,
        windows,
        event,
    } = recorded?;
    let measured = window::seconds(&windows);
    let mut head = format!(
        "bench-scenes profile: {scene} — {} samples over {} viewpoints x {} frames, \
         {measured:.1} s measured ({}{event})",
        tally.samples,
        picked.views.len(),
        opts.frames,
        profiler.name(),
    );
    let expected = measured * f64::from(profiler.hz());
    let refused = trust::refusal(&tally, expected);
    let tables = tables_path(out_dir, scene);
    let folded = out_dir.join(format!("{scene}.folded"));
    let write = |path: &Path, text: String| {
        // lint: allow(asset-write) a bench profile, not read by a path-keyed cache
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    };
    if let Some(refused) = refused {
        head = format!("{head}\n{refused}");
        write(&tables, format!("{head}\n"))?;
        // A stale flame graph from an earlier run would read as this one's.
        let _ = std::fs::remove_file(&folded);
        println!("{head}\n  {}", tables.display());
        return Ok(tables);
    }
    head = format!("{head}\nwhere the samples landed: {}", tally.images_line());
    for warning in &tally.warnings {
        head = format!("{head}\nwarning: {warning}");
    }
    write(&tables, stacks.tables(&head))?;
    write(&folded, stacks.text())?;
    println!("{head}\n  {}", tables.display());
    Ok(tables)
}

/// Where `scene`'s profile text is kept in `out_dir`.
pub fn tables_path(out_dir: &Path, scene: &str) -> PathBuf {
    out_dir.join(format!("{scene}.profile.txt"))
}

/// The closing lines naming each of `scenes` whose profile in `out_dir` refused
/// itself, with the opening of its first reason; `None` when none did. Said,
/// not failed: the recording ran, and its file says why it is not to be read.
pub fn refusals<'a>(out_dir: &Path, scenes: impl Iterator<Item = &'a String>) -> Option<String> {
    let refused = |scene: &String| {
        let text = std::fs::read_to_string(tables_path(out_dir, scene)).ok()?;
        trust::refusal_gist(&text).map(|why| format!("\n  {scene}: {why}"))
    };
    let lines: String = scenes.filter_map(refused).collect();
    (!lines.is_empty()).then(|| format!("profile not to be trusted (see its .profile.txt):{lines}"))
}

/// Whether this binary is the `profiling` build (cargo puts a profile's
/// binaries in a folder named after it), whose timings a report says it is on.
pub fn on_profiling_build() -> bool {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().and_then(Path::file_name) == Some(OsStr::new("profiling"))
}

/// What [`record`] gives: the folded samples, their tally, the measured windows
/// and the event perf sampled (`, cycles:u`).
struct Recorded {
    stacks: Folded,
    tally: Tally,
    windows: Vec<Window>,
    event: String,
}

impl Recorded {
    /// Stop `r` and keep what it sampled inside `windows`.
    fn keep(&mut self, r: Recording, windows: &[Window], own: &str) -> Result<(), String> {
        if let Recording::Perf(p) = &r {
            self.event = format!(", {}", p.event);
        }
        let (folded, tally) = r.stop(windows, own)?;
        self.stacks.merge(folded);
        self.tally.merge(tally);
        Ok(())
    }
}

/// Measure every viewpoint of `views` while `profiler` records: once over the
/// whole scene when it cuts by time, else once per measured window.
fn record(
    profiler: &Profiler,
    harness: &mut Harness,
    views: &[Viewpoint],
    scratch: &Path,
    opts: Options,
) -> Result<Recorded, String> {
    let own = trust::own_image();
    let mut out = Recorded {
        stacks: Folded::default(),
        tally: Tally::default(),
        windows: Vec::new(),
        event: String::new(),
    };
    let whole = match profiler.cuts_by_time() {
        true => Some(profiler.start(scratch)?),
        false => None,
    };
    for viewpoint in views {
        let (mut recording, mut start) = (None, 0);
        let mut started = || {
            if whole.is_none() {
                recording = Some(profiler.start(scratch));
            }
            start = window::now();
        };
        measure::one_with(harness, None, viewpoint, opts, &mut started);
        let measured = Window {
            start,
            end: window::now(),
        };
        out.windows.push(measured);
        if whole.is_none() {
            let r = recording.ok_or("the profiler never started")??;
            out.keep(r, &[measured], &own)?;
        }
    }
    if let Some(r) = whole {
        let windows = out.windows.clone();
        out.keep(r, &windows, &own)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_profiler_on_path_is_an_error_naming_what_to_install() {
        let empty = rusty_test_support::temp::dir().join(format!("nopath_{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let err = find(Some(empty.as_os_str())).unwrap_err();
        let package = if cfg!(target_os = "macos") {
            "sample"
        } else {
            "linux-perf"
        };
        assert!(err.contains(package), "{err}");
        assert!(find(None).is_err(), "no PATH at all");
    }
}
