//! `gpu-ops`: the renderer's GPU calls are counted (#2341).
//!
//! Every render and compute pass the renderer begins, every queue submit, bind
//! group made and buffer write goes through `crates/render/src/gpu/ops.rs`
//! (`CountedEncoder`, `CountedQueue`, `CountedDevice`), which counts it into the
//! frame's `RenderCounters::ops` (the `render_passes`, `compute_passes`, `submits`,
//! `bind_groups_created` and `buffer_writes` bench rows), and every texture write,
//! whose bytes it counts into the renderer's `Staging`, which bounds what wgpu holds
//! of them until a submit. A raw wgpu call elsewhere in `crates/render/src/` would
//! go uncounted, so the rows would lie or the staging grow unbounded: it is a
//! finding naming the helper. Test code is out of scope (it is no frame's work),
//! and so is everything outside the render crate: the editor's egui paint passes
//! in `crates/shell/src/editor/` belong to no frame's counters.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use crate::modules::module_path;

pub const RULE: &str = "gpu-ops";
const SCOPE: &str = "src/render/";
const HOME: &str = "src/render/gpu/ops.rs";

/// What a frame-counted call goes uncounted in.
const ROWS: &str = "the frame's GPU-call rows";

/// Each raw wgpu method, the counted helper it should be, and what it goes
/// uncounted in. A call is flagged in its method form (`.submit(`) and its path
/// form (`wgpu::Queue::submit(`).
const CALLS: [(&str, &str, &str); 6] = [
    ("begin_render_pass", "counted_render_pass", ROWS),
    ("begin_compute_pass", "counted_compute_pass", ROWS),
    ("submit", "counted_submit", ROWS),
    ("create_bind_group", "counted_bind_group", ROWS),
    ("write_buffer", "counted_write_buffer", ROWS),
    (
        "write_texture",
        "counted_write_texture",
        "the staging bound, so its bytes pile up until a submit (`gpu::ops::Staging`)",
    ),
];

/// Every raw counted-kind wgpu call in the production code of `path` (`test`, per line).
pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = module_path(path);
    if !at.starts_with(SCOPE) || at == HOME {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (i, code) in bare_lines(source).iter().enumerate() {
        if test.get(i).copied().unwrap_or(false) {
            continue;
        }
        for (name, helper, missed) in CALLS {
            let forms = [format!(".{name}("), format!("::{name}(")];
            if let Some(raw) = forms.iter().find(|f| code.contains(f.as_str())) {
                found.push(Finding {
                    path: path.to_string(),
                    line: i + 1,
                    rule: RULE,
                    message: format!(
                        "`{raw}` goes uncounted in {missed}; call \
                         `.{helper}(` from `crate::gpu::ops` instead (import its trait \
                         `as _`)"
                    ),
                });
            }
        }
    }
    found
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "gpu_ops_tests.rs"]
mod tests;
