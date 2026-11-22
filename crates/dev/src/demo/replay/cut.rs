use std::path::{Path, PathBuf};

/// As `Harness.DemoPath()` answers it, so a cut's check matches however the demo was named.
pub fn full_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn scenario(demo: &Path, from: u64, to: u64) -> String {
    let full = full_path(demo);
    let demo = full.display();
    // A Rust string literal's escapes read the same in Lua: `\\`, `\"`, `\u{..}`.
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
