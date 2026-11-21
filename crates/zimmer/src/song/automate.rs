//! A value that moves across the piece.
//!
//! Everything else in a recipe is one number for the whole piece: a track's
//! `gain`, a filter's `cutoff`. Envelopes and the LFO move things *within a
//! note*, which is a different scale entirely — so a **build**, the filter
//! opening and the level rising over eight bars into a drop, was unwritable,
//! and so were a riser, a fade-in on one instrument and a lead getting
//! brighter as the piece intensifies. What could be written was a section
//! louder than the one before it, which is a step, not a build.
//!
//! ## The same shape as a keyframe, in beats
//!
//! `scorsese-core` animates a clip with `(property, [(t, value, easing)])`,
//! and this is that shape one crate over: a list of points, a value between
//! them, and core's five plain easing curves under the same five names. The
//! two cannot share code — this crate has never heard of `core` — but they
//! must not disagree about what `ease_in_out` means, so they do not. Core's
//! overshooting curves (`back_*`, `spring`, `cubic_bezier`) are not here: they
//! exist to make motion land, and a gain or a cutoff that went past its mark
//! and settled back would be a pump, not a build.
//!
//! **Time is beats**, like everything else in a song, so changing the tempo of
//! a piece with a build in it is still one number.
//!
//! ## Which properties, and why it is a closed list
//!
//! `core`'s property path is a string, deliberately: a path nothing animates
//! is ignored and warned about, because a project written against a newer
//! build still has to render on an older one. Nothing in this crate works that
//! way. A song is refused for a typo'd track or pattern name rather than
//! rendered with a hole in it, because *silence in the middle of a piece* is
//! the failure that costs an agent a whole iteration to even notice — and an
//! automation curve that moves nothing is exactly that failure, quieter.
//!
//! So [`Param`] is a closed enum. A misspelled parameter is refused by serde,
//! against a list of the words that work, before anything is rendered. A curve
//! on a track that does not exist, or a `cutoff` curve on an instrument with
//! no filter, is refused too — see the song's validation.
//!
//! ## Where the list closes, which is the mixing desk
//!
//! Closure is one decision and *where* it closes is another, so: all four
//! params are properties of a [`Track`] — level, position, brightness and how
//! much of it reaches the room, all at the fader — and that is the boundary,
//! chosen rather than merely inherited from what a track happens to hold.
//! **Automation moves the mix; what an instrument *is* belongs to its envelope
//! and its LFO**, which are per note because a note is where an instrument's
//! identity lives.
//!
//! The two gestures that fall on the far side of that line are an effect's
//! `mix` opening through a closing bar and a source's own depth — an
//! [`fm2`](crate::patch::Source) index brightening through a build. The first
//! is mostly served from the fader side now: a [`Param::Send`] curve washes a
//! part out into the song's room without reaching inside a chain. Neither is
//! out because it is hard to render; they are out because reaching them
//! means addressing a thing *inside* a track, and a curve names a track
//! precisely because a track has exactly one gain, one pan, one send and one
//! filter. A chain is an ordered list with no names in it and a source's
//! fields are per kind, so the alternative is a naming scheme with a long
//! tail, bought for a want that a fader has so far always covered. If it comes
//! back with a concrete piece that the fader cannot serve, it reopens with that
//! case in hand — which is a stronger argument than this paragraph is.
//!
//! ## Where it is sampled, which is not one answer
//!
//! A curve is sampled **where the thing it moves is decided**, and the
//! parameters this touches decide at different rates:
//!
//! - [`Param::Gain`], [`Param::Pan`] and [`Param::Send`] are faders. A fader
//!   is a property of the mix at every instant, so they are applied **per sample**, on the
//!   track's own bus, after its chain. That costs one multiply per sample per
//!   automated track and it is *correct*: a pad holding one chord through the
//!   whole build actually swells, which a per-note answer could not do. A
//!   send is read the same way, on the part's way into the room's sum.
//! - [`Param::Cutoff`] is a voice parameter, so it is read **inside each
//!   note**: the note's frames are turned back into beats and the curve is
//!   read across all of them, its release and ring-out included (#1123). It
//!   used to be read once at the onset and held, on the argument that the
//!   difference was audible only inside one note. That is exactly where a
//!   build lives: a pad held through eight bars got one value, and a riser
//!   had to be written as noise re-struck every 32nd note to sound like one,
//!   which stuttered.
//!
//! ## Under `fit`
//!
//! Beats are counted from the start of the **rendered piece**, once, and a
//! curve never repeats.
//!
//! - `stretch` moves the tempo and leaves the beats alone, so the build
//!   stretches with the music and stays a build over the same eight bars.
//! - `loop` repeats the *arrangement*, and the curve does not go back with it:
//!   a build that restarted every pass would be a saw. A piece looping to
//!   45 seconds can carry a curve across all of it, because the beats keep
//!   counting — which is the general answer, and the reason nothing has to be
//!   refused here.
//! - `once` plays through and pads with silence; the curve simply ends with
//!   the music.

