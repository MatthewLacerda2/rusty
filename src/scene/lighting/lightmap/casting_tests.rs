//! *Cast Shadows* in the bake (#1571), Unity's rule: an `Off` mesh blocks no light
//! but is still seen (it bounces and emits); a `ShadowsOnly` one blocks light but is
//! seen by nothing and gets no lightmap. The probe bake's shadow rays agree.

use glam::Vec3;

use super::tests::{floor, mean, settings};
use super::*;
use crate::components::ShadowCasting;
use crate::scene::lighting::sh::Sh9;

/// A 4×4 panel 1 unit over the floor, facing down onto it, glowing `emissive`.
fn panel(mode: ShadowCasting, emissive: f32) -> BakeMesh {
    let mut panel = floor();
    panel.entity = 3;
    panel.positions = panel.positions.iter().map(|p| *p + Vec3::Y).rev().collect();
    panel.normals = vec![Vec3::NEG_Y; 4];
    panel.runs[0].emissive = Vec3::splat(emissive);
    panel.cast_shadows = mode;
    panel
}

/// The floor under `panel`, unlit but for what `lights` add.
fn under(panel: BakeMesh, lights: Vec<BakeLight>) -> BakeScene {
    BakeScene {
        meshes: vec![floor(), panel],
        lights,
        sky: Vec3::ZERO.into(),
    }
}

/// A `Baked` sun shining straight down.
fn noon() -> BakeLight {
    BakeLight {
        shape: LightShape::Directional {
            direction: Vec3::NEG_Y,
        },
        radiance: Vec3::splat(3.0),
        bakes_direct: true,
    }
}

#[test]
fn an_off_emissive_panel_still_lights_the_floor_and_gets_its_lightmap() {
    let maps = bake(&under(panel(ShadowCasting::Off, 2.0), vec![]), &settings());
    let floor = mean(&maps, 1);
    assert!(floor > 0.5, "an Off panel still emits: {floor}");
    assert!(
        maps.iter().any(|m| m.entity == 3),
        "an Off panel is lightmapped"
    );
}

#[test]
fn a_shadows_only_panel_emits_nothing_and_gets_no_lightmap() {
    let maps = bake(
        &under(panel(ShadowCasting::ShadowsOnly, 2.0), vec![]),
        &settings(),
    );
    assert_eq!(mean(&maps, 1), 0.0, "nothing sees a ShadowsOnly panel");
    assert!(maps.iter().all(|m| m.entity != 3), "no lightmap texels");
}

#[test]
fn only_a_panel_that_casts_blocks_the_sun() {
    let floor = |mode| {
        mean(
            &bake(&under(panel(mode, 0.0), vec![noon()]), &settings()),
            1,
        )
    };
    let direct = 3.0 / std::f32::consts::PI;
    let off = floor(ShadowCasting::Off);
    // At least the sun's direct light, plus what the panel still bounces back.
    assert!(off > direct - 0.05, "the sun passes an Off panel: {off}");
    for mode in [ShadowCasting::On, ShadowCasting::ShadowsOnly] {
        let lit = floor(mode);
        assert!(lit < 0.1, "{mode:?} shades the floor: {lit}");
    }
}

#[test]
fn the_probe_bake_shadows_by_the_same_rule() {
    let probe = |mode| baked_direct_sh(&under(panel(mode, 0.0), vec![noon()]), &[Vec3::Y * 0.5]);
    assert_ne!(probe(ShadowCasting::Off), vec![Sh9::zero()]);
    assert_eq!(probe(ShadowCasting::ShadowsOnly), vec![Sh9::zero()]);
}
