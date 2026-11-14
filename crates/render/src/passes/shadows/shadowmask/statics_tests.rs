//! Which casters a masked light's realtime maps hold (#1695). Under plain
//! `Shadowmask` its static casters leave them, but only while its mask pages are
//! bound and no volumetric fog is on, whose air reads only the maps; under
//! `DistanceShadowmask` they always stay. A mask page whitened after the bake
//! (lit everywhere) shows what the maps alone draw.

use super::fixture::{bake, patch, renderer, scene, settled};
use super::static_tests::{pillar, LIT, SHADED};
use crate::Renderer;
use rusty_components::{LightMode, VolumetricFog};
use rusty_core::settings::quality::ShadowmaskMode;
use rusty_scene::lighting::MixedLightingMode::Shadowmask;
use rusty_scene::Scene;

/// Every mask page lit at every texel: what's dark is the realtime maps' doing.
pub(super) fn whiten(scene: &Scene) {
    for path in &scene.lightmaps.masks {
        let (w, h) = image::image_dimensions(path).expect("a mask page");
        let white = image::RgbaImage::from_pixel(w, h, image::Rgba([255; 4]));
        white.save(path).expect("rewritten");
    }
}

/// A thin volumetric fog on the camera's volume, or none.
pub(super) fn set_fog(scene: &mut Scene, on: bool) {
    let fog = VolumetricFog {
        active: on,
        mean_free_path: 60.0,
        ..VolumetricFog::default()
    };
    for id in scene.world.ids_with_post_processing() {
        scene.world.post_processing_mut(id).unwrap().volumetric_fog = fog;
    }
}

/// The floor behind the static pillar against the lit floor, as a share.
fn shade(r: &mut Renderer, s: &Scene, mode: ShadowmaskMode) -> f32 {
    r.quality.shadowmask_mode = mode;
    let frame = settled(r, s);
    let (behind, lit) = (patch(&frame, SHADED), patch(&frame, LIT));
    eprintln!("[shadowmask] {mode:?}: behind the pillar {behind}, lit {lit}");
    behind / lit
}

/// The pillar scene, baked; its masks whitened when `white`.
fn pillar_scene(name: &str, white: bool) -> Scene {
    let (mut s, _) = scene(LightMode::Mixed, Shadowmask, 1.0);
    pillar(&mut s);
    bake(&mut s, name);
    if white {
        whiten(&s);
    }
    s
}

#[test]
fn gpu_static_casters_leave_a_masked_lamps_map_under_shadowmask_only() {
    let Some(mut r) = renderer() else { return };
    let s = pillar_scene("statics_out", true);
    let plain = shade(&mut r, &s, ShadowmaskMode::Shadowmask);
    let distance = shade(&mut r, &s, ShadowmaskMode::DistanceShadowmask);
    assert!(
        plain > 0.5,
        "the static pillar casts no realtime shadow: {plain}"
    );
    assert!(distance < 0.25, "it casts one under Distance: {distance}");
}

#[test]
fn gpu_with_no_mask_bound_a_static_pillar_still_shadows_the_floor() {
    let Some(mut r) = renderer() else { return };
    let mut s = pillar_scene("unbound", false);
    // Channels recorded, no page bound: as while a page decodes, or failed to.
    s.lightmaps.masks.clear();
    assert!(!s.lightmaps.channels.is_empty());
    let plain = shade(&mut r, &s, ShadowmaskMode::Shadowmask);
    assert!(plain < 0.25, "the maps keep their statics: {plain}");
}

#[test]
fn gpu_volumetric_fog_keeps_static_casters_in_the_maps_and_toggling_it_rebakes() {
    let Some(mut r) = renderer() else { return };
    let mut s = pillar_scene("fogged", true);
    let mut at = |s: &Scene| shade(&mut r, s, ShadowmaskMode::Shadowmask);
    let clear = at(&s);
    set_fog(&mut s, true);
    let fogged = at(&s);
    set_fog(&mut s, false);
    let again = at(&s);
    assert!(clear > 0.5, "no fog: statics out ({clear})");
    assert!(fogged < 0.25, "fog: the pillar is in the map ({fogged})");
    assert!(again > 0.5, "fog off: re-baked without it ({again})");
}
