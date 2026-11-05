//! A shadowmasked lamp on static, lightmapped surfaces (#1695).

use glam::Vec3;

use super::fixture::{bake, block, patch, renderer, scene, settled};
use crate::components::LightMode;
use crate::scene::lighting::MixedLightingMode::{BakedIndirect, Shadowmask};

/// A floor point the lamp lights, and one a pillar beside it shades.
const LIT: Vec3 = Vec3::new(-1.0, 0.0, -5.0);
const SHADED: Vec3 = Vec3::new(1.6, 0.0, -5.0);

/// How far a shadowmasked lamp's diffuse on a lightmapped floor may be from the
/// same lamp `Baked` (a 16-sample path trace against a live light with a little
/// rough specular).
const LIKE_BAKED: f32 = 0.2;

fn pillar(scene: &mut crate::scene::Scene) {
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
    // The pillar is static, so under `Shadowmask` it is in no realtime shadow map:
    // the mask alone shades the floor behind it.
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
    // Where the camera sees the lamp mirrored in the floor.
    let glint = Vec3::new(0.0, 0.0, -1.875);
    let mut frame = |roughness, covered: bool, name: &str| {
        let (mut s, _) = scene(LightMode::Mixed, Shadowmask, roughness);
        if covered {
            // Static, between the lamp and the glint, out of the camera's way.
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
        glossy > rough * 2.0,
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

#[test]
fn gpu_static_casters_leave_a_masked_lamps_map_under_shadowmask_only() {
    use crate::core::settings::quality::ShadowmaskMode;
    let Some(mut r) = renderer() else { return };
    let (mut s, _) = scene(LightMode::Mixed, Shadowmask, 1.0);
    pillar(&mut s);
    bake(&mut s, "statics_out");
    // Without its pages a lightmapped floor has the realtime shadow alone.
    s.lightmaps.masks.clear();
    let mut behind = |mode| {
        r.quality.shadowmask_mode = mode;
        let frame = settled(&mut r, &s);
        (patch(&frame, SHADED), patch(&frame, LIT))
    };
    let (plain, lit) = behind(ShadowmaskMode::Shadowmask);
    let (distance, _) = behind(ShadowmaskMode::DistanceShadowmask);
    eprintln!(
        "[shadowmask] unmasked pillar shadow: {plain} Shadowmask, {distance} Distance, {lit} lit"
    );
    assert!(
        plain > lit * 0.5,
        "the static pillar casts no realtime shadow: {plain}"
    );
    assert!(
        distance < lit * 0.25,
        "it casts one under Distance: {distance}"
    );
}
