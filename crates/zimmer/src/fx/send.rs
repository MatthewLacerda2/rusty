//! A chain run beside a **send**: the whole mix, and the part of it that
//! reaches the room.
//!
//! A song's chain is one chain over the sum, and most of it means the same
//! thing to every track — glue, a shelf, a little drive. The room does not: a
//! kick and a bass want to stay dry while the pad over them rings, all in one
//! space. So the song's chain is run over **two** signals carried side by
//! side, the mix itself and the *sent* mix (each track at its `send`), and the
//! only stages that tell them apart are [`super::reverb`] and [`super::delay`]:
//! their dry side is the mix's and their tail is the sent mix's.
//!
//! Everything else runs on both, so the sent mix stays the same piece of music
//! as the mix it was taken from — an EQ before the reverb shapes what reaches
//! the room as well as what does not. Two choices make that hold:
//!
//! - **The room's own output is sent onward in full.** After a reverb the sent
//!   mix carries the tail too, so a delay further down the chain echoes the
//!   room, and a dry kick stays dry through both.
//! - **A compressor reads its level from the mix and applies the same gain to
//!   the sent mix.** Compressing the sent mix on its own level would duck it by
//!   a different amount than the signal it is a part of, and the room would
//!   pump against the music.
//!
//! The one stage that is honestly approximate is a `saturate` **before** a
//! room: drive is not linear, so the sent mix driven alone is not the sent
//! share of the mix driven whole. It is close, it is the obvious meaning, and
//! drive belongs after the room more often than before it.
//!
//! When every track sends everything, the sent mix *is* the mix, sample for
//! sample, at every stage — so this produces the samples [`super::apply_chain`]
//! does. The mixer still takes the plain path then, because it is half the
//! work; that it would agree is what makes a send of `1.0` mean "as before".

use super::{NoKeys, Spacing, Tempo, apply_one, compress, delay, reverb};
use crate::patch::Fx;
use crate::stereo::Stereo;

/// Runs `chain` over `buf`, with its rooms hearing `sent` instead of `buf`,
/// and its delays timed in beats reading them off `tempo`.
///
/// `sent` is carried through the chain too, and comes out as the sent share of
/// the result; the caller wants `buf`. The two should be the same length — the
/// rooms hear silence past the end of a shorter `sent`.
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
                reverb::apply_sent(buf, sent, *size, *damp, *predelay, *mix, rate);
                reverb::apply(sent, *size, *damp, *predelay, *mix, rate);
            }
            Fx::Delay {
                time,
                beats,
                feedback,
                mix,
                ping_pong: true,
            } => {
                let spacing = Spacing::new(*time, *beats, tempo);
                delay::ping_pong_sent(buf, sent, spacing, *feedback, *mix, rate);
                delay::ping_pong(sent, spacing, *feedback, *mix, rate);
            }
            Fx::Delay {
                time,
                beats,
                feedback,
                mix,
                ping_pong: false,
            } => {
                let spacing = Spacing::new(*time, *beats, tempo);
                delay::apply_sent(&mut buf.l, &sent.l, spacing, *feedback, *mix, rate);
                delay::apply_sent(&mut buf.r, &sent.r, spacing, *feedback, *mix, rate);
                sent.each(|channel| delay::apply(channel, spacing, *feedback, *mix, rate));
            }
            Fx::Compress {
                threshold,
                ratio,
                attack,
                release,
                makeup,
                mix,
                // A song's chain has no tracks to key from; validation refuses
                // one there, and the plain chain ignores it the same way.
                sidechain: _,
            } => {
                let compressor =
                    compress::Compressor::new(*threshold, *ratio, *attack, *release, *makeup, *mix);
                let level = buf.clone();
                compressor.apply(buf, None, rate);
                compressor.apply(sent, Some(&level), rate);
            }
            other => {
                apply_one(buf, other, rate, &NoKeys, tempo);
                apply_one(sent, other, rate, &NoKeys, tempo);
            }
        }
    }
}

/// What the send changes, and what it leaves alone, by the sample.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{NoTempo, apply_chain};

    const RATE: f32 = 44_100.0;

    /// A burst at the start and silence after it, long enough for a tail.
    fn burst() -> Stereo {
        let mut mono = vec![0.0; 22_050];
        for (i, sample) in mono.iter_mut().take(200).enumerate() {
            *sample = ((i as f32) * 0.3).sin() * 0.8;
        }
        Stereo::centred(mono)
    }

    /// Every kind of stage, rooms between the others, so the identity below
    /// covers each arm.
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

    /// The promise a send of `1.0` makes: the same samples, not nearly.
    #[test]
    fn sending_everything_is_the_plain_chain_sample_for_sample() {
        let mut plain = burst();
        apply_chain(&mut plain, &everything(), RATE);
        let mut mixed = burst();
        let mut sent = burst();
        apply_chain_sent(&mut mixed, &mut sent, &everything(), RATE, &NoTempo);
        assert_eq!(mixed, plain);
    }

    /// Sending nothing leaves a room nothing to ring with: what comes out is
    /// the dry side alone, at the room's dry level.
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

    /// The tail is the sent mix's: half the signal sent is half the tail.
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

    /// A compressor on the mix reaches the sent mix with the mix's own gain,
    /// so a quiet send is not compressed less than the signal it belongs to.
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
