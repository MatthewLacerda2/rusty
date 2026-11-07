//! TAA on lavapipe (#1702): off means off, FXAA skips when it runs, a still view
//! writes no motion through the jitter, a still frame converges toward a
//! supersampled reference, and a NaN in the history does not stay.

use glam::Vec3;

use super::super::jitter_of;
use super::fixture::{big_view, downsample, errors, frame, holds, renderer, settle};
use super::fixture::{taa_scene, tilted_cube, view};
use crate::render::motion::gpu_fixture::{eye, render, H, W};
use crate::render::readback;

#[test]
fn gpu_off_means_off_no_history_no_velocity_no_jitter() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, _) = taa_scene(false, true);
    tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
    let mut v = view(&r);
    let first = frame(&mut r, &mut v, &scene, &eye(0.0));
    let second = frame(&mut r, &mut v, &scene, &eye(0.0));
    assert!(!holds(&v), "no history target");
    assert!(
        v.post_fx.motion.active_target().is_none(),
        "no velocity target"
    );
    assert_eq!(jitter_of(&v), glam::Vec2::ZERO, "no jitter");
    assert!(first == second, "no frame-to-frame change");
}

/// One anti-aliasing: on a frame the resolve ran, FXAA's switch changes nothing.
#[test]
fn gpu_fxaa_skips_on_a_frame_taa_resolved() {
    let Some(mut r) = renderer() else { return };
    let mut run = |fxaa| {
        let (mut scene, _) = taa_scene(true, fxaa);
        tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
        let mut v = view(&r);
        let out = settle(&mut r, &mut v, &scene, &eye(0.0), 3);
        assert!(v.post_fx.taa_ran(), "the resolve ran");
        out
    };
    assert!(run(true) == run(false), "FXAA ran over TAA's frame");
}

#[test]
fn gpu_a_still_camera_on_a_still_scene_writes_no_motion_through_the_jitter() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, _) = taa_scene(true, true);
    tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
    let mut v = view(&r);
    for n in 0..4 {
        let velocity = render(&mut r, &mut v, &scene, &eye(0.0)).expect("TAA asks for velocity");
        assert!(
            holds(&v) && jitter_of(&v) != glam::Vec2::ZERO,
            "TAA runs, jittered"
        );
        let worst = (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .map(|(x, y)| velocity.at(x, y).abs().max_element())
            .fold(0.0f32, f32::max);
        // One jitter step is ≥ 1/W of NDC; a half float holds 0 exactly.
        assert!(
            worst < 1e-4,
            "frame {n}: a still pixel moved {worst} in NDC"
        );
    }
}

/// Supersampling factor of the reference, and frames TAA gets to converge.
const SUPER: u32 = 8;
const FRAMES: u32 = 48;

#[test]
fn gpu_a_still_frame_converges_toward_the_supersampled_reference() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let at = Vec3::new(0.0, 0.0, -4.0);
    let (mut plain, _) = taa_scene(false, false);
    tilted_cube(&mut plain, at);
    let mut big = big_view(&r, SUPER);
    let reference = downsample(&frame(&mut r, &mut big, &plain, &cam), SUPER);
    let mut v = view(&r);
    let aliased = errors(&frame(&mut r, &mut v, &plain, &cam), &reference);
    let (mut fxaa_scene, _) = taa_scene(false, true);
    tilted_cube(&mut fxaa_scene, at);
    let mut v = view(&r);
    let fxaa = errors(&frame(&mut r, &mut v, &fxaa_scene, &cam), &reference);
    let (mut scene, _) = taa_scene(true, true);
    tilted_cube(&mut scene, at);
    let mut v = view(&r);
    let taa = errors(&settle(&mut r, &mut v, &scene, &cam, FRAMES), &reference);
    // The edge pixels: where the aliased frame is off the reference.
    let edges: Vec<usize> = (0..aliased.len()).filter(|&i| aliased[i] > 16.0).collect();
    let mean = |e: &[f32]| edges.iter().map(|&i| e[i]).sum::<f32>() / edges.len() as f32;
    let (a, f, t) = (mean(&aliased), mean(&fxaa), mean(&taa));
    let worst_flat = (0..taa.len())
        .filter(|i| !edges.contains(i))
        .map(|i| taa[i])
        .fold(0.0, f32::max);
    eprintln!("[taa] {} edge pixels off the reference by: aliased {a:.1}, FXAA {f:.1}, TAA {t:.1}; flat worst {worst_flat:.1}", edges.len());
    assert!(edges.len() > 20, "the cube's edges cross the frame");
    assert!(
        t < 0.5 * a,
        "TAA's edges sit within half the aliased error: {t:.1} vs {a:.1}"
    );
    assert!(
        t < f,
        "TAA's edges sit nearer the reference than FXAA's: {t:.1} vs {f:.1}"
    );
    assert!(
        worst_flat <= 24.0,
        "a flat pixel drifted {worst_flat:.1} off the reference"
    );
}

#[test]
fn gpu_a_nan_in_the_history_does_not_stay() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, _) = taa_scene(true, true);
    tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -5.0));
    let mut v = view(&r);
    let cam = eye(0.0);
    let settled = settle(&mut r, &mut v, &scene, &cam, 8);
    // Every texel of the history NaN (half float 0x7e00), as if one bad frame wrote it.
    let latest = v.post_fx.taa_latest().expect("TAA ran");
    let nan = 0x7e00u16.to_le_bytes().repeat((W * H * 4) as usize);
    let layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(W * 8),
        rows_per_image: Some(H),
    };
    r.queue
        .write_texture(latest.as_image_copy(), &nan, layout, latest.size());
    let after = frame(&mut r, &mut v, &scene, &cam);
    let history = v.post_fx.taa_latest().unwrap();
    let bytes = readback::read_texture_bytes(&r.device, &r.queue, history, (W, H), 8);
    let halves = bytes.chunks(2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let bad = halves.filter(|h| h & 0x7c00 == 0x7c00).count();
    assert_eq!(bad, 0, "{bad} history channels not finite a frame later");
    let off = (0..W * H).map(|i| super::fixture::diff(&after, &settled, (i % W, i / W)));
    let worst = off.max().unwrap();
    eprintln!("[taa] a frame after a NaN history, the image is {worst} off the settled one");
    assert!(
        worst <= 96,
        "the frame after the NaN is the current frame, not garbage"
    );
}
