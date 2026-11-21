//! A chord played one voice at a time: `"arp": "up"`.

use serde::{Deserialize, Serialize};

use super::super::steps::GRID_EPSILON;
use super::super::Note;
use super::Chord;
use crate::error::SynthError;

const MAX_FIGURE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arp {
    Up,
    Down,
    UpDown,
}

impl Arp {
    fn figure(self, voices: &[Note]) -> Vec<&Note> {
        match self {
            Self::Up => voices.iter().collect(),
            Self::Down => voices.iter().rev().collect(),
            Self::UpDown => {
                let mut figure: Vec<&Note> = voices.iter().collect();
                let inner = voices.len().saturating_sub(1);
                figure.extend(voices.iter().take(inner).skip(1).rev());
                figure
            }
        }
    }
}

pub(super) fn spread(
    chord: &Chord,
    arp: Arp,
    voices: &[Note],
    out: &mut Vec<Note>,
) -> Result<(), SynthError> {
    let refuse = |why| {
        Err(SynthError::BadArp {
            track: chord.track.clone(),
            start: chord.start,
            why,
        })
    };
    let Some(div) = chord.div else {
        return refuse("an `arp` needs a `div` — how long one step of the figure is, in beats");
    };
    if !(div.is_finite() && div > 0.0) {
        return refuse(
            "`div` is how long one step of the figure lasts, so it is a positive number of beats",
        );
    }
    let gate = chord.gate.unwrap_or(div);
    if !(gate.is_finite() && gate > 0.0) {
        return refuse(
            "`gate` is how long one note of the figure sounds, so it is a positive number of beats",
        );
    }
    let steps = (chord.dur / div + GRID_EPSILON).floor();
    if !steps.is_finite() || steps < 1.0 {
        return refuse(
            "`div` is longer than the chord's `dur`, so the figure has no room to sound a single note",
        );
    }
    if steps > MAX_FIGURE as f32 {
        return refuse(
            "the figure is more than 4096 notes — that is a texture rather than an arpeggio, so raise `div` or shorten the chord",
        );
    }
    let figure = arp.figure(voices);
    out.extend(
        figure
            .iter()
            .cycle()
            .take(steps as usize)
            .enumerate()
            .map(|(step, voice)| Note {
                start: chord.start + step as f32 * div,
                dur: gate,
                ..(*voice).clone()
            }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::Pitch;
    use super::*;

    fn voices(count: usize) -> Vec<Note> {
        (0..count)
            .map(|index| Note {
                track: "keys".to_owned(),
                note: Pitch::Midi(index as f32),
                start: 0.0,
                dur: 4.0,
                vel: 0.8,
                articulation: None,
            })
            .collect()
    }

    fn walked(arp: Arp, count: usize) -> Vec<f32> {
        arp.figure(&voices(count))
            .iter()
            .map(|note| note.note.to_midi().expect("a MIDI pitch"))
            .collect()
    }

    #[test]
    fn each_word_walks_the_voices_it_names() {
        assert_eq!(walked(Arp::Up, 4), vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(walked(Arp::Down, 4), vec![3.0, 2.0, 1.0, 0.0]);
        assert_eq!(walked(Arp::UpDown, 4), vec![0.0, 1.0, 2.0, 3.0, 2.0, 1.0]);
        assert_eq!(walked(Arp::UpDown, 3), vec![0.0, 1.0, 2.0, 1.0]);
        assert_eq!(walked(Arp::UpDown, 2), walked(Arp::Up, 2));
        assert_eq!(walked(Arp::UpDown, 1), vec![0.0]);
        assert!(walked(Arp::UpDown, 0).is_empty());
    }

    #[test]
    fn the_figure_cap_is_the_one_the_refusal_states() {
        assert_eq!(MAX_FIGURE, 4096, "the refusal says 4096 out loud");
    }
}
