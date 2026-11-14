//! A shadowmasked lamp over dynamic objects (#1695): a dynamic caster still casts a
//! realtime shadow, and a dynamic receiver behind baked cover is shaded by the
//! probes' occlusion, since under `Shadowmask` the static cover is in no
//! realtime map.

use glam::{Vec3, Vec4};

use super::fixture::{bake, block, patch, renderer, scene, settled};
use rusty_components::LightMode;
use rusty_scene::lighting::lightmap::{mixed_occlusion, BakeScene};
use rusty_scene::lighting::MixedLightingMode::Shadowmask;

#[test]
fn gpu_a_dynamic_cube_under_a_shadowmask_lamp_casts_a_realtime_shadow() {
    let Some(mut r) = renderer() else { return };
    // On the floor toward the camera, in the cube's shadow from the lamp.
    let shade = Vec3::new(0.0, 0.0, -3.5);
    let mut frame = |cube: bool, name: &str| {
        let (mut s, _) = scene(LightMode::Mixed, Shadowmask, 1.0);
        bake(&mut s, name);
        if cube {
            let at = (Vec3::new(0.0, 0.3, -4.0), Vec3::splat(0.6));
            block(&mut s, "cube", at, 1.0, false);
        }
        patch(&settled(&mut r, &s), shade)
    };
    let (open, shaded) = (frame(false, "no_cube"), frame(true, "cube"));
    eprintln!("[shadowmask] floor by the cube: {open} open, {shaded} shaded");
    assert!(shaded < open * 0.5, "{shaded} vs {open}");
}

#[test]
fn gpu_a_dynamic_cube_behind_baked_cover_reads_the_probes_occlusion() {
    let Some(mut r) = renderer() else { return };
    let (mut s, lamp) = scene(LightMode::Mixed, Shadowmask, 1.0);
    let cover = (Vec3::new(1.0, 1.0, -5.0), Vec3::new(0.2, 2.0, 2.0));
    block(&mut s, "cover", cover, 1.0, true);
    bake(&mut s, "cover");
    assert_eq!(s.lightmaps.mask_channel(lamp), Some(0));
    let at = Vec3::new(2.0, 0.3, -5.0);
    block(&mut s, "cube", (at, Vec3::splat(0.6)), 1.0, false);
    s.probes.add_probe(at);
    // As the probe bake writes it (`ibl::probe::bounce::add_baked_direct`).
    let occlusion = mixed_occlusion(&BakeScene::gather(&s, &|_| None), &[at]);
    assert_eq!(
        occlusion,
        [Vec4::new(0.0, 1.0, 1.0, 1.0)],
        "behind the cover"
    );
    let top = at + Vec3::Y * 0.3;
    let lit = patch(&settled(&mut r, &s), top);
    s.probes.probes_mut()[0].occlusion = occlusion[0];
    let shaded = patch(&settled(&mut r, &s), top);
    eprintln!("[shadowmask] cube top: {lit} unoccluded probe, {shaded} occluded");
    assert!(shaded < lit * 0.25, "{shaded} vs {lit}");
}
