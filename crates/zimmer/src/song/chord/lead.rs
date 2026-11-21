//! A named chord voiced other than in root position: `"inv": 1`, or
//! `"voicing": "near"`.
//!
//! Root position is the default and stays it — the parent module has why. But
//! a progression of root-position blocks moves every voice in parallel, and
//! `Dmaj7` to `Bm7` at one octave jumps all four of them a sixth. A player
//! would keep the three notes the chords share and move one. These are the two
//! ways a document says so.
//!
//! - **`inv`** rotates the chord: its lowest voice up an octave, that many
//!   times. `C` with `inv: 1` is `E G C`, with `inv: 2` is `G C E`. A
//!   slash bass stays where it is and only the voices above it turn. An
//!   inversion past the last one is root position an octave up, which is
//!   `oct`'s job, so it is refused rather than wrapped.
//! - **`near`** picks, among the chord's inversions in the octave above and
//!   below root position, the one whose voices move least from the previous
//!   chord **on the same track in the same pattern**, in document order — the
//!   oldest voice-leading rule there is. The first chord of a pattern has
//!   nothing to lead from and is voiced in root position.
//!
//! **Why only inside one pattern.** A pattern is a block the arrangement
//! places, repeats and transposes; if its first chord were voiced from
//! whatever the previous block ended on, the same pattern would sound
//! different each time it was placed, and nothing on the page would say so.
//! Inside one pattern the previous chord is on the same page, a few lines up.
//!
//! **What "least" means.** Two chords of the same size are compared voice by
//! voice, low to high, and the total distance is the sum — the textbook
//! measure. Chords of different sizes have no such pairing, so each voice is
//! measured to its nearest neighbour in the other chord, both ways round. A
//! tie goes to the inversion nearest root position, and between two equally
//! near the upward one, so the answer never depends on iteration order.
//!
//! **What `oct` still means.** It places the root-position chord, which must
//! be on the keyboard whatever is chosen; `near` only looks an octave either
//! side of it, so a progression cannot drift up or down the keyboard chord by
//! chord, and the register stays the one the writer chose.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::Note;

/// How a named chord is voiced from the chord before it.
///
/// One word today, and an enum rather than a flag so a second rule — a fixed
/// top voice, say — is a new word rather than a second field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Leading {
    /// The inversion, within an octave of root position, whose voices move
    /// least from the previous chord on the same track in the same pattern.
    Near,
}

/// What each track's most recent chord sounded, as a pattern is expanded — the
/// one fact [`Leading::Near`] reads.
///
/// Built fresh for every pattern, which is how leading stops at a pattern's
/// edge (see the module doc). Every chord records itself, named or spelled,
/// stated or arpeggiated, so a spelled voicing can be the one a run of named
/// chords leads from.
#[derive(Debug, Default)]
pub(crate) struct Led(BTreeMap<String, Vec<f32>>);

impl Led {
    /// The voices of the last chord on `track`, low to high.
    pub(crate) fn last(&self, track: &str) -> Option<&[f32]> {
        self.0.get(track).map(Vec::as_slice)
    }

    /// Remembers `voices` as the last chord on `track`.
    pub(crate) fn record(&mut self, track: &str, voices: &[Note]) {
        let mut midi: Vec<f32> = voices
            .iter()
            .filter_map(|voice| voice.note.to_midi().ok())
            .collect();
        midi.sort_by(f32::total_cmp);
        self.0.insert(track.to_owned(), midi);
    }
}

/// A chord split into what turns and what does not: the slash bass, if it has
/// one, and the voices above it, low to high, as MIDI numbers.
pub(crate) struct Stack {
    /// The slash bass, which no inversion moves.
    pub(crate) bass: Option<i32>,
    /// Everything above the bass, ascending.
    pub(crate) upper: Vec<i32>,
}

impl Stack {
    /// The chord with its upper voices rotated `turns` times: up an octave from
    /// the bottom for a positive count, down an octave from the top for a
    /// negative one. Low to high, the bass first.
    ///
    /// Fewer turns than there are upper voices, either way — the callers keep
    /// to that, and one more would be the same chord an octave away.
    pub(crate) fn turned(&self, turns: i32) -> Vec<i32> {
        let mut upper = self.upper.clone();
        let (up, down) = (turns.max(0) as usize, (-turns).max(0) as usize);
        let count = upper.len();
        upper.rotate_left(up);
        upper[count - up..]
            .iter_mut()
            .for_each(|voice| *voice += 12);
        upper.rotate_right(down);
        upper[..down].iter_mut().for_each(|voice| *voice -= 12);
        self.bass.into_iter().chain(upper).collect()
    }

