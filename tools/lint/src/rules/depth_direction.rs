use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use crate::modules::module_path;

pub const RULE: &str = "depth-direction";

/// The two homes that say which way a view's depth runs; everything else names them.
const HOMES: [&str; 2] = ["src/scene/camera/depth.rs", "src/render/gpu/mod.rs"];

const COMPARE: &str = "`crate::gpu::depth::NEARER` (or `NEARER_OR_EQUAL`)";
const PROJECTION: &str = "`rusty_scene::camera::depth::perspective` (or `orthographic`)";

/// A spelling of the direction and the helper that says it once. `CompareFunction::Less` also
/// matches `LessEqual`, so one row covers both.
const SPELLINGS: [(&str, &str); 5] = [
    ("CompareFunction::Less", COMPARE),
    ("CompareFunction::Greater", COMPARE),
    ("directx::perspective", PROJECTION),
    ("directx::orthographic", PROJECTION),
    ("LoadOp::Clear(", "`crate::gpu::depth::CLEAR`"),
];

/// A depth clear names a float; a colour clear names a `wgpu::Color`, which passes.
fn spelled(code: &str, form: &str) -> bool {
    code.match_indices(form).any(|(at, _)| {
        let rest = &code[at + form.len()..];
        !form.ends_with('(') || rest.trim_start().starts_with(|c: char| c.is_ascii_digit())
    })
}

pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = module_path(path);
    if !at.starts_with("src/") || HOMES.contains(&at.as_ref()) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (i, code) in bare_lines(source).iter().enumerate() {
        if test.get(i).copied().unwrap_or(false) {
            continue;
        }
        if let Some((form, helper)) = SPELLINGS.iter().find(|(f, _)| spelled(code, f)) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: format!(
                    "`{form}` spells which way depth runs by hand; a camera's depth is reversed \
                     (`rusty_scene::camera::depth`), so name {helper}. A shadow map keeps \
                     ordinary depth: say so with `// lint: allow({RULE}) <why>`"
                ),
            });
        }
    }
    found
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "depth_direction_tests.rs"]
mod tests;
