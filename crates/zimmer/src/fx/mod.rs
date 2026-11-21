//! fx — the post-chain: delay, reverb, drive, EQ, compression, chorus, and the mandatory limiter

pub(crate) mod chorus;
pub(crate) mod compress;
pub(crate) mod delay;
pub(crate) mod eq;
mod gain;
pub(crate) mod limiter;
pub(crate) mod reach;
pub(crate) mod reverb;
pub(crate) mod saturate;
mod send;

pub(crate) use delay::Spacing;
pub(crate) use reach::{close_tail, lookahead_seconds, tail_seconds};
pub(crate) use send::apply_chain_sent;

use crate::patch::Fx;
#[cfg(test)]
use crate::patch::{EqBand, EqKind};
use crate::stereo::Stereo;

pub(crate) trait Keys {
    fn part(&self, track: &str) -> Option<&Stereo>;
}

pub(crate) trait Tempo {
    fn beat_seconds(&self, frame: usize) -> f32;
    fn longest_beat(&self) -> f32;
}

pub(crate) struct NoTempo;

impl Tempo for NoTempo {
    fn beat_seconds(&self, _frame: usize) -> f32 {
        0.0
    }
    fn longest_beat(&self) -> f32 {
        0.0
    }
}

struct NoKeys;

impl Keys for NoKeys {
    fn part(&self, _track: &str) -> Option<&Stereo> {
        None
    }
}

pub(crate) fn apply_chain(buf: &mut Stereo, chain: &[Fx], rate: f32) {
    apply_chain_timed(buf, chain, rate, &NoTempo);
}

pub(crate) fn apply_chain_timed(buf: &mut Stereo, chain: &[Fx], rate: f32, tempo: &dyn Tempo) {
    apply_chain_keyed(buf, chain, rate, &NoKeys, tempo);
}

pub(crate) fn apply_chain_keyed(
    buf: &mut Stereo,
    chain: &[Fx],
    rate: f32,
    keys: &dyn Keys,
    tempo: &dyn Tempo,
) {
    for fx in chain {
        apply_one(buf, fx, rate, keys, tempo);
    }
}

fn apply_one(buf: &mut Stereo, fx: &Fx, rate: f32, keys: &dyn Keys, tempo: &dyn Tempo) {
    match fx {
        Fx::Delay {
            time,
            beats,
            feedback,
            mix,
            ping_pong: true,
        } => {
            let spacing = Spacing::new(*time, *beats, tempo);
            delay::ping_pong(buf, spacing, *feedback, *mix, rate);
        }
        Fx::Delay {
            time,
            beats,
            feedback,
            mix,
            ping_pong: false,
        } => {
            let spacing = Spacing::new(*time, *beats, tempo);
            buf.each(|channel| delay::apply(channel, spacing, *feedback, *mix, rate));
        }
        Fx::Reverb {
            size,
            damp,
            mix,
            predelay,
        } => reverb::apply(buf, *size, *damp, *predelay, *mix, rate),
        Fx::Saturate { drive, mix } => {
            buf.each(|channel| saturate::apply(channel, *drive, *mix));
        }
        Fx::Eq { bands } => buf.each(|channel| eq::apply(channel, bands, rate)),
        Fx::Compress {
            threshold,
            ratio,
            attack,
            release,
            makeup,
            mix,
            sidechain,
        } => {
            let key = sidechain.as_deref().and_then(|track| keys.part(track));
            compress::Compressor::new(*threshold, *ratio, *attack, *release, *makeup, *mix)
                .apply(buf, key, rate);
        }
        Fx::Chorus {
            rate: sweep,
            depth,
            voices,
            mix,
        } => chorus::apply(buf, *sweep, *depth, *voices, *mix, rate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn only_the_room_the_ensemble_and_a_ping_pong_go_off_centre() {
        let mut room = impulse(22_050);
        apply_chain(
            &mut room,
            &[Fx::Reverb {
                size: 0.7,
                damp: 0.5,
                mix: 1.0,
                predelay: 0.0,
            }],
            44_100.0,
        );
        assert_ne!(room.l, room.r, "a room has two sides");
        let mut section = impulse(22_050);
        apply_chain(
            &mut section,
            &[Fx::Chorus {
                rate: 0.8,
                depth: 0.7,
                voices: 4,
                mix: 1.0,
            }],
            44_100.0,
        );
        assert_ne!(section.l, section.r, "the copies are placed apart");
        let mut narrow = impulse(22_050);
        apply_chain(
            &mut narrow,
            &[
                Fx::Delay {
                    time: 0.05,
                    beats: None,
                    feedback: 0.4,
                    mix: 0.5,
                    ping_pong: false,
                },
                Fx::Saturate {
                    drive: 4.0,
                    mix: 1.0,
                },
            ],
            44_100.0,
        );
        assert_eq!(narrow.l, narrow.r, "neither of these is a width effect");
        let mut walked = impulse(22_050);
        apply_chain(
            &mut walked,
            &[Fx::Delay {
                time: 0.05,
                beats: None,
                feedback: 0.4,
                mix: 0.5,
                ping_pong: true,
            }],
            44_100.0,
        );
        assert_ne!(walked.l, walked.r, "the repeats crossed over");
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
}
