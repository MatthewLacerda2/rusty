//! A lamp in a dark room: with volumetric fog its air glows, a `Baked` lamp's too
//! (it is in no frame light list, #1703), and with it off the frame is the same
//! bytes whatever the renderer's volume held before.

use super::fixture::{
    draw, fog, glow, lamp, renderer, room, set_fog, settled, view, BRIGHT, LAMP_AT,
};
use rusty_components::{LightMode, VolumetricFog};

/// How much brighter the air around a lamp must read with the fog on than off.
const HALO: f32 = 0.5;

/// The glow around the lamp, fog off and on, for a lamp in `mode`.
fn off_and_on(mode: LightMode) -> Option<(f32, f32)> {
    let mut r = renderer()?;
    let mut scene = room(VolumetricFog::default());
    lamp(&mut scene, LAMP_AT, BRIGHT, mode);
    let off = glow(&settled(&mut r, &scene, 2), LAMP_AT);
    set_fog(&mut scene, fog());
    let on = glow(&settled(&mut r, &scene, 4), LAMP_AT);
    Some((off, on))
}

#[test]
fn gpu_a_lamp_in_a_dark_room_gets_a_halo() {
    let Some((off, on)) = off_and_on(LightMode::Realtime) else {
        return;
    };
    eprintln!("[volumetric] realtime lamp: {off} off, {on} on");
    assert!(on > off + HALO, "the air glows: {on} vs {off}");
}

#[test]
fn gpu_a_baked_lamp_gets_a_halo_too() {
    let Some((off, on)) = off_and_on(LightMode::Baked) else {
        return;
    };
    eprintln!("[volumetric] baked lamp: {off} off, {on} on");
    assert!(on > off + HALO, "the air glows: {on} vs {off}");
}

#[test]
fn gpu_off_is_the_same_frame_whatever_the_volume_held() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(VolumetricFog::default());
    lamp(&mut scene, LAMP_AT, BRIGHT, LightMode::Realtime);
    let before = settled(&mut r, &scene, 2);
    set_fog(&mut scene, fog());
    let mut v = view(&r);
    for _ in 0..3 {
        draw(&mut r, &mut v, &scene);
    }
    set_fog(&mut scene, VolumetricFog::default());
    assert!(r.fog_volume.grid().is_some(), "the volume holds a grid");
    let after = draw(&mut r, &mut v, &scene);
    assert_eq!(before, after, "off draws no fog, byte for byte");
    let held = v.post_fx.targets.get::<super::super::VolumetricTargets>();
    assert!(held.is_none(), "off holds no target");
    for _ in 0..super::super::volume::IDLE_RENDERS {
        draw(&mut r, &mut v, &scene);
    }
    assert_eq!(r.fog_volume.grid(), None, "an idle volume is let go");
}
