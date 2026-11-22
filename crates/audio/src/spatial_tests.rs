use super::{Listener, SpatialResult};
use crate::{spatial, AudioMaestro};
use glam::{Quat, Vec3};
use rusty_components::AudioSourceComponent;

fn at_origin() -> Listener {
    Listener::from_transform(Vec3::ZERO, Quat::IDENTITY)
}

fn resolve_3d(pos: Vec3, vol: f32, initial: f32, final_d: f32) -> SpatialResult {
    spatial::resolve(&at_origin(), pos, vol, 1.0, initial, final_d)
}

#[test]
fn rolloff_is_flat_within_initial_distance() {
    let near = resolve_3d(Vec3::new(0.0, 0.0, 0.5), 1.0, 1.0, 5.0);
    let at_initial = resolve_3d(Vec3::new(0.0, 0.0, 1.0), 1.0, 1.0, 5.0);
    assert!((near.gain - 1.0).abs() < 1e-6, "got {near:?}");
    assert!((at_initial.gain - 1.0).abs() < 1e-6, "got {at_initial:?}");
}

#[test]
fn rolloff_is_linear_between_initial_and_final() {
    // Band 1→5 (width 4): at distance 3 we are halfway, so gain == 0.5 * volume.
    let mid = resolve_3d(Vec3::new(0.0, 0.0, 3.0), 1.0, 1.0, 5.0);
    assert!((mid.gain - 0.5).abs() < 1e-6, "got {mid:?}");
    // A quarter of the way (distance 2) gives 0.75.
    let quarter = resolve_3d(Vec3::new(0.0, 0.0, 2.0), 1.0, 1.0, 5.0);
    assert!((quarter.gain - 0.75).abs() < 1e-6, "got {quarter:?}");
}

#[test]
fn rolloff_is_silent_at_and_beyond_final_distance() {
    let at_final = resolve_3d(Vec3::new(0.0, 0.0, 5.0), 1.0, 1.0, 5.0);
    let beyond = resolve_3d(Vec3::new(0.0, 0.0, 50.0), 1.0, 1.0, 5.0);
    assert_eq!(at_final.gain, 0.0);
    assert_eq!(beyond.gain, 0.0);
}

#[test]
fn rolloff_scales_with_source_volume() {
    // Halfway through the band at half volume → 0.25.
    let r = resolve_3d(Vec3::new(0.0, 0.0, 3.0), 0.5, 1.0, 5.0);
    assert!((r.gain - 0.25).abs() < 1e-6, "got {r:?}");
}

#[test]
fn source_on_the_right_pans_right() {
    let r = resolve_3d(Vec3::new(3.0, 0.0, 0.0), 1.0, 100.0, 200.0);
    assert!((r.pan - 1.0).abs() < 1e-6, "got {r:?}");
}

#[test]
fn source_on_the_left_pans_left() {
    let r = resolve_3d(Vec3::new(-3.0, 0.0, 0.0), 1.0, 100.0, 200.0);
    assert!((r.pan + 1.0).abs() < 1e-6, "got {r:?}");
}

#[test]
fn source_dead_ahead_is_centred() {
    let r = resolve_3d(Vec3::new(0.0, 0.0, -3.0), 1.0, 100.0, 200.0);
    assert!(r.pan.abs() < 1e-6, "got {r:?}");
}

#[test]
fn co_located_source_is_centred() {
    // No direction when source sits on the listener — pan must not NaN.
    let r = resolve_3d(Vec3::ZERO, 1.0, 1.0, 5.0);
    assert_eq!(r.pan, 0.0);
}

#[test]
fn listener_rotation_rotates_the_pan_axis() {
    // Turned 180° about Y, its right is -X: a source at +X sits on its left.
    let listener =
        Listener::from_transform(Vec3::ZERO, Quat::from_rotation_y(std::f32::consts::PI));
    let r = spatial::resolve(&listener, Vec3::new(3.0, 0.0, 0.0), 1.0, 1.0, 100.0, 200.0);
    assert!((r.pan + 1.0).abs() < 1e-5, "got {r:?}");
}

#[test]
fn spatial_blend_lerps_2d_to_3d() {
    let pos = Vec3::new(3.0, 0.0, 0.0); // distance 3 in a 1→5 band → rolloff 0.5
    let two_d = spatial::resolve(&at_origin(), pos, 1.0, 0.0, 1.0, 5.0);
    let half = spatial::resolve(&at_origin(), pos, 1.0, 0.5, 1.0, 5.0);
    let three_d = spatial::resolve(&at_origin(), pos, 1.0, 1.0, 1.0, 5.0);

    assert!((two_d.gain - 1.0).abs() < 1e-6, "got {two_d:?}");
    assert!(two_d.pan.abs() < 1e-6, "got {two_d:?}");
    assert!((three_d.gain - 0.5).abs() < 1e-6, "got {three_d:?}");
    assert!((three_d.pan - 1.0).abs() < 1e-6, "got {three_d:?}");
    assert!((half.gain - 0.75).abs() < 1e-6, "got {half:?}");
    assert!((half.pan - 0.5).abs() < 1e-6, "got {half:?}");
}

#[test]
fn degenerate_band_is_a_hard_cutoff_at_initial() {
    let inside = resolve_3d(Vec3::new(0.0, 0.0, 1.0), 1.0, 2.0, 2.0);
    let outside = resolve_3d(Vec3::new(0.0, 0.0, 3.0), 1.0, 2.0, 2.0);
    assert_eq!(inside.gain, 1.0);
    assert_eq!(outside.gain, 0.0);
}

#[test]
fn voice_info_reports_resolved_spatial_gain_and_pan() {
    let mut m = AudioMaestro::default();
    let s = AudioSourceComponent {
        clip: "shot.ogg".to_string(),
        volume: 1.0,
        spatial_blend: 1.0,
        initial_distance: 1.0,
        final_distance: 5.0,
        ..Default::default()
    };
    m.play_source(1, &s, [3.0, 0.0, 0.0], 0);
    // Source at +X, distance 3 in a 1→5 band → gain 0.5, panned fully right.
    let info = m.voice_info(1, &s, Vec3::new(3.0, 0.0, 0.0), &at_origin());
    assert!(info.playing);
    assert!(
        (info.spatial.gain - 0.5).abs() < 1e-6,
        "got {:?}",
        info.spatial
    );
    assert!(
        (info.spatial.pan - 1.0).abs() < 1e-6,
        "got {:?}",
        info.spatial
    );
}
