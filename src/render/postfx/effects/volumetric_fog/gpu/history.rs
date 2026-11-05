//! The froxels' history (#1703): a light that moved leaves under 5 % of its old
//! glow after the ghost frames, a cut starts over at once, a resize holds, and
//! settings past their ranges (a file that skipped the ops) draw what their
//! clamped values draw.

use glam::Vec3;

use super::super::GHOST_FRAMES;
use super::fixture::{draw, fog, glow, lamp, renderer, room, settled, view, BRIGHT, LAMP_AT};
use crate::components::{LightMode, VolumetricFog};

/// Where the lamp moves to: across the room, out of the old halo's reach.
const MOVED: Vec3 = Vec3::new(3.0, 1.5, -4.0);

/// How much of the old glow may stay, with room for the jitter's noise.
const LEFT: f32 = 0.05;
const NOISE: f32 = 0.05;

/// The glow at the lamp's old place: settled there, then `frames` frames after it
/// moved, `cut` renewing the scene's identity when it moves. Also the settled glow
/// there with the lamp at its new place alone.
fn trail(frames: u32, cut: bool) -> Option<(f32, f32, f32)> {
    let mut r = renderer()?;
    let mut scene = room(fog());
    let id = lamp(&mut scene, LAMP_AT, BRIGHT, LightMode::Realtime);
    let mut v = view(&r);
    let mut frame = Vec::new();
    for _ in 0..8 {
        frame = draw(&mut r, &mut v, &scene);
    }
    let before = glow(&frame, LAMP_AT);
    scene.world.transform_mut(id).unwrap().position = MOVED;
    if cut {
        scene.renew_id();
    }
    for _ in 0..frames {
        frame = draw(&mut r, &mut v, &scene);
    }
    let fresh = glow(&settled(&mut r, &scene, 8), LAMP_AT);
    Some((before, glow(&frame, LAMP_AT), fresh))
}

#[test]
fn gpu_a_moved_light_leaves_no_trail_past_the_ghost_frames() {
    let Some((before, after, fresh)) = trail(GHOST_FRAMES, false) else {
        return;
    };
    eprintln!("[volumetric] trail: {before} before, {after} after, {fresh} fresh");
    assert!(before > fresh + 0.5, "it glowed there");
    let left = (after - fresh) / (before - fresh);
    assert!(left <= LEFT + NOISE, "{left} of the glow stays");
}

#[test]
fn gpu_a_cut_starts_over_at_once() {
    let Some((before, after, fresh)) = trail(1, true) else {
        return;
    };
    eprintln!("[volumetric] cut: {before} before, {after} after, {fresh} fresh");
    let left = (after - fresh) / (before - fresh);
    assert!(left <= NOISE, "{left} of the glow stays past a cut");
}

#[test]
fn gpu_a_resize_holds() {
    let Some(mut r) = renderer() else { return };
    let mut scene = room(fog());
    lamp(&mut scene, LAMP_AT, BRIGHT, LightMode::Realtime);
    let mut v = view(&r);
    draw(&mut r, &mut v, &scene);
    v.resize(&r.device, 64, 48);
    let frame = draw(&mut r, &mut v, &scene);
    assert_eq!(frame.len(), 64 * 48 * 4);
    assert!(frame.chunks_exact(4).any(|p| p[0] > 0), "it still glows");
}

#[test]
fn gpu_settings_out_of_range_draw_their_clamped_values() {
    let Some(mut r) = renderer() else { return };
    let wild = VolumetricFog {
        mean_free_path: f32::NAN,
        anisotropy: 1e30,
        depth_extent: f32::INFINITY,
        global_light_probe_dimmer: f32::NAN,
        albedo: [f32::NAN, 2.0, -1.0],
        ..fog()
    };
    let clamped = VolumetricFog {
        mean_free_path: VolumetricFog::default().mean_free_path,
        anisotropy: VolumetricFog::MAX_ANISOTROPY,
        depth_extent: VolumetricFog::default().depth_extent,
        global_light_probe_dimmer: 1.0,
        albedo: [1.0, 1.0, 0.0],
        ..fog()
    };
    let frame = |fog| {
        let mut scene = room(fog);
        lamp(&mut scene, LAMP_AT, BRIGHT, LightMode::Realtime);
        scene
    };
    let wild = settled(&mut r, &frame(wild), 2);
    let clamped = settled(&mut r, &frame(clamped), 2);
    assert_eq!(wild, clamped);
}
