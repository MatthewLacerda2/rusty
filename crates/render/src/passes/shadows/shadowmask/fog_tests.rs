//! Volumetric fog over shadowmasked lights (#1695). The fog's air is lit through
//! the realtime maps alone, never a mask, so while it is on the maps keep their
//! static casters: a sealed room's air stays as dark as on `BakedIndirect`, and a
//! masked sun's cascades re-bake with and without them as the fog toggles.

use glam::{Quat, Vec3};

use super::fixture::{bake, block, patch, renderer, scene, settled};
use super::statics_tests::{set_fog, whiten};
use rusty_components::{LightMode, LightType};
use rusty_core::settings::quality::ShadowmaskMode;
use rusty_scene::lighting::MixedLightingMode::{self, BakedIndirect, Shadowmask};
use rusty_scene::Scene;

/// How much brighter (linear, summed RGB / 3) the masked room's air may read than
/// the `BakedIndirect` one's: a sealed room holds no light either way.
const SEALED: f32 = 0.01;

/// A directional `Mixed` sun added to `s`, low from -X.
fn sun(s: &mut Scene) -> u32 {
    let id = s.add_entity("Sun".to_string());
    let light = rusty_components::LightComponent {
        light_type: LightType::Directional,
        mode: LightMode::Mixed,
        intensity: 3.0,
        ..rusty_scene::authoring::default_light()
    };
    s.world.set_light(id, Some(light));
    s.world.transform_mut(id).unwrap().rotation =
        Quat::from_rotation_z(-0.6) * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    id
}

/// The camera sealed in a static box (floor, ceiling and four walls), the fixture's
/// lamp just outside its back wall and a sun over it, both `Mixed`, baked on
/// `mixed`, with fog on.
fn sealed(mixed: MixedLightingMode, name: &str) -> Scene {
    let (mut s, lamp) = scene(LightMode::Mixed, mixed, 1.0);
    let sun = sun(&mut s);
    let walls = [
        (Vec3::new(0.0, 3.05, -1.0), Vec3::new(6.4, 0.1, 6.4)),
        (Vec3::new(0.0, 1.5, -4.1), Vec3::new(6.4, 3.2, 0.2)),
        (Vec3::new(0.0, 1.5, 2.1), Vec3::new(6.4, 3.2, 0.2)),
        (Vec3::new(-3.1, 1.5, -1.0), Vec3::new(0.2, 3.2, 6.4)),
        (Vec3::new(3.1, 1.5, -1.0), Vec3::new(0.2, 3.2, 6.4)),
    ];
    for (k, at) in walls.into_iter().enumerate() {
        block(&mut s, &format!("wall{k}"), at, 1.0, true);
    }
    bake(&mut s, name);
    if mixed == Shadowmask {
        assert!(s.mask_channel(sun).is_some() && s.mask_channel(lamp).is_some());
    }
    set_fog(&mut s, true);
    s
}

/// The mean of the frame (summed RGB / 3, linear), air and walls.
fn mean(frame: &[u8]) -> f32 {
    let lin = |b: &u8| rusty_core::color::srgb_to_linear(*b);
    let sum: f32 = frame.chunks(4).flat_map(|p| &p[..3]).map(lin).sum();
    sum / (frame.len() / 4 * 3) as f32
}

#[test]
fn gpu_a_sealed_rooms_air_stays_dark_under_shadowmask_with_fog() {
    let Some(mut r) = renderer() else { return };
    r.quality.shadowmask_mode = ShadowmaskMode::Shadowmask;
    let masked = mean(&settled(&mut r, &sealed(Shadowmask, "sealed_masked")));
    let plain = mean(&settled(&mut r, &sealed(BakedIndirect, "sealed_plain")));
    eprintln!("[shadowmask] sealed room under fog: {masked} Shadowmask, {plain} BakedIndirect");
    assert!(
        masked <= plain + SEALED,
        "no light through the walls: {masked} vs {plain}"
    );
}

#[test]
fn gpu_toggling_fog_rebakes_a_masked_suns_cascades() {
    let Some(mut r) = renderer() else { return };
    r.quality.shadowmask_mode = ShadowmaskMode::Shadowmask;
    let (mut s, lamp) = scene(LightMode::Mixed, Shadowmask, 1.0);
    s.world.set_light(lamp, None);
    let sun = sun(&mut s);
    let wall = (Vec3::new(-1.5, 1.0, -6.0), Vec3::new(0.2, 2.0, 14.0));
    block(&mut s, "wall", wall, 1.0, true);
    bake(&mut s, "sun_fog");
    assert!(s.mask_channel(sun).is_some());
    whiten(&s);
    let mut share = |s: &Scene| {
        let frame = settled(&mut r, s);
        patch(&frame, Vec3::new(-0.5, 0.0, -4.0)) / patch(&frame, Vec3::new(2.5, 0.0, -6.0))
    };
    let clear = share(&s);
    set_fog(&mut s, true);
    let fogged = share(&s);
    set_fog(&mut s, false);
    let again = share(&s);
    eprintln!("[shadowmask] sun by the wall: {clear} clear, {fogged} fogged, {again} again");
    assert!(clear > 0.5 && again > 0.5, "statics out: {clear}, {again}");
    assert!(fogged < 0.3, "the wall in the cascades: {fogged}");
}