use serde::{Deserialize, Serialize};

use super::clock::Clock;
use super::{Song, Track};
use crate::stereo::{self, Stereo};

/// What a curve moves.
///
/// Closed and small, for the reason the module doc gives: a wrong word is a
/// refusal here rather than a curve that quietly moves nothing. Each is a
/// number that was already a field of the document and was already constant
/// for the whole piece.
///
/// **An fx parameter is deliberately not here.** Every effect has different
/// fields, so naming one is a path into a chain rather than a word — a second
/// design — and a chain runs once over a whole bus, so honouring it would mean
/// a time-varying parameter inside every effect. The module doc has the rest of
/// that argument, and the line it draws: the mix moves, the instrument does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Param {
    /// The track's linear [`gain`](Track::gain) — the fader. A part rising
    /// into a chorus, a riser, a fade-in on one instrument.
    Gain,
    /// The track's [`pan`](Track::pan) — a part drifting across the image.
    /// Clamped to `-1.0..=1.0` at every instant, as a written `pan` is.
    Pan,
    /// The cutoff of the instrument's filter, in Hz — the build. Refused on a
    /// track whose patch has no filter, since there would be nothing to move.
    Cutoff,
    /// The track's [`send`](Track::send) — how much of it reaches the song's
    /// room. A part washing out into the reverb before a drop, a throw into
    /// the delay on one hit. Clamped to `0.0..=1.0` at every instant, as a
    /// written `send` is, and refused on a song whose chain has no `reverb` or
    /// `delay` for it to reach.
    Send,
}

impl Param {
    /// The word a recipe spells it with — what a refusal names it by.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gain => "gain",
            Self::Pan => "pan",
            Self::Cutoff => "cutoff",
            Self::Send => "send",
        }
    }
}

/// How a value approaches the next point.
///
/// Core's five plain curves, under the same names and with the same
/// arithmetic, as `scorsese_core::keyframe::Easing`. Written out again rather
/// than shared, because this crate does not depend on that one — but *chosen*
/// rather than invented, so an agent that has written a clip's keyframes
/// already knows these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    /// Constant rate throughout, and what a point that says nothing means.
    #[default]
    Linear,
    /// Starts slow and accelerates into the next point.
    EaseIn,
    /// Starts at full rate and settles as it arrives.
    EaseOut,
    /// Slow at both ends, quickest in the middle — the one that reads as
    /// deliberate rather than mechanical.
    EaseInOut,
    /// Holds this value until the next point, then jumps. A step, written on
    /// purpose.
    Hold,
}

impl Easing {
    /// Reshapes linear progress through a segment, `0.0..=1.0`, into eased
    /// progress.
    pub fn apply(self, progress: f32) -> f32 {
        let p = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => p,
            Self::EaseIn => p * p,
            Self::EaseOut => 1.0 - (1.0 - p) * (1.0 - p),
            // Smoothstep: symmetric, and flat at both ends.
            Self::EaseInOut => p * p * (3.0 - 2.0 * p),
            // The value does not travel at all; it jumps at the next point.
            Self::Hold => 0.0,
        }
    }
}

