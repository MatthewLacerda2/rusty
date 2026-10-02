//! Reverb zones (#469): the listener's weight in a zone, how overlapping zones
//! blend, and what the maestro hands the device.

use glam::Vec3;

use super::*;
use crate::audio::recording::RecordingBackend;
use crate::audio::AudioMaestro;
use crate::components::ReverbPreset;

/// A zone of `preset` with radii 10 / 15 (Unity's defaults).
fn zone(preset: ReverbPreset) -> ReverbZoneComponent {
    ReverbZoneComponent {
        preset,
        params: preset.params().unwrap(),
        ..Default::default()
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn weight_is_full_inside_linear_between_and_zero_outside() {
    assert_eq!(weight(0.0, 10.0, 15.0), 1.0);
    assert_eq!(weight(10.0, 10.0, 15.0), 1.0, "the inner radius is full");
    assert!(close(weight(12.5, 10.0, 15.0), 0.5));
    assert_eq!(weight(15.0, 10.0, 15.0), 0.0);
    assert_eq!(weight(40.0, 10.0, 15.0), 0.0);
    assert_eq!(
        weight(5.0, 5.0, 5.0),
        1.0,
        "a hard edge still has an inside"
    );
    assert_eq!(weight(5.1, 5.0, 5.0), 0.0);
}

#[test]
fn one_zone_fades_its_level_but_keeps_its_character() {
    let hall = zone(ReverbPreset::Hall);
    let at = |x: f32| blend(Vec3::new(x, 0.0, 0.0), &[(Vec3::ZERO, &hall)]);
    let inside = at(3.0);
    assert_eq!(inside.params, hall.params);
    assert_eq!((inside.weight, inside.zones), (1.0, 1));
    let between = at(12.5);
    assert!(close(between.params.wet, hall.params.wet * 0.5));
    assert!(close(between.params.decay_time, hall.params.decay_time));
    assert!(close(between.weight, 0.5));
    assert_eq!(at(20.0), ReverbState::DRY, "outside: dry");
}

#[test]
fn overlapping_zones_blend_by_weight() {
    let (room, tunnel) = (zone(ReverbPreset::Room), zone(ReverbPreset::Tunnel));
    let (r, t) = (room.params, tunnel.params);
    // Inside both: an even average, not a doubled level.
    let both = blend(Vec3::ZERO, &[(Vec3::ZERO, &room), (Vec3::X, &tunnel)]);
    assert!(close(
        both.params.decay_time,
        (r.decay_time + t.decay_time) / 2.0
    ));
    assert!(close(both.params.wet, (r.wet + t.wet) / 2.0));
    assert_eq!((both.weight, both.zones), (1.0, 2));
    // Full in the room, half-way out of the tunnel: weights 1 and ½.
    let tunnel_at = Vec3::new(12.5, 0.0, 0.0);
    let mixed = blend(Vec3::ZERO, &[(Vec3::ZERO, &room), (tunnel_at, &tunnel)]);
    assert!(close(
        mixed.params.damping,
        (r.damping + 0.5 * t.damping) / 1.5
    ));
    assert!(close(mixed.params.wet, (r.wet + 0.5 * t.wet) / 1.5));
}

#[test]
fn an_off_zone_dries_out_the_zone_it_sits_in() {
    let (hall, off) = (zone(ReverbPreset::Hall), zone(ReverbPreset::Off));
    let state = blend(Vec3::ZERO, &[(Vec3::ZERO, &hall), (Vec3::ZERO, &off)]);
    assert!(close(state.params.wet, hall.params.wet / 2.0));
}

#[test]
fn the_maestro_sends_only_changes_and_stop_dries_the_bus() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let hall = zone(ReverbPreset::Hall);
    m.resolve_reverb(Vec3::ZERO, &[(Vec3::ZERO, &hall)]);
    m.resolve_reverb(Vec3::ZERO, &[(Vec3::ZERO, &hall)]);
    assert_eq!(m.reverb_state().params, hall.params);
    m.exit_play();
    assert_eq!(m.reverb_state(), ReverbState::DRY);
    let sent = &rec.borrow().reverbs;
    assert_eq!(sent.as_slice(), [hall.params, ReverbParams::DRY]);
}
