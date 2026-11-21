//! How far a chain reaches beyond the signal it was handed — forwards in time, and backwards.

use super::{Spacing, Tempo, chorus, delay, reverb};
use crate::patch::Fx;
use crate::stereo::Stereo;

const MAX_TAIL: f32 = 15.0;

const FADE: f32 = 0.01;

pub(crate) fn lookahead_seconds(chain: &[Fx]) -> f32 {
    chain
        .iter()
        .map(|fx| match fx {
            Fx::Compress { attack, mix, .. } if *mix > 0.0 && attack.is_finite() => attack.max(0.0),
            _ => 0.0,
        })
        .sum()
}

pub(crate) fn tail_seconds(chain: &[Fx], tempo: &dyn Tempo) -> f32 {
    let total: f32 = chain
        .iter()
        .map(|fx| match fx {
            Fx::Delay {
                time,
                beats,
                feedback,
                mix,
                ..
            } if *mix > 0.0 => {
                let spacing = Spacing::new(*time, *beats, tempo);
                delay::tail_seconds(spacing.longest(), *feedback)
            }
            Fx::Reverb {
                size,
                mix,
                predelay,
                ..
            } if *mix > 0.0 => reverb::tail_seconds(*size) + reverb::predelay_seconds(*predelay),
            Fx::Chorus { depth, mix, .. } if *mix > 0.0 => chorus::tail_seconds(*depth),
            _ => 0.0,
        })
        .sum();
    total.min(MAX_TAIL)
}

pub(crate) fn close_tail(buf: &mut Stereo, chain: &[Fx], rate: f32, tempo: &dyn Tempo) {
    if tail_seconds(chain, tempo) <= 0.0 {
        return;
    }
    let frames = buf.frames();
    let fade = ((FADE * rate).round() as usize).min(frames);
    buf.each(|channel| {
        for (k, sample) in channel[frames - fade..].iter_mut().enumerate() {
            *sample *= 1.0 - (k + 1) as f32 / fade as f32;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::NoTempo;

    #[test]
    fn the_tail_covers_every_wet_effect_but_stays_bounded() {
        let wet = [
            Fx::Delay {
                time: 0.25,
                beats: None,
                feedback: 0.5,
                mix: 0.5,
                ping_pong: false,
            },
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 0.5,
                predelay: 0.0,
            },
        ];
        assert!(tail_seconds(&wet[..1], &NoTempo) > 0.0);
        assert!(tail_seconds(&wet, &NoTempo) > tail_seconds(&wet[..1], &NoTempo));
        assert!(tail_seconds(&wet, &NoTempo) <= MAX_TAIL);
        let long = vec![
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 1.0,
                predelay: 0.0,
            };
            8
        ];
        assert_eq!(
            tail_seconds(&long, &NoTempo),
            MAX_TAIL,
            "capped, however long the chain"
        );
    }

    #[test]
    fn a_dry_effect_asks_for_no_tail() {
        let dry = [
            Fx::Delay {
                time: 1.0,
                beats: None,
                feedback: 0.9,
                mix: 0.0,
                ping_pong: false,
            },
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 0.0,
                predelay: 0.0,
            },
            Fx::Chorus {
                rate: 0.5,
                depth: 1.0,
                voices: 4,
                mix: 0.0,
            },
        ];
        assert_eq!(tail_seconds(&dry, &NoTempo), 0.0);
    }

    #[test]
    fn a_chorus_asks_for_its_deepest_read_and_no_more() {
        let ensemble = |depth, mix| {
            [Fx::Chorus {
                rate: 0.5,
                depth,
                voices: 4,
                mix,
            }]
        };
        let shallow = tail_seconds(&ensemble(0.0, 1.0), &NoTempo);
        let deep = tail_seconds(&ensemble(1.0, 1.0), &NoTempo);
        assert!(shallow > 0.0, "a wet chorus does ring on: {shallow}");
        assert!(deep > shallow, "and further when it sweeps further");
        assert!(deep < 0.03, "but it is not a room: {deep}");
        let room = tail_seconds(
            &[Fx::Reverb {
                size: 0.5,
                damp: 0.5,
                mix: 1.0,
                predelay: 0.0,
            }],
            &NoTempo,
        );
        assert!(deep < room / 10.0, "{deep} against a small room's {room}");
    }

    #[test]
    fn a_rung_out_buffer_ends_on_silence_and_a_dry_one_is_left_alone() {
        let room = [Fx::Reverb {
            size: 0.5,
            damp: 0.5,
            mix: 1.0,
            predelay: 0.0,
        }];
        let mut rung = Stereo::centred(vec![0.5; 4410]);
        close_tail(&mut rung, &room, 44_100.0, &NoTempo);
        assert_eq!(rung.l[4409], 0.0, "the last sample is silence");
        assert_eq!(rung.l[3000], 0.5, "and only the last 10 ms moved");
        assert!(rung.l[4000] < 0.5 && rung.l[4000] > 0.0);
        let mut dry = Stereo::centred(vec![0.5; 4410]);
        close_tail(&mut dry, &[], 44_100.0, &NoTempo);
        assert_eq!(dry, Stereo::centred(vec![0.5; 4410]));
        let mut short = Stereo::centred(vec![0.5; 3]);
        close_tail(&mut short, &room, 44_100.0, &NoTempo);
        assert_eq!(
            short.l[2], 0.0,
            "a buffer shorter than the fade fades whole"
        );
        assert!(short.l[0] > short.l[1] && short.l[1] > 0.0);
    }

    #[test]
    fn only_a_compressor_asks_to_have_seen_the_future() {
        let compress = |attack, mix| Fx::Compress {
            threshold: -20.0,
            ratio: 4.0,
            attack,
            release: 0.1,
            makeup: 0.0,
            mix,
            sidechain: None,
        };
        assert_eq!(lookahead_seconds(&[]), 0.0);
        assert_eq!(
            lookahead_seconds(&[Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 1.0,
                predelay: 0.0,
            }]),
            0.0,
            "a room only ever repeats what it has already heard"
        );
        assert_eq!(lookahead_seconds(&[compress(0.05, 1.0)]), 0.05);
        assert_eq!(
            lookahead_seconds(&[compress(0.05, 1.0), compress(0.02, 1.0)]),
            0.07,
            "in series, so they add"
        );
        assert_eq!(
            lookahead_seconds(&[compress(0.05, 0.0)]),
            0.0,
            "a parked compressor changes nothing, so it reads nothing"
        );
        assert_eq!(lookahead_seconds(&[compress(f32::NAN, 1.0)]), 0.0);
        assert_eq!(lookahead_seconds(&[compress(-1.0, 1.0)]), 0.0);
    }
}