    /// The voicing [`Leading::Near`] chooses after `previous`: the rotation,
    /// within one octave of root position, that moves least from it while
    /// staying on the keyboard and above the bass. Root position when nothing
    /// else qualifies, which is the voicing the caller already checked.
    pub(crate) fn nearest(
        &self,
        previous: &[f32],
        range: &std::ops::RangeInclusive<i32>,
    ) -> Vec<i32> {
        let span = self.upper.len() as i32 - 1;
        // 0, 1, -1, 2, -2, …: nearest root position first, upward before
        // downward, so a strict `<` below settles every tie the same way.
        let turns = (0..=span).flat_map(|n| if n == 0 { vec![0] } else { vec![n, -n] });
        let mut best = (f32::INFINITY, self.turned(0));
        for turn in turns {
            let candidate = self.turned(turn);
            let fits = candidate.iter().all(|midi| range.contains(midi))
                && self
                    .bass
                    .is_none_or(|bass| candidate[1..].iter().all(|&voice| voice > bass));
            if !fits {
                continue;
            }
            let moved = distance(&candidate, previous);
            if moved < best.0 {
                best = (moved, candidate);
            }
        }
        best.1
    }
}

/// How far the voices travel from `previous` to `next`, both low to high — see
/// the module doc for the two measures and why there are two.
fn distance(next: &[i32], previous: &[f32]) -> f32 {
    if next.len() == previous.len() {
        return next
            .iter()
            .zip(previous)
            .map(|(&to, from)| (to as f32 - from).abs())
            .sum();
    }
    let nearest = |pitch: f32, among: &mut dyn Iterator<Item = f32>| {
        among
            .map(|other| (other - pitch).abs())
            .fold(f32::INFINITY, f32::min)
    };
    let next: Vec<f32> = next.iter().map(|&midi| midi as f32).collect();
    let forward: f32 = next
        .iter()
        .map(|&to| nearest(to, &mut previous.iter().copied()))
        .sum();
    let backward: f32 = previous
        .iter()
        .map(|&from| nearest(from, &mut next.iter().copied()))
        .sum();
    forward + backward
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: std::ops::RangeInclusive<i32> = 0..=127;

    /// A plain chord, no bass.
    fn stack(upper: &[i32]) -> Stack {
        Stack {
            bass: None,
            upper: upper.to_vec(),
        }
    }

    /// Rotation both ways, and the bass left alone.
    #[test]
    fn turning_rotates_the_upper_voices_only() {
        let c = stack(&[60, 64, 67]);
        assert_eq!(c.turned(0), vec![60, 64, 67]);
        assert_eq!(c.turned(1), vec![64, 67, 72]);
        assert_eq!(c.turned(2), vec![67, 72, 76]);
        assert_eq!(c.turned(-1), vec![55, 60, 64]);
        let over_e = Stack {
            bass: Some(52),
            upper: vec![60, 64, 67],
        };
        assert_eq!(over_e.turned(1), vec![52, 64, 67, 72]);
    }

    /// The issue's own example: Dmaj7 to Bm7 at octave 3 keeps the three
    /// shared notes and moves one voice a whole tone.
    #[test]
    fn near_keeps_the_common_tones() {
        let bm7 = stack(&[59, 62, 66, 69]);
        let dmaj7 = [50.0, 54.0, 57.0, 61.0];
        assert_eq!(bm7.nearest(&dmaj7, &KEYS), vec![50, 54, 57, 59]);
    }

    /// Ties go to root position, then upward; the bass stays lowest and the
    /// keyboard's ends are respected.
    #[test]
    fn near_breaks_ties_and_respects_its_limits() {
        // Root position and its downward rotation are both six semitones
        // away: the one nearer root position wins.
        let fifth = stack(&[60, 67]);
        assert_eq!(fifth.nearest(&[57.5, 63.5], &KEYS), vec![60, 67]);
        // Leading toward a chord far below takes the lowest rotation on offer,
        // an octave down and no further — unless the bass sits in the way.
        let c = stack(&[60, 64, 67]);
        let low = [40.0, 44.0, 47.0];
        assert_eq!(c.nearest(&low, &KEYS), vec![52, 55, 60]);
        let over_g = Stack {
            bass: Some(55),
            upper: vec![60, 64, 67],
        };
        assert_eq!(over_g.nearest(&low, &KEYS), vec![55, 60, 64, 67]);
        // Over an F the lowest rotation would dip under the bass, and the next
        // one down is the answer.
        let over_f = Stack {
            bass: Some(53),
            upper: vec![60, 64, 67],
        };
        assert_eq!(over_f.nearest(&low, &KEYS), vec![53, 55, 60, 64]);
        // Off the top of a narrowed keyboard, the upward rotations are out.
        assert_eq!(c.nearest(&[72.0, 76.0, 79.0], &(0..=70)), vec![60, 64, 67]);
    }

    /// Different sizes are measured nearest-neighbour, both ways round.
    #[test]
    fn chords_of_different_sizes_still_lead() {
        // C E G after D3 F#3 A3 C#4: the rotation sharing the most ground.
        let c = stack(&[48, 52, 55]);
        assert_eq!(
            c.nearest(&[50.0, 54.0, 57.0, 61.0], &KEYS),
            vec![52, 55, 60]
        );
        assert_eq!(distance(&[60], &[60.0, 72.0]), 12.0);
    }
}
