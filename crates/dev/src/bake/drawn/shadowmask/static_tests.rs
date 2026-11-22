use glam::Vec3;

use super::fixture::{bake, block, patch, renderer, scene, settled};
use rusty_components::LightMode;
use rusty_scene::lighting::MixedLightingMode::{BakedIndirect, Shadowmask};

pub(super) const LIT: Vec3 = Vec3::new(-1.0, 0.0, -5.0);
pub(super) const SHADED: Vec3 = Vec3::new(1.6, 0.0, -5.0);

/// The share the masked lamp's lit floor may stray from the `Baked` one's: a 16-sample trace
/// against a live light. Measured on lavapipe: 0.03.
const LIKE_BAKED: f32 = 0.2;

/// How many times brighter than a rough floor a glossy one reads where the lamp mirrors.
/// Measured on lavapipe: 2.06.
const GLINT: f32 = 1.5;

pub(super) fn pillar(scene: &mut rusty_scene::Scene) {
    let at = (Vec3::new(0.8, 1.0, -5.0), Vec3::new(0.3, 2.0, 0.3));
    block(scene, "pillar", at, 1.0, true);
}

#[test]
fn gpu_a_shadowmask_lamp_lights_a_static_floor_as_a_baked_one_and_its_shadow_holds() {
    let Some(mut r) = renderer() else { return };
    let mut frame = |mode, name: &str| {
        let (mut s, _) = scene(mode, Shadowmask, 1.0);
        pillar(&mut s);
        bake(&mut s, name);
        settled(&mut r, &s)
    };
    let baked = frame(LightMode::Baked, "as_baked");
    let masked = frame(LightMode::Mixed, "as_mixed");
    let (b, m) = (patch(&baked, LIT), patch(&masked, LIT));
    eprintln!("[shadowmask] lit floor: {b} baked, {m} masked");
    assert!(b > 0.05 && (m - b).abs() <= b * LIKE_BAKED, "{m} vs {b}");
    // The static pillar is in no realtime map under `Shadowmask`: the mask alone shades.
    let shaded = patch(&masked, SHADED);
    eprintln!(
        "[shadowmask] behind the pillar: {shaded} masked, {} baked",
        patch(&baked, SHADED)
    );
    assert!(shaded < m * 0.25, "{shaded} vs {m}");
}

#[test]
fn gpu_a_shadowmask_lamp_glints_on_a_glossy_floor_and_its_baked_cover_blocks_the_glint() {
    let Some(mut r) = renderer() else { return };
    // Where the eye's line to the lamp's mirror image meets the floor: z = -5 × 1.5 / 4.
    let glint = Vec3::new(0.0, 0.0, -1.875);
    let mut frame = |roughness, covered: bool, name: &str| {
        let (mut s, _) = scene(LightMode::Mixed, Shadowmask, roughness);
        if covered {
            // On the lamp-to-glint line, beyond the glint, so the camera never sees it.
            let at = (Vec3::new(0.0, 1.06, -3.2), Vec3::new(1.5, 0.2, 0.6));
            block(&mut s, "cover", at, 1.0, true);
        }
        bake(&mut s, name);
        patch(&settled(&mut r, &s), glint)
    };
    let (glossy, rough) = (frame(0.2, false, "glossy"), frame(1.0, false, "rough"));
    let covered = frame(0.2, true, "covered");
    eprintln!("[shadowmask] glint: {glossy} glossy, {rough} rough, {covered} covered");
    assert!(
        glossy > rough * GLINT,
        "a live highlight: {glossy} vs {rough}"
    );
    assert!(
        covered < rough * 0.5,
        "the mask blocks it: {covered} vs {rough}"
    );
}

#[test]
fn gpu_baked_indirect_draws_what_a_baked_indirect_bake_draws_whatever_the_mask() {
    let Some(mut r) = renderer() else { return };
    let (mut was_masked, _) = scene(LightMode::Mixed, Shadowmask, 0.3);
    pillar(&mut was_masked);
    bake(&mut was_masked, "was_masked");
    assert!(!was_masked.lightmaps.masks.is_empty());
    was_masked.lighting_settings.mixed_lighting_mode = BakedIndirect;
    let (mut plain, _) = scene(LightMode::Mixed, BakedIndirect, 0.3);
    pillar(&mut plain);
    bake(&mut plain, "plain");
    assert!(plain.lightmaps.masks.is_empty());
    let (a, b) = (settled(&mut r, &was_masked), settled(&mut r, &plain));
    assert!(a == b, "off means off");
}
