use super::RULE;
use crate::rules::{allowed, test_code::test_lines, Finding};

fn check_file(path: &str, src: &str) -> Vec<Finding> {
    super::check_file(path, src, &test_lines(path, src, false))
}

const PASS: &str = "    let p = encoder.begin_render_pass(&desc);\n";

#[test]
fn a_raw_call_in_a_render_module_is_flagged_naming_its_helper() {
    let found = check_file(
        "crates/render/src/passes/a.rs",
        &format!("fn f() {{\n{PASS}}}\n"),
    );
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].rule, found[0].line), (RULE, 2));
    assert!(found[0].message.contains("counted_render_pass"));
    let all = "q.submit(x);\nq.write_buffer(b, 0, d);\nd.create_bind_group(&g);\ne.begin_compute_pass(&c);\n";
    assert_eq!(check_file("crates/render/src/lib.rs", all).len(), 4);
    let texture = check_file(
        "crates/render/src/gpu/lightmaps/upload.rs",
        "q.write_texture(to, d, l, s);\n",
    );
    assert_eq!(texture.len(), 1);
    assert!(texture[0].message.contains("counted_write_texture"));
}

#[test]
fn a_path_form_call_is_flagged_too() {
    let path = "wgpu::Queue::submit(&q, x);\nwgpu::Device::create_bind_group(d, &desc);\n";
    let found = check_file("crates/render/src/passes/a.rs", path);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].message.contains("`::submit(`"));
    assert!(found[1].message.contains("counted_bind_group"));
}

#[test]
fn the_helpers_tests_comments_and_other_crates_pass() {
    assert!(check_file("crates/render/src/gpu/ops.rs", PASS).is_empty());
    assert!(check_file("crates/render/src/passes/a_tests.rs", PASS).is_empty());
    let gated = format!("#[cfg(test)]\nfn t() {{\n{PASS}}}\nfn g() {{}}\n");
    assert!(check_file("crates/render/src/passes/a.rs", &gated).is_empty());
    let doc = "/// a raw `encoder.begin_render_pass(` is counted nowhere\nlet s = \".submit(\";\n";
    assert!(check_file("crates/render/src/passes/a.rs", doc).is_empty());
    assert!(check_file("crates/shell/src/editor/paint.rs", PASS).is_empty());
    let counted = "    let p = encoder.counted_render_pass(&desc);\nq.counted_submit(x);\nq.counted_write_texture(s, to, d, l, z);\n";
    assert!(check_file("crates/render/src/passes/a.rs", counted).is_empty());
    let custom = "ring.resolve_frame(device, queue);\nself.submit_copy(x);\n";
    assert!(check_file("crates/render/src/passes/a.rs", custom).is_empty());
}

#[test]
fn a_reasoned_allow_excuses_a_raw_call() {
    let src = format!("// lint: allow(gpu-ops) a probe timed outside any frame\n{PASS}");
    let found = check_file("crates/render/src/timing/clock.rs", &src);
    assert!(allowed(&src, found[0].line, RULE));
}
