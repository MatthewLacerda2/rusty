//! Master's chain after the reverb bus (#1855): a volley inside a big zone at full
//! wet stays under the limiter's ceiling, a mix under the ceiling is untouched,
//! and the speaker-mode stage runs after the limiter. Home theater passes it bit
//! for bit, headphones' crossfeed only averages the two channels so the ceiling
//! holds, and TV's +9 dB makeup lifts it again into the mode's own soft limit,
//! which stays under full scale.

use super::loud_tests::{capture, ceiling, peak};
use super::*;
use crate::mixer::{EffectChain, GroupPatch};
use crate::reverb::ReverbState;
use crate::SpeakerMode;
use rusty_components::ReverbParams;

/// SFX sending everything into a long zone at full wet, `master`'s effects on Master.
pub(super) fn hall(maestro: &mut AudioMaestro, master: EffectChain) {
    let send = GroupPatch {
        reverb_send: Some(1.0),
        ..Default::default()
    };
    maestro.set_group("SFX", &send).unwrap();
    maestro.set_group_effects("Master", master).unwrap();
    let params = ReverbParams {
        decay_time: 8.0,
        pre_delay: 0.0,
        damping: 0.0,
        wet: 1.0,
    };
    let weight = 1.0;
    maestro.set_reverb(ReverbState {
        params,
        weight,
        zones: 1,
    });
}

/// `shots` gunshots at `volume` in the [`hall`].
fn in_a_hall(name: &str, shots: u32, volume: f32, master: EffectChain) -> Vec<f32> {
    capture(name, shots, volume, |maestro| hall(maestro, master))
}

#[test]
fn a_volley_in_a_full_wet_zone_stays_under_the_ceiling_with_the_reverb_limited_too() {
    let raw = in_a_hall("rusty_hall_raw.wav", 8, 0.5, EffectChain::default());
    let raw_peak = peak(&raw);
    assert!(
        raw_peak > 1.0,
        "the reverb takes it past full scale: {raw_peak}"
    );
    let limited = peak(&in_a_hall("rusty_hall.wav", 8, 0.5, EffectChain::master()));
    assert!(
        limited <= ceiling() * 1.000_001,
        "peak {limited} past −0.3 dBFS"
    );
    assert!(limited > 0.9, "and the hall is loud: {limited}");
}

#[test]
fn a_mix_under_the_ceiling_in_a_zone_is_bit_identical_with_the_limiter_on() {
    let raw = in_a_hall("rusty_quiet_raw.wav", 1, 0.1, EffectChain::default());
    let quiet = peak(&raw);
    assert!(
        quiet > 0.01 && quiet < ceiling() * 0.9,
        "quiet but heard: {quiet}"
    );
    let limited = in_a_hall("rusty_quiet.wav", 1, 0.1, EffectChain::master());
    assert!(
        raw == limited,
        "the limiter changed a mix under its ceiling"
    );
}

/// A loud volley in the hall, Master's default limiter on, played in `mode`.
fn volley_in(name: &str, mode: SpeakerMode) -> Vec<f32> {
    capture(name, 8, 0.5, |maestro| {
        maestro.set_speaker_mode(mode);
        hall(maestro, EffectChain::master());
    })
}

#[test]
fn the_ceiling_holds_in_home_theater_and_headphones_and_tv_stays_under_full_scale() {
    let home = peak(&volley_in("rusty_mode_home.wav", SpeakerMode::HomeTheater));
    assert!(home <= ceiling() * 1.000_001, "home theater: {home}");
    let phones = peak(&volley_in("rusty_mode_phones.wav", SpeakerMode::Headphones));
    assert!(phones <= ceiling() * 1.000_001, "headphones: {phones}");
    let tv = peak(&volley_in("rusty_mode_tv.wav", SpeakerMode::Tv));
    assert!(tv < 1.0, "tv's soft limit keeps it under full scale: {tv}");
    assert!(
        tv > ceiling(),
        "tv's makeup lifts it past the master ceiling: {tv}"
    );
}
