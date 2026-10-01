use std::collections::BTreeMap;

use super::super::recipe::{BlockSel, ParamValue};
use super::*;

fn surface_base() -> String {
    // A minimal stand-in containing the splice point, so assembler unit tests
    // don't read the real asset (the bake/validate tests do that end-to-end).
    format!("fn fs_main() -> vec4<f32> {{\n    var lighting_color = vec3<f32>(1.0);\n    var base_color = vec4<f32>(1.0);\n    {SURFACE_RETURN}\n}}")
}

#[test]
fn surface_folds_blocks_into_the_final_return() {
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: "t".into(),
        blocks: vec![BlockSel {
            id: "toon_ramp".into(),
            params: BTreeMap::new(),
        }],
    };
    let wgsl = assemble(&recipe, &surface_base()).unwrap();
    assert!(wgsl.contains("const toon_ramp_0_steps: f32 = 4.0;"));
    assert!(wgsl.contains("fn srf_toon_ramp"));
    assert!(wgsl.contains("srf_toon_ramp(lighting_color, in, toon_ramp_0_steps)"));
    // The original flat return is gone (rewritten to apply the chain).
    assert!(!wgsl.contains(SURFACE_RETURN));
    // ...and the fog still wraps the folded chain, so authored looks are fogged.
    assert!(wgsl.contains("apply_fog(camera.fog, srf_toon_ramp("));
}

#[test]
fn postfx_emits_fullscreen_entry_points_and_chain() {
    let mut params = BTreeMap::new();
    params.insert(
        "color".to_string(),
        ParamValue::Vector(vec![1.0, 0.5, 0.25]),
    );
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "t".into(),
        blocks: vec![BlockSel {
            id: "tint".into(),
            params,
        }],
    };
    let wgsl = assemble(&recipe, "").unwrap();
    assert!(wgsl.contains("fn vs_fullscreen"));
    assert!(wgsl.contains("fn fs_main"));
    assert!(wgsl.contains("const tint_0_color: vec3<f32> = vec3<f32>(1.0, 0.5, 0.25);"));
    assert!(wgsl.contains("color = pfx_tint(color, uv, tint_0_color);"));
}

#[test]
fn unknown_block_is_rejected() {
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "t".into(),
        blocks: vec![BlockSel {
            id: "nope".into(),
            params: BTreeMap::new(),
        }],
    };
    assert!(assemble(&recipe, "").is_err());
}

#[test]
fn assembly_is_deterministic() {
    let recipe = ShaderRecipe {
        pass: PassKind::Postfx,
        name: "t".into(),
        blocks: vec![
            BlockSel {
                id: "vignette".into(),
                params: BTreeMap::new(),
            },
            BlockSel {
                id: "scanline".into(),
                params: BTreeMap::new(),
            },
        ],
    };
    let a = assemble(&recipe, "").unwrap();
    let b = assemble(&recipe, "").unwrap();
    assert_eq!(a, b);
}
