//! A song's tempo map as MIDI tempo events — and a ramp, which MIDI has no
//! event for, as a staircase of them.
//!
//! A jump is exactly a tempo event: this tempo from this tick. A ramp is not,
//! so it is written as **one step every sixteenth note** ([`STEP_BEATS`]), and
//! each step is not the tempo at either end of it but the **logarithmic mean**
//! of the two: `(b − a) / ln(b / a)`. That is the one constant tempo that
//! takes exactly as long to play the sixteenth as the ramp does — the ramp's
//! seconds over a stretch are `60/s · ln(b/a)`, which the step divides out —
//! so **every sixteenth lands where the recipe puts it**, and the only error
//! is inside one step, where a note between two sixteenths is placed as
//! though the tempo held still for a quarter of a beat.
//!
//! How large that is: at most `h²/8 · 60·s/B²` seconds, for a step of `h`
//! beats in a ramp gaining `s` bpm per beat at tempo `B`. A ritardando from
//! 120 to 80 over two bars is under half a millisecond; an accelerando from 60
//! to 180 in one bar — steeper than music is written — about four. Both are
//! under the ten or so milliseconds a listener hears as a note being early.
//! A finer step would spend tempo events for accuracy nobody can hear, and a
//! DAW draws every one of them in its tempo lane.
//!
//! Tempos are rounded to MIDI's whole microseconds per beat, which is under a
//! thousandth of a bpm — what the importer rounds to on the way back in.

use std::collections::BTreeMap;

use super::TICKS_PER_BEAT;
use crate::song::{Song, TempoChange};

/// How far apart a ramp's steps are, in beats: a sixteenth note.
const STEP_BEATS: f64 = 0.25;

/// A song's tempo as the conductor track will hold it.
#[derive(Debug)]
pub(super) struct Tempos {
    /// `(tick, microseconds per beat)`, ascending, the first on tick zero, no
    /// event restating the tempo before it.
    pub(super) events: Vec<(u64, u32)>,
    /// How many ramps were written as steps.
    ramps: usize,
}

/// The tempo events for `song`'s `bpm` and `tempo` map.
pub(super) fn events(song: &Song) -> Tempos {
    let ppq = f64::from(TICKS_PER_BEAT);
    // Keyed by tick so a step starting where a jump lands replaces it: the
    // later write is the tempo from that tick on.
    let mut at: BTreeMap<u64, u32> = BTreeMap::new();
    at.insert(0, micros(f64::from(song.bpm)));
    let (mut from, mut tempo) = (0.0_f64, f64::from(song.bpm));
    let mut ramps = 0;
    for &TempoChange { beat, bpm, ramp } in &song.tempo {
        let (beat, bpm) = (f64::from(beat), f64::from(bpm));
        if ramp {
            ramps += 1;
            let tempo_at = |x: f64| tempo + (bpm - tempo) * (x - from) / (beat - from);
            let mut x = from;
            while x < beat {
                let y = (x + STEP_BEATS).min(beat);
                let step = log_mean(tempo_at(x), tempo_at(y));
                at.insert((x * ppq).round() as u64, micros(step));
                x = y;
            }
        }
        at.insert((beat * ppq).round() as u64, micros(bpm));
        (from, tempo) = (beat, bpm);
    }
    let mut events: Vec<(u64, u32)> = Vec::with_capacity(at.len());
    for (tick, micros) in at {
        if events.last().is_none_or(|&(_, before)| before != micros) {
            events.push((tick, micros));
        }
    }
    Tempos { events, ramps }
}

impl Tempos {
    /// The sentence saying ramps were stepped, if any were.
    pub(super) fn sentences(&self) -> Vec<String> {
        if self.ramps == 0 {
            return Vec::new();
        }
        vec![format!(
            "the smoothness of {} tempo ramp(s): MIDI has only jumps, so each is a step \
             every sixteenth note, timed so every sixteenth lands exactly where the ramp \
             puts it",
            self.ramps
        )]
    }
}

/// The logarithmic mean of two tempos: the constant tempo that plays a
/// stretch in the time a ramp between them does.
fn log_mean(a: f64, b: f64) -> f64 {
    if (a - b).abs() < 1e-9 {
        a
    } else {
        (b - a) / (b / a).ln()
    }
}

/// A tempo as a MIDI tempo event holds it: whole microseconds per beat,
/// within the three bytes the event has.
fn micros(bpm: f64) -> u32 {
    (60_000_000.0 / bpm)
        .round()
        .clamp(1.0, f64::from(0xFF_FFFF_u32)) as u32
}