/// One control point: a beat, what the value reads there, and how it travels
/// from there to the next.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// Beats from the start of the rendered piece. Not from the start of a
    /// pattern and not from the start of a `loop` pass — see the module doc.
    pub beat: f32,
    /// What the parameter reads at this instant, in the parameter's own units:
    /// linear for `gain`, `-1.0..=1.0` for `pan`, Hz for `cutoff`, `0.0..=1.0`
    /// for `send`.
    pub value: f32,
    /// How the value travels from here to the next point.
    #[serde(default, skip_serializing_if = "is_linear")]
    pub easing: Easing,
}

/// Whether a point travels at a constant rate — the test that keeps
/// `"easing": "linear"` out of every saved document, for the reason a song's
/// `swing` is left out when it is zero: a serialiser that started writing a
/// default would invalidate every cached bake for no change in the audio.
fn is_linear(easing: &Easing) -> bool {
    *easing == Easing::Linear
}

/// One parameter of one track, moving across the piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    /// The [`Track::name`] this rides. It is the *arrangement* that builds, so
    /// this lives on the song and names a track — a patch shared between two
    /// songs must not carry one song's build.
    pub track: String,
    /// Which parameter moves.
    pub param: Param,
    /// The control points, in ascending beat order.
    pub points: Vec<Point>,
}

impl Automation {
    /// What this parameter reads at `beat`.
    ///
    /// `None` only when there are no points at all, which validation refuses —
    /// a curve with nothing to say leaves whatever reads it on the number the
    /// document already wrote, rather than asserting a zero nobody meant.
    ///
    /// Outside the written span the value **holds**: before the first point it
    /// is the first value, after the last it is the last. Extrapolating would
    /// make a two-point build keep climbing forever, which is nobody's reading
    /// of two points.
    pub fn value_at(&self, beat: f32) -> Option<f32> {
        let first = self.points.first()?;
        let last = self.points.last()?;
        if beat <= first.beat {
            return Some(first.value);
        }
        if beat >= last.beat {
            return Some(last.value);
        }
        let pair = self
            .points
            .windows(2)
            .find(|pair| pair[0].beat <= beat && beat < pair[1].beat)?;
        let (from, to) = (&pair[0], &pair[1]);
        let span = to.beat - from.beat;
        if span <= 0.0 {
            return Some(from.value);
        }
        let progress = from.easing.apply((beat - from.beat) / span);
        Some(from.value + (to.value - from.value) * progress)
    }

    /// True when the beats ascend strictly. Checked by validation; everything
    /// that evaluates a curve is entitled to assume it.
    pub(super) fn is_sorted(&self) -> bool {
        self.points
            .windows(2)
            .all(|pair| pair[0].beat < pair[1].beat)
    }
}

/// The curves riding one track, looked up once instead of per note.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Riding<'a> {
    /// The fader's level, if it moves.
    pub(super) gain: Option<&'a Automation>,
    /// The fader's position, if it moves.
    pub(super) pan: Option<&'a Automation>,
    /// The instrument's filter cutoff, if it moves.
    pub(super) cutoff: Option<&'a Automation>,
    /// How much of the part reaches the room, if it moves.
    pub(super) send: Option<&'a Automation>,
}

impl Riding<'_> {
    /// Whether anything about this track's placement in the mix moves, which
    /// is what decides whether it needs a bus of its own to ride.
    pub(super) fn moves_the_fader(&self) -> bool {
        self.gain.is_some() || self.pan.is_some()
    }

    /// Whether this track needs a bus of its own to ride: a moving fader, or
    /// a moving send, which has to be read off the part sample by sample on
    /// its way into the room.
    pub(super) fn needs_a_bus(&self) -> bool {
        self.moves_the_fader() || self.send.is_some()
    }
}

/// One entry per track of `song`, in track order.
///
/// Built once per render: the note loop and the fold-down both ask about a
/// track many thousands of times, and neither should be scanning a list of
/// curves to do it.
pub(super) fn riding(song: &Song) -> Vec<Riding<'_>> {
    song.tracks
        .iter()
        .map(|track| {
            let mut riding = Riding::default();
            for curve in song.automation.iter().filter(|it| it.track == track.name) {
                let slot = match curve.param {
                    Param::Gain => &mut riding.gain,
                    Param::Pan => &mut riding.pan,
                    Param::Cutoff => &mut riding.cutoff,
                    Param::Send => &mut riding.send,
                };
                *slot = Some(curve);
            }
            riding
        })
        .collect()
}

