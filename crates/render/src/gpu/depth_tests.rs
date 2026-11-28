use rusty_scene::camera::depth::{in_front, FAR, NEAR};

use super::depth::{CLEAR, NEARER, NEARER_OR_EQUAL};

/// `const <name>: f32 = <value>;` in the WGSL `depth` module.
fn wgsl_const(source: &str, name: &str) -> f32 {
    let line = source
        .lines()
        .find(|l| l.starts_with(&format!("const {name}: f32 = ")))
        .unwrap_or_else(|| panic!("depth.wgsl declares no `{name}`"));
    let value = line.rsplit("= ").next().unwrap().trim_end_matches(';');
    value.parse().unwrap_or_else(|e| panic!("{line}: {e}"))
}

#[test]
fn the_shaders_the_compares_and_the_clear_agree_with_the_camera() {
    let path = format!("{}/depth.wgsl", rusty_shadergen::engine_shader_dir());
    let source = std::fs::read_to_string(&path).expect("engine/shaders/depth.wgsl");
    assert_eq!(wgsl_const(&source, "NEAR"), NEAR);
    assert_eq!(wgsl_const(&source, "FAR"), FAR);
    assert!(
        matches!(CLEAR, wgpu::LoadOp::Clear(d) if d == FAR),
        "the clear is the far plane"
    );
    // The compare a nearer fragment passes: the near plane's depth against the far plane's.
    let passes = |f: wgpu::CompareFunction, a: f32, b: f32| match f {
        wgpu::CompareFunction::Greater => a > b,
        wgpu::CompareFunction::GreaterEqual => a >= b,
        wgpu::CompareFunction::Less => a < b,
        wgpu::CompareFunction::LessEqual => a <= b,
        other => panic!("{other:?} is no depth direction"),
    };
    assert!(in_front(NEAR, FAR) && passes(NEARER, NEAR, FAR) && !passes(NEARER, FAR, NEAR));
    assert!(!passes(NEARER, FAR, FAR) && passes(NEARER_OR_EQUAL, FAR, FAR));
    assert!(passes(NEARER_OR_EQUAL, NEAR, FAR) && !passes(NEARER_OR_EQUAL, FAR, NEAR));
}
