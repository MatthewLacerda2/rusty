//! Doppler in the per-tick mix (#1749): which pushes measure motion and which keep
//! the rate, and what counts as a jump.

use glam::{Quat, Vec3};

use crate::audio::{AudioMaestro, Listener, MixEnv};
use crate::components::AudioSourceComponent;

const UP: f32 = 343.0 / 293.0;

fn env(listener: Vec3, dt: f32) -> MixEnv {
    MixEnv {
        listener: Listener::from_transform(listener, Quat::IDENTITY),
        dt,
        ..MixEnv::default()
    }
}

fn source(spatial_blend: f32) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: "hum.ogg".into(),
        looping: true,
        spatial_blend,
        ..Default::default()
    }
}

/// A maestro with entity 1 playing `src` at x = -100, then one push of it moved to
/// `x`, the listener at the origin; the rate it plays at.
fn pushed(src: &AudioSourceComponent, x: f32, dt: f32) -> (AudioMaestro, f32) {
    let mut maestro = AudioMaestro::default();
    maestro.play_source(1, src, [-100.0, 0.0, 0.0], 0);
    let at = Vec3::new(x, 0.0, 0.0);
    maestro.apply_mix(&env(Vec3::ZERO, dt), |_| Some((src.clone(), at)));
    let rate = maestro.source_doppler(1);
    (maestro, rate)
}

#[test]
fn a_3d_source_closing_at_50_ms_is_shifted_up() {
    let (_, rate) = pushed(&source(1.0), -99.5, 0.01);
    assert!((rate - UP).abs() < 1e-4, "{rate}");
}

#[test]
fn a_2d_source_or_level_zero_never_shifts() {
    assert_eq!(pushed(&source(0.0), -99.5, 0.01).1, 1.0, "2D");
    let off = AudioSourceComponent {
        doppler_level: 0.0,
        ..source(1.0)
    };
    assert_eq!(pushed(&off, -99.5, 0.01).1, 1.0, "level 0");
}

#[test]
fn a_push_with_no_sim_time_keeps_the_rate() {
    let src = source(1.0);
    let (mut maestro, rate) = pushed(&src, -99.5, 0.01);
    // Paused: the source does not move, the push carries no sim time.
    let at = Vec3::new(-99.5, 0.0, 0.0);
    maestro.apply_mix(&env(Vec3::ZERO, 0.0), |_| Some((src.clone(), at)));
    assert_eq!(maestro.source_doppler(1), rate, "held through the pause");
    // Resumed: one tick of motion again, no jump from the time paused.
    let on = Vec3::new(-99.0, 0.0, 0.0);
    maestro.apply_mix(&env(Vec3::ZERO, 0.01), |_| Some((src.clone(), on)));
    assert!((maestro.source_doppler(1) - UP).abs() < 1e-4);
}

#[test]
fn a_teleport_is_no_shift() {
    let (_, rate) = pushed(&source(1.0), 0.0, 0.01);
    assert_eq!(rate, 1.0, "100 m in one tick is faster than sound");
}

#[test]
fn a_forgotten_listener_jump_is_no_shift() {
    let src = source(1.0);
    let at = Vec3::new(-100.0, 0.0, 0.0);
    let mut maestro = AudioMaestro::default();
    maestro.play_source(1, &src, at.to_array(), 0);
    maestro.apply_mix(&env(Vec3::ZERO, 0.01), |_| Some((src.clone(), at)));
    // A scene load puts the camera 1 m closer: 100 m/s if it were measured.
    maestro.forget_motion();
    let near = Vec3::new(-1.0, 0.0, 0.0);
    maestro.apply_mix(&env(near, 0.01), |_| Some((src.clone(), at)));
    assert_eq!(maestro.source_doppler(1), 1.0);
    // Measured, the same jump shifts it.
    maestro.apply_mix(&env(near * 2.0, 0.01), |_| Some((src.clone(), at)));
    assert!(maestro.source_doppler(1) > 1.0);
}

#[test]
fn the_speed_of_sound_sets_the_shift_and_refuses_nonsense() {
    let src = source(1.0);
    let mut maestro = AudioMaestro::default();
    assert_eq!(maestro.speed_of_sound(), 343.0);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(!maestro.set_speed_of_sound(bad), "{bad}");
    }
    assert!(maestro.set_speed_of_sound(100.0));
    maestro.play_source(1, &src, [-100.0, 0.0, 0.0], 0);
    let at = Vec3::new(-99.5, 0.0, 0.0);
    maestro.apply_mix(&env(Vec3::ZERO, 0.01), |_| Some((src.clone(), at)));
    assert!(
        (maestro.source_doppler(1) - 2.0).abs() < 1e-4,
        "100/50, clamped at 2"
    );
}
