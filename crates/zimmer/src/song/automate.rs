//! A value that moves across the piece.

use serde::{Deserialize, Serialize};

use super::clock::Clock;
use super::{Song, Track};
use crate::stereo::{self, Stereo};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Param {
    Gain,
    Pan,
    Cutoff,
    Send,
}

impl Param {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gain => "gain",
            Self::Pan => "pan",
            Self::Cutoff => "cutoff",
            Self::Send => "send",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Hold,
}

impl Easing {
    pub fn apply(self, progress: f32) -> f32 {
        let p = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => p,
            Self::EaseIn => p * p,
            Self::EaseOut => 1.0 - (1.0 - p) * (1.0 - p),
            Self::EaseInOut => p * p * (3.0 - 2.0 * p),
            Self::Hold => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub beat: f32,
    pub value: f32,
    #[serde(default, skip_serializing_if = "is_linear")]
    pub easing: Easing,
}

fn is_linear(easing: &Easing) -> bool {
    *easing == Easing::Linear
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub track: String,
    pub param: Param,
    pub points: Vec<Point>,
}

impl Automation {
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

    pub(super) fn is_sorted(&self) -> bool {
        self.points
            .windows(2)
            .all(|pair| pair[0].beat < pair[1].beat)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Riding<'a> {
    pub(super) gain: Option<&'a Automation>,
    pub(super) pan: Option<&'a Automation>,
    pub(super) cutoff: Option<&'a Automation>,
    pub(super) send: Option<&'a Automation>,
}

impl Riding<'_> {
    pub(super) fn moves_the_fader(&self) -> bool {
        self.gain.is_some() || self.pan.is_some()
    }

    pub(super) fn needs_a_bus(&self) -> bool {
        self.moves_the_fader() || self.send.is_some()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
