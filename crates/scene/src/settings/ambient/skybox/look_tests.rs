use super::procedural as cpu;
use crate::settings::sky;

const LOOK: &str = include_str!("../../../../../../engine/shaders/sky/look.wgsl");

fn wgsl_value(name: &str) -> f32 {
    let head = format!("const {name}: f32 = ");
    let line = LOOK
        .lines()
        .find_map(|l| l.strip_prefix(&head))
        .unwrap_or_else(|| panic!("sky/look.wgsl should declare `{head}…;`"));
    line.trim_end_matches(';').parse().expect("an f32 literal")
}

#[test]
fn every_sky_look_constant_is_one_number_in_rust_and_wgsl() {
    let mirrors = [
        ("SKY_HORIZON_TO_WHITE", sky::SKY_HORIZON_TO_WHITE),
        ("SKY_GROUND_OF_ZENITH", sky::SKY_GROUND_OF_ZENITH),
        ("SKY_ZENITH_EXPONENT", cpu::SKY_ZENITH_EXPONENT),
        ("SKY_GROUND_EXPONENT", cpu::SKY_GROUND_EXPONENT),
        ("CLOUD_DOME_MIN_HEIGHT", cpu::CLOUD_DOME_MIN_HEIGHT),
        ("CLOUD_CELLS_PER_UNIT", cpu::CLOUD_CELLS_PER_UNIT),
        ("CLOUD_FBM_EDGE_START", cpu::CLOUD_FBM_EDGE_START),
        ("CLOUD_FBM_EDGE_END", cpu::CLOUD_FBM_EDGE_END),
        ("CLOUD_BAND_START", cpu::CLOUD_BAND_START),
        ("CLOUD_BAND_END", cpu::CLOUD_BAND_END),
        ("CLOUD_TO_WHITE", cpu::CLOUD_TO_WHITE),
    ];
    for (name, rust) in mirrors {
        assert_eq!(
            wgsl_value(name),
            rust,
            "{name} differs between Rust and WGSL"
        );
    }
    let declared = LOOK.lines().filter(|l| l.starts_with("const ")).count();
    assert_eq!(
        declared,
        mirrors.len(),
        "a constant in sky/look.wgsl has no Rust mirror here"
    );
}
