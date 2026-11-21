//! How far a chain reaches beyond the signal it was handed — forwards in
//! time, and backwards.
//!
//! Two numbers, and a renderer that means to produce anything other than a
//! whole piece needs both. A **tail** is how much buffer a chain still has to
//! write into after its input stops: the echo that has not repeated yet, the
//! room that has not finished decaying. A **lookahead** is the opposite
//! question and the less obvious one — how much input a chain has to have
//! *seen* for the sample it is producing now to be right. A compressor's gain
//! is already on its way down before the peak that asks for it, so a sample
//! here depends on signal an attack away.
//!
//! They live together because they are the same fact from either end, and
//! because the two callers are the same two callers: the note and bus lengths
//! come off the tail, and [`crate::song::excerpt`]'s guard comes off the
//! lookahead.

use super::{Spacing, Tempo, chorus, delay, reverb};
use crate::patch::Fx;
use crate::stereo::Stereo;

/// Longest fx tail a note is padded by, in seconds. A chain of long effects should
/// not turn a 200 ms blip into a half-minute file — but a full-size room rings
/// for over ten (#1123), so the cap sits past the longest one effect makes.
const MAX_TAIL: f32 = 15.0;

/// How long the very end of a ring-out is faded over, in seconds.
///
/// A tail is sized to where an effect has fallen 60 dB, which is quiet but not
/// zero, and [`MAX_TAIL`] can cut a long chain sooner. Either way the buffer
/// stops on whatever sample the echo was at; a few milliseconds of fade turn
/// that step into nothing an ear can find.
const FADE: f32 = 0.01;

/// How far **ahead of itself** the chain looks: the sum of the attack ramps
/// its compressors duck over.
///
/// The mirror of [`tail_seconds`], and the other thing a renderer that means
/// to produce less than a whole piece has to know. A tail says how much extra
/// buffer a chain needs at the end; this says how much extra *signal* it needs
/// to have seen — a compressor's gain is already on its way down before the
/// peak that asks for it, so a sample here depends on input up to an attack
/// away. Summed rather than maxed because a chain runs in series, and each
/// stage's lookahead is spent on the stage before it.
///
/// Everything else answers zero, and each for a reason it already carries: a
/// delay, a reverb and a chorus read only the past, a waveshaper reads only
/// the present, and an EQ's biquad is the same. Only [`compress`] and
/// [`limiter`] compute a gain track and walk it backwards.
pub(crate) fn lookahead_seconds(chain: &[Fx]) -> f32 {
    chain
        .iter()
        .map(|fx| match fx {
            Fx::Compress { attack, mix, .. } if *mix > 0.0 && attack.is_finite() => attack.max(0.0),
            _ => 0.0,
        })
        .sum()
}

/// How much extra time the chain needs to ring out after the note itself ends —
/// what the renderer pads the buffer by, so an echo or a reverb tail is never cut
/// off mid-repeat. A fully dry effect asks for nothing, and neither does a
/// waveshaper: [`saturate`] is memoryless, so there is no state left in it to
/// decay once the signal stops. Nor does [`eq`], which does have state but
/// neither delays nor repeats — its own module doc argues that one. Nor does
/// [`compress`], for the plainest reason of the three: it only ever scales the
/// samples it was handed, so silence stays silence however long the release
/// says the gain takes to come back.
///
/// [`chorus`] does ask, and asks for very little: it has no feedback path, so
/// its tail is exactly the deepest its delay line is ever read from — a
/// fortieth of a second rather than a decay to be estimated.
///
/// A delay timed in beats is measured at the longest beat `tempo` plays, and a
/// reverb's pre-delay is added to its room: both move the ring-out later.
pub(crate) fn tail_seconds(chain: &[Fx], tempo: &dyn Tempo) -> f32 {
    let total: f32 = chain
        .iter()
        .map(|fx| match fx {
            // `ping_pong` is not read: the repeats are the same repeats,
            // decaying at the same rate, and only the side they land on
            // differs — so the tail is the same length either way.
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

/// Fades the last [`FADE`] of `buf` to silence when `chain` rang out into it.
///
/// Called on a buffer [`tail_seconds`] grew, after the chain ran: the end of
/// that buffer is where a ring-out was cut. A chain with no tail leaves the
/// buffer alone, since its end is the signal's own.
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

    /// A chorus asks for the length of its own deepest read and nothing more:
    /// a fortieth of a second against a room's seconds. Both halves of the
    /// `mix` guard are here — a dry one is in the test above, a wet one is
    /// this, and the number moves with `depth` because the delay does.
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

    /// The other end of the same question: only the two stages that walk a
    /// gain track backwards ask to have seen anything, and a chain's asks add
    /// because it runs in series.
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