/// Applies a track's moving fader to its own bus, sample by sample.
///
/// The written `gain` and `pan` are what a curve that does not exist leaves
/// behind, so a track that automates only its level keeps the position it was
/// written at, and the other way round.
///
/// `clock` is the one the piece is actually rendered at, which is the
/// stretched one under a `stretch` fit and carries the song's tempo map — that
/// is what makes a build stretch with the music, and speed up with it, rather
/// than land somewhere else in it.
pub(super) fn ride(bus: &mut Stereo, track: &Track, riding: Riding<'_>, clock: &Clock) {
    let fixed = stereo::pan_gains(track.pan);
    for frame in 0..bus.frames() {
        let beat = clock.beat_at(frame);
        let level = riding
            .gain
            .and_then(|curve| curve.value_at(beat))
            .unwrap_or(track.gain);
        let (left, right) = match riding.pan {
            Some(curve) => stereo::pan_gains(curve.value_at(beat).unwrap_or(track.pan)),
            None => fixed,
        };
        bus.l[frame] *= level * left;
        bus.r[frame] *= level * right;
    }
}

/// Adds a track's part into the room's sum at a moving send, sample by sample.
///
/// `placed` is what the part still needs on the way in: its static fader and
/// pan, or unity when [`ride`] has already written a moving one onto it. The
/// curve is clamped at every instant, as the written `send` is.
pub(super) fn send_into(
    sent: &mut Stereo,
    part: &Stereo,
    (left, right): (f32, f32),
    curve: &Automation,
    written: f32,
    clock: &Clock,
) {
    sent.grow_to(part.frames());
    for frame in 0..part.frames() {
        let send = curve
            .value_at(clock.beat_at(frame))
            .unwrap_or(written)
            .clamp(0.0, 1.0);
        sent.l[frame] += part.l[frame] * (left * send);
        sent.r[frame] += part.r[frame] * (right * send);
    }
}

/// Which curve puts a track on a bus of its own.
///
/// A mix asserts what it sounds like, and that is exactly the wrong instrument
/// for this: whether a track is bussed changes the *rounding* of a sum and
/// nothing else, so a predicate stuck at "yes" sounds identical to one that
/// works. It is asked here, by the number.
#[cfg(test)]
mod tests {
    use super::*;

    /// A curve of one point on `param` — enough to occupy a slot.
    fn parked(param: Param) -> Automation {
        Automation {
            track: "pad".to_owned(),
            param,
            points: vec![Point {
                beat: 0.0,
                value: 1.0,
                easing: Easing::Linear,
            }],
        }
    }

    /// The faders, and only the faders. A `cutoff` is read inside each note
    /// and changes nothing about how the part is summed, so a track that
    /// only sweeps its filter keeps the straight-to-master path a track with no
    /// curves at all takes — which is the path the bit-identical promise rests
    /// on.
    #[test]
    fn a_fader_puts_a_track_on_a_bus_and_a_sweep_does_not() {
        let (gain, pan, cutoff, send) = (
            parked(Param::Gain),
            parked(Param::Pan),
            parked(Param::Cutoff),
            parked(Param::Send),
        );
        let sending = Riding {
            send: Some(&send),
            ..Riding::default()
        };
        assert!(sending.needs_a_bus(), "a moving send is read off the part");
        assert!(!sending.moves_the_fader(), "but it leaves the level alone");
        for (moves, riding) in [
            (false, Riding::default()),
            (
                true,
                Riding {
                    gain: Some(&gain),
                    ..Riding::default()
                },
            ),
            (
                true,
                Riding {
                    pan: Some(&pan),
                    ..Riding::default()
                },
            ),
            (
                false,
                Riding {
                    cutoff: Some(&cutoff),
                    ..Riding::default()
                },
            ),
        ] {
            assert_eq!(riding.moves_the_fader(), moves, "{riding:?}");
            assert_eq!(riding.needs_a_bus(), moves, "{riding:?}");
        }
    }
}
