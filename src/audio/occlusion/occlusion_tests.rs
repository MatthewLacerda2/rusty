//! Occlusion (#467): the effect curve, the ray fan, smoothing, and the maestro's
//! casts and round-robin over a fake occluder (a wall across z = 5).

use std::cell::Cell;
use std::rc::Rc;

use glam::Vec3;

use super::*;
use crate::audio::recording::RecordingBackend;
use crate::audio::AudioMaestro;
use crate::components::AudioSourceComponent;

#[test]
fn the_effect_is_untouched_at_zero_and_full_muffle_at_one() {
    assert_eq!(effect(0.0), (1.0, LOW_PASS_OFF));
    let (gain, cutoff) = effect(1.0);
    assert!((gain - OCCLUDED_GAIN).abs() < 1e-6);
    assert!((cutoff - OCCLUDED_CUTOFF).abs() < 0.5, "{cutoff}");
    let (half_gain, half_cut) = effect(0.5);
    assert!(half_gain < 1.0 && half_gain > OCCLUDED_GAIN);
    let octave_mid = (LOW_PASS_OFF * OCCLUDED_CUTOFF).sqrt();
    assert!((half_cut - octave_mid).abs() < 1.0, "octaves: {half_cut}");
}

#[test]
fn the_fan_is_level_and_spread_across_the_line_of_sight() {
    let [mid, a, b] = ray_targets(Vec3::ZERO, Vec3::new(0.0, 1.0, 10.0));
    assert_eq!(mid, Vec3::new(0.0, 1.0, 10.0));
    assert_eq!((a.y, b.y), (1.0, 1.0), "level with the source");
    assert!((a.x.abs() - SOURCE_SPREAD).abs() < 1e-6 && a.x == -b.x);
}

#[test]
fn smoothing_is_linear_over_the_fade() {
    let mut v = VoiceOcclusion::at(1.0);
    v.target = 0.0;
    v.step(FADE_SECONDS / 2.0);
    assert!((v.value - 0.5).abs() < 1e-6);
    v.step(FADE_SECONDS);
    assert_eq!(v.value, 0.0, "lands, never overshoots");
}

/// A maestro whose occluder sees a wall across z = 5 for `|x| < half_width` while
/// `wall` is up, counting the rays cast.
fn rig(wall: Rc<Cell<bool>>, half_width: f32, rays: Rc<Cell<u32>>) -> AudioMaestro {
    let mut m = AudioMaestro::default();
    m.set_occluder(Box::new(move |from: Vec3, to: Vec3, _, _| {
        rays.set(rays.get() + 1);
        let t = (5.0 - from.z) / (to.z - from.z);
        let x = from.x + (to.x - from.x) * t;
        wall.get() && (0.0..=1.0).contains(&t) && x.abs() < half_width
    }));
    m
}

fn spatial() -> AudioSourceComponent {
    AudioSourceComponent {
        clip: "shot.ogg".into(),
        spatial_blend: 1.0,
        final_distance: 100.0,
        ..Default::default()
    }
}

#[test]
fn a_voice_behind_a_wall_starts_muffled_and_clears_after_smoothing() {
    let wall = Rc::new(Cell::new(true));
    let mut m = rig(Rc::clone(&wall), 10.0, Rc::default());
    let (backend, rec) = RecordingBackend::new();
    m.set_backend(backend);
    m.play_source(1, &spatial(), [0.0, 0.0, 10.0], 0);
    assert_eq!(m.source_occlusion(1), 1.0, "cast at start");
    let first = rec.borrow().current(rec.borrow().last_voice()).unwrap();
    assert!(first.low_pass < 1_000.0 && first.gain < 0.5, "{first:?}");

    wall.set(false);
    let at = |_| Some((spatial(), Vec3::new(0.0, 0.0, 10.0)));
    m.occlude(Vec3::ZERO, 0.1, at);
    let mid = m.source_occlusion(1);
    assert!(mid > 0.0 && mid < 1.0, "still fading: {mid}");
    m.occlude(Vec3::ZERO, FADE_SECONDS, at);
    assert_eq!(m.source_occlusion(1), 0.0);
}

#[test]
fn a_door_frame_occludes_partially() {
    let mut m = rig(Rc::new(Cell::new(true)), 0.2, Rc::default());
    m.play_source(1, &spatial(), [0.0, 0.0, 10.0], 0);
    let third = m.source_occlusion(1);
    assert!(
        (third - 1.0 / 3.0).abs() < 1e-6,
        "only the middle ray: {third}"
    );
}

#[test]
fn opted_out_2d_and_strength_zero_are_never_cast() {
    let rays = Rc::new(Cell::new(0));
    let mut m = rig(Rc::new(Cell::new(true)), 10.0, Rc::clone(&rays));
    let opted_out = AudioSourceComponent {
        occlusion_enabled: false,
        ..spatial()
    };
    m.play_source(1, &opted_out, [0.0, 0.0, 10.0], 0);
    m.play_source(2, &AudioSourceComponent::default(), [0.0, 0.0, 10.0], 0);
    assert_eq!((m.source_occlusion(1), m.source_occlusion(2)), (0.0, 0.0));
    m.set_occlusion_settings(OcclusionSettings {
        strength: 0.0,
        ..Default::default()
    });
    m.play_source(3, &spatial(), [0.0, 0.0, 10.0], 0);
    m.occlude(Vec3::ZERO, 0.1, |_| None);
    assert_eq!(rays.get(), 0);
    assert_eq!(m.source_occlusion(3), 0.0);
}

#[test]
fn the_budget_round_robins_every_voice() {
    let rays = Rc::new(Cell::new(0));
    let wall = Rc::new(Cell::new(false));
    let mut m = rig(Rc::clone(&wall), 10.0, Rc::clone(&rays));
    m.set_occlusion_settings(OcclusionSettings {
        voices_per_tick: 1,
        ..Default::default()
    });
    for id in 1..=3 {
        m.play_source(id, &spatial(), [0.0, 0.0, 10.0], 0);
    }
    wall.set(true);
    rays.set(0);
    let mut cast = Vec::new();
    for _ in 0..3 {
        m.occlude(Vec3::ZERO, 0.0, |_| None);
        cast.push((1..=3).filter(|&id| m.voice_target(id) == 1.0).count());
    }
    assert_eq!(rays.get(), 3 * 3, "one voice (three rays) per tick");
    assert_eq!(cast, [1, 2, 3], "each voice in turn");
}

impl AudioMaestro {
    /// Entity `id`'s latest cast, for the round-robin test.
    fn voice_target(&self, id: u32) -> f32 {
        let voice = self.entity_voices[&id].voice;
        self.occlusion.voices[&voice].target
    }
}
