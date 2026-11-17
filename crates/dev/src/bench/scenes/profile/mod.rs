//! `PROFILE=1`: which function inside a stage costs the time. A scene is loaded
//! and each viewpoint warmed up as a timed run does, then the platform's sampling
//! profiler records this process for that viewpoint's measured frames only
//! ([`perf`] on Linux, [`sample`] on macOS), and the viewpoints' samples are
//! folded together ([`folded`]). Kept beside the scene's report:
//! `<scene>.profile.txt`, the top functions by self and by total time, and
//! `<scene>.folded`, the raw folded stacks for a flame graph.
//!
//! A run of its own, after the timed ones: the profiler's overhead never reaches
//! a timing. A release build with line tables (`make bench-scenes PROFILE=1`
//! builds the `profiling` profile), so the stacks name functions and lines. No
//! profiler on `PATH` is an error naming the package to install, never a skip.

pub mod folded;
pub mod perf;
pub mod sample;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use rusty_core::project::ProjectSettings;

use super::measure;
use super::{load, views, Options};
use folded::Folded;

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
}

impl Recording {
    fn stop(self) -> Result<Folded, String> {
        match self {
            Self::Perf(r) => r.stop(),
            Self::Sample(r) => r.stop(),
        }
    }
}

/// Profile `scene` of the project at `root` with `profiler`: each viewpoint's
/// measured frames, folded, written beside its report in `out_dir`. Returns the
/// path of the tables.
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
    let mut stacks = Folded::default();
    let mut event = String::new();
    for viewpoint in &picked.views {
        let mut recording = None;
        let mut started = || {
            recording = Some(profiler.start(&scratch));
        };
        measure::one_with(&mut harness, None, viewpoint, opts, &mut started);
        let recording = recording.ok_or("the profiler never started")??;
        if let Recording::Perf(r) = &recording {
            event = format!(", {}", r.event);
        }
        stacks.merge(recording.stop()?);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let head = format!(
        "bench-scenes profile: {scene} — {} samples over {} viewpoints x {} frames ({}{event})",
        stacks.samples(),
        picked.views.len(),
        opts.frames,
        profiler.name(),
    );
    let tables = out_dir.join(format!("{scene}.profile.txt"));
    let write = |path: &Path, text: String| {
        // lint: allow(asset-write) a bench profile, not read by a path-keyed cache
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    };
    write(&tables, stacks.tables(&head))?;
    write(&out_dir.join(format!("{scene}.folded")), stacks.text())?;
    println!("{head}\n  {}", tables.display());
    Ok(tables)
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
