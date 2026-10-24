//! `fullscreen-triangle`: the fullscreen triangle is written once (#1370).
//!
//! Every fullscreen pass draws three vertices with no vertex buffer, and the vertex
//! maths lives in `engine/shaders/fullscreen.wgsl`, the composable module a pass
//! `#import`s (`src/render/CLAUDE.md`, *Shaders*). A shader that spells the
//! triangle's bit-twiddle itself (`(i << 1u) & 2u`, the line every copy had) is a
//! finding; the fix is `fullscreen::triangle` / `fullscreen::uv`. A shader that must
//! stand alone says why with `// lint: allow(fullscreen-triangle) <why>`.
//!
//! The rule reads `engine/**/*.wgsl` itself: `Scan` holds only Rust. Shaders written
//! inside Rust strings (`shadergen`'s baked post-FX module, which is self-contained
//! by design) are out of its reach.

use super::finding::Finding;
use super::scan::Scan;
use std::fs;
use std::path::Path;

pub const RULE: &str = "fullscreen-triangle";
const HOME: &str = "engine/shaders/fullscreen.wgsl";

/// Every code line of a shader that spells the triangle's vertex-index maths.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if path == HOME {
        return Vec::new();
    }
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            // comment-cut: WGSL has no strings, so everything before `//` is code
            let code = line.split("//").next().unwrap_or("");
            let squeezed: String = code.split_whitespace().collect();
            squeezed.contains("<<1u)&2u")
        })
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "the fullscreen triangle lives in engine/shaders/fullscreen.wgsl; \
                      `#import fullscreen` and call `fullscreen::triangle` or `fullscreen::uv`"
                .to_string(),
        })
        .collect()
}

/// The rule over every shader under `engine/`.
pub fn run(_: &Scan) -> Vec<Finding> {
    crate::size::shaders_in(Path::new("engine"))
        .iter()
        .flat_map(|p| {
            let path = p.to_string_lossy().replace('\\', "/");
            check(&path, &fs::read_to_string(p).unwrap_or_default())
        })
        .collect()
}

#[cfg(test)]
#[path = "fullscreen_triangle_tests.rs"]
mod tests;
