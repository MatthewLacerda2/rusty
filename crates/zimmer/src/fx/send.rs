//! A chain run beside a **send**: the whole mix, and the part of it that reaches the room.

use super::{apply_one, compress, delay, reverb, NoKeys, Spacing, Tempo};
use crate::patch::Fx;
use crate::stereo::Stereo;

/// `sent` should be as long as `buf`: rooms hear silence past a shorter one.
pub(crate) fn apply_chain_sent(
    buf: &mut Stereo,
    sent: &mut Stereo,
    chain: &[Fx],
    rate: f32,
    tempo: &dyn Tempo,
) {
    for fx in chain {
        match fx {
            Fx::Reverb {
                size,
                damp,
                mix,
                predelay,
            } => {
                reverb::apply_sent(buf, sent, [*size, *damp, *predelay], *mix, rate);
                // The room's output goes on in the send too, so a later delay echoes the room.
                reverb::apply(sent, *size, *damp, *predelay, *mix, rate);
            }
            Fx::Delay {
                time,
                beats,
                feedback,
                mix,
                ping_pong,
            } => {
                let spacing = Spacing::new(*time, *beats, tempo);
                if *ping_pong {
                    delay::ping_pong_sent(buf, sent, spacing, *feedback, *mix, rate);
                    delay::ping_pong(sent, spacing, *feedback, *mix, rate);
                } else {
                    delay::apply_sent(&mut buf.l, &sent.l, spacing, *feedback, *mix, rate);
                    delay::apply_sent(&mut buf.r, &sent.r, spacing, *feedback, *mix, rate);
                    sent.each(|channel| delay::apply(channel, spacing, *feedback, *mix, rate));
                }
            }
            Fx::Compress {
                threshold,
                ratio,
                attack,
                release,
                makeup,
                mix,
                // Validation refuses a `sidechain` on the song's chain.
                sidechain: _,
            } => {
                let compressor =
                    compress::Compressor::new(*threshold, *ratio, *attack, *release, *makeup, *mix);
                // Cloned before the mix is compressed: the sent mix takes the mix's own gain.
                let level = buf.clone();
                compressor.apply(buf, None, rate);
                compressor.apply(sent, Some(&level), rate);
            }
            // Drive is not linear, so a `saturate` before a room only approximates the sent share.
            other => {
                apply_one(buf, other, rate, &NoKeys, tempo);
                apply_one(sent, other, rate, &NoKeys, tempo);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{apply_chain, NoTempo};

    const RATE: f32 = 44_100.0;

    fn burst() -> Stereo {
        let mut mono = vec![0.0; 22_050];
        for (i, sample) in mono.iter_mut().take(200).enumerate() {
            *sample = ((i as f32) * 0.3).sin() * 0.8;
        }
        Stereo::centred(mono)
    }

    fn everything() -> Vec<Fx> {
        vec![
            Fx::Saturate {
                drive: 1.5,
                mix: 0.5,
            },
            Fx::Reverb {
                size: 0.6,
                damp: 0.4,
                mix: 0.3,
                predelay: 0.0,
            },
            Fx::Compress {
                threshold: -12.0,
                ratio: 2.0,
                attack: 0.01,
                release: 0.1,
                makeup: 2.0,
                mix: 1.0,
                sidechain: None,
            },
            Fx::Delay {
                time: 0.05,
                beats: None,
                feedback: 0.3,
                mix: 0.2,
                ping_pong: false,
            },
            Fx::Delay {
                time: 0.07,
                beats: None,
                feedback: 0.3,
                mix: 0.2,
                ping_pong: true,
            },
        ]
    }

    #[test]
    fn sending_everything_is_the_plain_chain_sample_for_sample() {
        let mut plain = burst();
        apply_chain(&mut plain, &everything(), RATE);
        let mut mixed = burst();
        let mut sent = burst();
        apply_chain_sent(&mut mixed, &mut sent, &everything(), RATE, &NoTempo);
        assert_eq!(mixed, plain);
    }

    #[test]
    fn sending_nothing_leaves_only_the_dry_side() {
        let room = [Fx::Reverb {
            size: 0.7,
            damp: 0.5,
            mix: 0.25,
            predelay: 0.0,
        }];
        let mut mixed = burst();
        let mut sent = Stereo::silence(mixed.frames());
        apply_chain_sent(&mut mixed, &mut sent, &room, RATE, &NoTempo);
        let dry = burst();
        for (out, input) in mixed.l.iter().zip(&dry.l) {
            assert_eq!(*out, input * 0.75);
        }
        assert!(sent.l.iter().all(|s| *s == 0.0), "nothing was sent");
    }

    #[test]
    fn the_tail_scales_with_what_was_sent() {
        let echo = [Fx::Delay {
            time: 0.05,
            beats: None,
            feedback: 0.0,
            mix: 0.5,
            ping_pong: false,
        }];
        let mut mixed = burst();
        let mut sent = burst();
        sent.each(|channel| channel.iter_mut().for_each(|s| *s *= 0.5));
        apply_chain_sent(&mut mixed, &mut sent, &echo, RATE, &NoTempo);
        let at = 2205 + 10;
        assert_eq!(mixed.l[at], burst().l[10] * 0.5 * 0.5, "the repeat");
        assert_eq!(mixed.l[10], burst().l[10] * 0.5, "the dry side, whole");
    }

    #[test]
    fn a_compressor_ducks_the_send_by_what_it_ducked_the_mix() {
        let squeeze = [Fx::Compress {
            threshold: -30.0,
            ratio: 8.0,
            attack: 0.001,
            release: 0.05,
            makeup: 0.0,
            mix: 1.0,
            sidechain: None,
        }];
        let mut mixed = Stereo::centred(vec![0.9; 4410]);
        let mut sent = Stereo::centred(vec![0.09; 4410]);
        apply_chain_sent(&mut mixed, &mut sent, &squeeze, RATE, &NoTempo);
        let ratio = sent.l[2205] / mixed.l[2205];
        assert!((ratio - 0.1).abs() < 1e-5, "the share held: {ratio}");
    }
}
