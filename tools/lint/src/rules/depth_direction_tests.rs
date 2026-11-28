use super::RULE;
use crate::rules::{allowed, test_code::test_lines, Finding};

fn check_file(path: &str, src: &str) -> Vec<Finding> {
    super::check_file(path, src, &test_lines(path, src, false))
}

const PASS: &str = "crates/render/src/passes/a.rs";

#[test]
fn a_hand_spelled_compare_clear_or_projection_is_flagged() {
    let src = "fn f() {\n    let c = wgpu::CompareFunction::LessEqual;\n    let l = wgpu::LoadOp::Clear(1.0);\n    let p = directx::perspective(f, a, n, z);\n    let o = directx::orthographic(l, r, b, t, n, z);\n    let g = wgpu::CompareFunction::Greater;\n}\n";
    let found = check_file(PASS, src);
    let lines: Vec<usize> = found.iter().map(|f| f.line).collect();
    assert_eq!(lines, [2, 3, 4, 5, 6], "{found:?}");
    assert!(found.iter().all(|f| f.rule == RULE));
    assert!(found[0].message.contains("NEARER"));
    assert!(found[1].message.contains("depth::CLEAR"));
    assert!(found[2].message.contains("camera::depth::perspective"));
    assert_eq!(
        check_file("crates/lighting/src/v.rs", src).len(),
        5,
        "every crate"
    );
}

#[test]
fn the_helpers_colour_clears_tests_and_comments_pass() {
    let src = "let c = wgpu::CompareFunction::Less;\nlet l = wgpu::LoadOp::Clear(0.0);\n";
    assert!(check_file("crates/render/src/gpu/mod.rs", src).is_empty());
    assert!(check_file(
        "crates/scene/src/camera/depth.rs",
        "directx::perspective(f, a, z, n)\n"
    )
    .is_empty());
    assert!(check_file("crates/render/src/passes/a_tests.rs", src).is_empty());
    let gated = format!("#[cfg(test)]\nfn t() {{\n{src}}}\nfn g() {{}}\n");
    assert!(check_file(PASS, &gated).is_empty());
    let named = "let c = crate::gpu::depth::NEARER;\nlet l = crate::gpu::depth::CLEAR;\nlet k = wgpu::LoadOp::Clear(wgpu::Color::BLACK);\nlet a = wgpu::CompareFunction::Always;\n/// `CompareFunction::Less` was the old way\n";
    assert!(check_file(PASS, named).is_empty());
}

#[test]
fn a_reasoned_allow_excuses_a_shadow_map() {
    let src = "// lint: allow(depth-direction) a shadow map keeps ordinary depth\nlet c = wgpu::CompareFunction::Less;\n";
    let found = check_file("crates/render/src/passes/shadows/setup.rs", src);
    assert!(allowed(src, found[0].line, RULE));
}
