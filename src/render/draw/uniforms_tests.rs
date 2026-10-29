//! The transparent-surface terms a material packs into its draw's uniform (#1523),
//! without a GPU: what the shader branches on and the blend picks.

use super::surface_uniform;
use crate::components::{BlendMode, MaterialAsset, RefractionModel, RenderMode};

fn glass() -> MaterialAsset {
    MaterialAsset {
        render_mode: RenderMode::Transparent,
        ..MaterialAsset::default()
    }
}

#[test]
fn an_opaque_default_receives_shadows_and_nothing_else() {
    let u = surface_uniform(true, Some(&MaterialAsset::default()));
    assert_eq!(
        (u.receive_shadows, u.premultiply_diffuse, u.uses_opaque),
        (1, 0, 0)
    );
    assert_eq!(surface_uniform(true, None).receive_shadows, 1);
    let postpass = MaterialAsset {
        depth_postpass: true,
        ..MaterialAsset::default()
    };
    assert_eq!(
        surface_uniform(true, Some(&postpass)).depth_postpass,
        0,
        "glass only"
    );
}

#[test]
fn refraction_reads_the_copy_under_alpha_and_premultiply_only() {
    let planar = MaterialAsset {
        refraction_model: RefractionModel::Planar,
        thickness: 0.7,
        ..glass()
    };
    let u = surface_uniform(true, Some(&planar));
    assert_eq!(
        (u.uses_opaque, u.refraction_model, u.thickness),
        (1, 1, 0.7)
    );
    let thin = MaterialAsset {
        refraction_model: RefractionModel::Thin,
        ..planar.clone()
    };
    assert_eq!(
        surface_uniform(true, Some(&thin)).thickness,
        0.005,
        "HDRP's 5 mm"
    );
    let additive = MaterialAsset {
        blend_mode: BlendMode::Additive,
        ..planar
    };
    let u = surface_uniform(true, Some(&additive));
    assert_eq!((u.uses_opaque, u.refraction_model), (0, 0));
}

#[test]
fn absorption_is_hdrps_coefficient_and_white_absorbs_nothing() {
    assert_eq!(
        surface_uniform(true, Some(&glass())).absorption,
        [0.0, 0.0, 0.0, 1.0],
        "w: the default thickness, the sky's absorption depth"
    );
    let murky = MaterialAsset {
        transmittance_color: [(-1.0f32).exp(), 1.0, 1.0],
        absorption_distance: 2.0,
        refraction_model: RefractionModel::Thin,
        thickness: 3.0,
        ..glass()
    };
    let u = surface_uniform(true, Some(&murky));
    assert_eq!(u.uses_opaque, 1, "absorbing reads the copy");
    assert!((u.absorption[0] - 0.5).abs() < 1e-3, "{:?}", u.absorption);
    assert_eq!(
        u.absorption[3], 3.0,
        "the sky depth is the thickness, Thin or not"
    );
}

#[test]
fn transmission_packs_colour_times_amount() {
    let leaf = MaterialAsset {
        transmission_color: [0.5, 1.0, 0.0],
        transmission: 0.5,
        ..MaterialAsset::default()
    };
    assert_eq!(
        surface_uniform(true, Some(&leaf)).transmission,
        [0.25, 0.5, 0.0, 0.5]
    );
}

#[test]
fn an_unlit_draw_blends_straight_and_skips_the_copy() {
    let murky = MaterialAsset {
        transmittance_color: [0.5, 0.5, 0.5],
        ..glass()
    };
    let u = surface_uniform(false, Some(&murky));
    assert_eq!((u.premultiply_diffuse, u.uses_opaque), (0, 0));
}

#[test]
fn only_a_transparent_material_packs_its_blend_mode() {
    let multiply = |render_mode| MaterialAsset {
        render_mode,
        blend_mode: BlendMode::Multiply,
        ..MaterialAsset::default()
    };
    let packed = |mode| surface_uniform(true, Some(&multiply(mode))).blend_mode;
    assert_eq!(packed(RenderMode::Opaque), 0);
    assert_eq!(packed(RenderMode::Cutout), 0);
    assert_eq!(packed(RenderMode::Transparent), 3);
}
