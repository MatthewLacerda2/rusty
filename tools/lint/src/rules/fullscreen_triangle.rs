use super::finding::{self, Finding};
use super::scan::Scan;

pub const RULE: &str = "fullscreen-triangle";
const HOME: &str = "engine/shaders/fullscreen.wgsl";

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

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.shaders()
        .iter()
        .flat_map(|(path, source)| judged(path, source))
        .collect()
}

pub fn judged(path: &str, source: &str) -> Vec<Finding> {
    let mut found = check(path, source);
    found.retain(|f| !finding::allowed(source, f.line, f.rule));
    found.extend(finding::unexplained(path, source));
    found
}

#[cfg(test)]
#[path = "fullscreen_triangle_tests.rs"]
mod tests;
