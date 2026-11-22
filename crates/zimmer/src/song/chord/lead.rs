use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::Note;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Leading {
    Near,
}

#[derive(Debug, Default)]
pub(crate) struct Led(BTreeMap<String, Vec<f32>>);

impl Led {
    pub(crate) fn last(&self, track: &str) -> Option<&[f32]> {
        self.0.get(track).map(Vec::as_slice)
    }

    pub(crate) fn record(&mut self, track: &str, voices: &[Note]) {
        let mut midi: Vec<f32> = voices
            .iter()
            .filter_map(|voice| voice.note.to_midi().ok())
            .collect();
        midi.sort_by(f32::total_cmp);
        self.0.insert(track.to_owned(), midi);
    }
}

pub(crate) struct Stack {
    pub(crate) bass: Option<i32>,
    pub(crate) upper: Vec<i32>,
}

impl Stack {
    /// Callers keep `|turns|` below `upper.len()`; beyond `upper.len()` the rotation panics.
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

    pub(crate) fn nearest(
        &self,
        previous: &[f32],
        range: &std::ops::RangeInclusive<i32>,
    ) -> Vec<i32> {
        // Within an octave of root position, so `near` chords cannot drift along the keyboard.
        let span = self.upper.len() as i32 - 1;
        // Nearest root position first, then upward: the strict `<` below keeps the first of a tie.
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

    fn stack(upper: &[i32]) -> Stack {
        Stack {
            bass: None,
            upper: upper.to_vec(),
        }
    }

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

    #[test]
    fn near_keeps_the_common_tones() {
        let bm7 = stack(&[59, 62, 66, 69]);
        let dmaj7 = [50.0, 54.0, 57.0, 61.0];
        assert_eq!(bm7.nearest(&dmaj7, &KEYS), vec![50, 54, 57, 59]);
    }

    #[test]
    fn near_breaks_ties_and_respects_its_limits() {
        // Root position and its downward rotation are both six semitones away: the one nearer root
        // position wins.
        let fifth = stack(&[60, 67]);
        assert_eq!(fifth.nearest(&[57.5, 63.5], &KEYS), vec![60, 67]);
        // Leading toward a chord far below takes the lowest rotation on offer, an octave down and
        // no further — unless the bass sits in the way.
        let c = stack(&[60, 64, 67]);
        let low = [40.0, 44.0, 47.0];
        assert_eq!(c.nearest(&low, &KEYS), vec![52, 55, 60]);
        let over_g = Stack {
            bass: Some(55),
            upper: vec![60, 64, 67],
        };
        assert_eq!(over_g.nearest(&low, &KEYS), vec![55, 60, 64, 67]);
        let over_f = Stack {
            bass: Some(53),
            upper: vec![60, 64, 67],
        };
        assert_eq!(over_f.nearest(&low, &KEYS), vec![53, 55, 60, 64]);
        assert_eq!(c.nearest(&[72.0, 76.0, 79.0], &(0..=70)), vec![60, 64, 67]);
    }

    #[test]
    fn chords_of_different_sizes_still_lead() {
        let c = stack(&[48, 52, 55]);
        assert_eq!(
            c.nearest(&[50.0, 54.0, 57.0, 61.0], &KEYS),
            vec![52, 55, 60]
        );
        assert_eq!(distance(&[60], &[60.0, 72.0]), 12.0);
    }
}
