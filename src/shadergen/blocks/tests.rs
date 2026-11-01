//! The block catalogs' invariants.

use super::*;

#[test]
fn every_block_has_unique_id_within_its_pass() {
    for pass in PassKind::ALL {
        let cat = catalog(pass);
        for (i, b) in cat.iter().enumerate() {
            assert!(
                cat.iter().skip(i + 1).all(|o| o.id != b.id),
                "duplicate block id {} in {}",
                b.id,
                pass.name()
            );
        }
    }
}

#[test]
fn per_lane_defaults_fill_their_arity() {
    for pass in PassKind::ALL {
        for b in catalog(pass) {
            for p in b.params {
                assert_eq!(p.defaults().len(), p.arity, "{}.{}", b.id, p.name);
            }
        }
    }
}

#[test]
fn postfx_strengths_are_runtime() {
    // #671: the amount a script fades (a hurt vignette following health) is live.
    let live = |id, name| {
        find(PassKind::Postfx, id)
            .and_then(|b| b.params.iter().find(|p| p.name == name))
            .is_some_and(|p| p.runtime)
    };
    assert!(live("damage_vignette", "intensity"));
    assert!(live("sharpen", "amount"));
    assert!(live("radial_blur", "strength"));
    assert!(live("barrel", "amount"));
    assert!(live("phosphor_mask", "strength"));
    assert!(!live("posterize", "levels"), "structural params stay baked");
}

#[test]
fn vertex_stage_blocks_are_surface_only_and_ui_has_no_uv_stage() {
    assert!(catalog(PassKind::Ui)
        .iter()
        .all(|b| b.stage == Stage::Color));
    assert!(catalog(PassKind::Postfx)
        .iter()
        .all(|b| b.stage != Stage::Vertex));
    for stage in [Stage::Uv, Stage::Vertex] {
        assert!(catalog(PassKind::Surface).iter().any(|b| b.stage == stage));
    }
    // #1679: barrel warps where the frame is sampled.
    assert_eq!(find(PassKind::Postfx, "barrel").unwrap().stage, Stage::Uv);
}

#[test]
fn choice_params_are_baked_scalars_with_names() {
    for pass in PassKind::ALL {
        for p in catalog(pass).iter().flat_map(|b| b.params) {
            if !p.choices.is_empty() {
                assert!(!p.runtime && p.arity == 1 && p.default == 0.0, "{}", p.name);
            }
        }
    }
}

#[test]
fn only_surface_blocks_cut_and_their_helper_defines_the_cut() {
    for pass in [PassKind::Postfx, PassKind::Ui] {
        assert!(catalog(pass).iter().all(|b| b.cut.is_none()));
    }
    for b in catalog(PassKind::Surface) {
        if let Some(cut) = b.cut {
            let name = cut.split('(').next().unwrap();
            assert!(b.helper.contains(&format!("fn {name}(")), "{}", b.id);
        }
    }
}

#[test]
fn only_lit_surface_blocks_change_the_reflection_and_scale_through_their_helpers() {
    for pass in [PassKind::Postfx, PassKind::Ui] {
        assert!(catalog(pass)
            .iter()
            .all(|b| b.reflection == Reflection::Kept));
    }
    for b in catalog(PassKind::Surface) {
        if let Reflection::Scaled(factor) = b.reflection {
            let name = factor.split('(').next().unwrap();
            assert!(b.helper.contains(&format!("fn {name}(")), "{}", b.id);
        }
    }
}

#[test]
fn block_textures_name_real_slots_and_only_surface_blocks_have_any() {
    for b in catalog(PassKind::Surface) {
        for t in b.textures {
            assert!(crate::shadergen::textures::slot(t).is_ok(), "{}: {t}", b.id);
        }
    }
    for pass in [PassKind::Postfx, PassKind::Ui] {
        assert!(catalog(pass).iter().all(|b| b.textures.is_empty()));
    }
}

#[test]
fn find_resolves_known_blocks_and_rejects_unknown() {
    assert!(find(PassKind::Surface, "toon_ramp").is_some());
    assert!(find(PassKind::Postfx, "scanline").is_some());
    assert!(find(PassKind::Surface, "scanline").is_none());
    // #1439: the volume owns these now, so a recipe naming one is refused.
    for owned in ["vignette", "film_grain", "chromatic_aberration"] {
        assert!(find(PassKind::Postfx, owned).is_none(), "{owned}");
    }
    assert!(find(PassKind::Postfx, "definitely_not_a_block").is_none());
    assert!(find(PassKind::Ui, "glitch_slices").is_some());
    assert!(find(PassKind::Postfx, "glitch_slices").is_none());
}
