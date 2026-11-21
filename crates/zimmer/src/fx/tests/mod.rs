//! Tests for running a chain: order, width, and what each effect reaches.

use super::*;

mod width;

fn impulse(n: usize) -> Stereo {
    let mut mono = vec![0.0; n];
    mono[0] = 1.0;
    Stereo::centred(mono)
}

#[test]
fn an_empty_chain_changes_nothing() {
    let mut buf = impulse(512);
    apply_chain(&mut buf, &[], 44_100.0);
    assert_eq!(buf, impulse(512));
}

#[test]
fn the_chain_applies_in_list_order() {
    // Delay-then-reverb smears the echoes; reverb-then-delay echoes the smear. They must
    // differ, which proves order is honoured rather than commuted.
    let chain = [
        Fx::Delay {
            time: 0.05,
            beats: None,
            feedback: 0.4,
            mix: 0.5,
            ping_pong: false,
        },
        Fx::Reverb {
            size: 0.6,
            damp: 0.5,
            mix: 0.5,
            predelay: 0.0,
        },
    ];
    let mut forward = impulse(22_050);
    let mut reversed = impulse(22_050);
    apply_chain(&mut forward, &chain, 44_100.0);
    apply_chain(
        &mut reversed,
        &[chain[1].clone(), chain[0].clone()],
        44_100.0,
    );
    assert_ne!(forward, reversed);
    assert!(forward.l.iter().chain(&forward.r).all(|s| s.is_finite()));
}

#[test]
fn a_waveshaper_reaches_the_signal_it_was_handed() {
    let drive = [Fx::Saturate {
        drive: 4.0,
        mix: 1.0,
    }];
    let mut buf = Stereo::centred(vec![0.5]);
    buf.resize(512);
    apply_chain(&mut buf, &drive, 44_100.0);
    assert!(
        buf.l[0] > 0.9,
        "the sample was shaped, not passed: {}",
        buf.l[0]
    );
    assert!(
        buf.l[1..].iter().all(|s| *s == 0.0),
        "and nothing after it moved, because there is no memory"
    );
}

#[test]
fn an_eq_reaches_the_signal_it_was_handed() {
    let carve = [Fx::Eq {
        bands: vec![EqBand {
            kind: EqKind::Peak,
            freq: Some(250.0),
            gain_db: -12.0,
            q: 2.0,
        }],
    }];
    let tone: Vec<f32> = (0..4410)
        .map(|i| (std::f32::consts::TAU * 250.0 * i as f32 / 44_100.0).sin())
        .collect();
    let mut buf = Stereo::centred(tone);
    let peak = |buf: &[f32]| buf[2205..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let before = peak(&buf.l);
    apply_chain(&mut buf, &carve, 44_100.0);
    let after = peak(&buf.l);
    assert!(
        after < before * 0.5,
        "the band was cut: {before} to {after}"
    );
    assert_eq!(buf.l, buf.r);
}

#[test]
fn a_chain_with_no_tracks_to_name_keys_a_compressor_from_its_own_signal() {
    let asked = [Fx::Compress {
        threshold: -20.0,
        ratio: 8.0,
        attack: 0.005,
        release: 0.05,
        makeup: 0.0,
        mix: 1.0,
        sidechain: Some("nowhere".to_owned()),
    }];
    let mut buf = Stereo::centred(vec![0.9; 4410]);
    apply_chain(&mut buf, &asked, 44_100.0);
    assert!(
        buf.l[2205] < 0.5,
        "it compressed nothing, so it read nothing: {}",
        buf.l[2205]
    );
    assert_eq!(buf.l, buf.r, "and one gain reached both sides");
}
