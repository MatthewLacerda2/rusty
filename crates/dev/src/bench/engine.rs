use std::fmt::Write;

use rusty_core::project::EngineStamp;

pub(super) fn engine_line(engine: &EngineStamp, previous: Option<&EngineStamp>) -> String {
    let mut line = format!("engine {} {}", engine.commit, trees(engine));
    let moved = |p: &&EngineStamp| {
        (&p.content_found, &p.content_built) != (&engine.content_found, &engine.content_built)
    };
    if let Some(before) = previous.filter(moved) {
        let _ = write!(line, " (last run: {})", trees(before));
    }
    line.push('\n');
    if let Some(before) = previous.filter(|p| p.content_matches == Some(false)) {
        let _ = writeln!(
            line,
            "warning: the last run was not built with the engine content it read ({}): \
             its numbers paired one build's code with another tree's files, and the \
             changes above are measured against them",
            trees(before)
        );
    }
    line
}

fn trees(e: &EngineStamp) -> String {
    let digest = |d: &Option<String>| d.clone().unwrap_or_else(|| "unknown".into());
    let mut out = format!("content {}", digest(&e.content_found));
    if e.content_matches == Some(false) {
        let _ = write!(out, ", built {}", digest(&e.content_built));
    }
    out
}
