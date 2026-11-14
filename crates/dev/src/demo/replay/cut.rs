//! Cutting a scenario out of a demo (#1597): `Harness.CutDemo(from, to, path)`
//! writes a scenario that reproduces ticks `from` to `to` of the demo being
//! replayed, so a moment found in a playtest becomes a regression test.
//!
//! The scenario copies no world state: it runs on the source demo (`play --demo
//! <demo> <scenario> <out>`, named in its header), checks that it does
//! (`Harness.DemoPath()`, the replayed demo's full path), seeks to the tick
//! before `from`, steps the range on the recorded input, and ends with an assert
//! stub to fill in. The replay's own check (`the demo replays bit for bit as far as
//! the scenario stepped it`) proves the range came out the same.

use std::path::{Path, PathBuf};

/// `path` in full, as `Harness.DemoPath()` answers it: the same file however it
/// was named on the command line.
pub fn full_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The scenario reproducing ticks `from..=to` of the demo at `demo`.
pub fn scenario(demo: &Path, from: u64, to: u64) -> String {
    let full = full_path(demo);
    let demo = full.display();
    // A Rust string literal reads the same in Lua: `\\`, `\"`, `\u{..}`.
    let named = format!("{:?}", full.to_string_lossy());
    let seek = match from {
        0 => String::new(),
        _ => format!(
            "Harness.SeekDemo({})  -- the demo up to tick {from}\n",
            from - 1
        ),
    };
    // lint: allow(markers) the stub is the generated scenario's, for its author to fill in
    let stub = format!("Harness.Expect(true, \"TODO: what should hold at tick {to}\")\n");
    format!(
        "-- Ticks {from} to {to} of the demo {demo} (Harness.CutDemo, #1597).\n\
         -- It runs on that demo, which must stay where it is:\n\
         --   play --demo {demo} <this file> <out_dir>\n\
         assert(Harness.DemoPath() == {named}, \"not on its demo: play --demo \" .. {named})\n\
         {seek}\
         Harness.Step({steps})  -- ticks {from} to {to}, on their recorded input\n\
         {stub}",
        steps = to - from + 1,
    )
}

/// Write the cut of ticks `from..=to` of `demo` (which holds `ticks`) to `path`.
pub fn cut(demo: &Path, ticks: u64, from: u64, to: u64, path: &Path) -> Result<(), String> {
    if from > to || to >= ticks {
        return Err(format!(
            "ticks {from} to {to} are not a range of the demo: it holds ticks 0 to {}",
            ticks.saturating_sub(1)
        ));
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // lint: allow(asset-write) a scenario the agent asked for, never read by a path-keyed cache
    std::fs::write(path, scenario(demo, from, to)).map_err(|e| format!("{}: {e}", path.display()))
}
