use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;
use crate::modules::module_path;

pub const RULE: &str = "gpu-ops";
const SCOPE: &str = "src/render/";
const HOME: &str = "src/render/gpu/ops.rs";

const ROWS: &str = "the frame's GPU-call rows";

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

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "gpu_ops_tests.rs"]
mod tests;
