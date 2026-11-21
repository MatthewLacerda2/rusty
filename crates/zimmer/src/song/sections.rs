//! Where a song's sections fall in the piece it renders to.

use super::shape::plan;
use super::Song;
use crate::level::Cut;

fn whole(song: &Song) -> Vec<Cut> {
    let (clock, passes) = plan(song);
    if clock.bpm() <= 0.0 {
        return Vec::new();
    }
    let mut beats = 0.0f32;
    let mut cuts = Vec::new();
    for entry in song
        .arrangement
        .iter()
        .cycle()
        .take(song.arrangement.len() * passes as usize)
    {
        let Some(slot) = song.slot_beats(entry) else {
            continue;
        };
        beats += slot;
        cuts.push(Cut {
            label: entry.label(),
            end_seconds: f64::from(clock.seconds(beats)),
        });
    }
    cuts
}

impl Song {
    pub fn sections(&self) -> Vec<Cut> {
        whole(self)
    }
}

pub(crate) fn of(song: &Song, start_seconds: f64) -> Vec<Cut> {
    whole(song)
        .into_iter()
        .map(|cut| Cut {
            end_seconds: cut.end_seconds - start_seconds,
            ..cut
        })
        .filter(|cut| cut.end_seconds > 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Pattern, Song};
    use std::collections::BTreeMap;

    fn two_sections() -> Song {
        let mut patterns = BTreeMap::new();
        for name in ["verse", "chorus"] {
            patterns.insert(
                name.to_owned(),
                Pattern {
                    beats: 4.0,
                    notes: Vec::new(),
                },
            );
        }
        Song {
            bpm: 120.0,
            tempo: vec![],
            seed: 0,
            key: None,
            tracks: Vec::new(),
            patterns,
            arrangement: vec!["verse".into(), "chorus".into()],
            swing: 0.0,
            humanize: None,
            fx: Vec::new(),
            automation: Vec::new(),
            fit: None,
            anchors: Vec::new(),
            fade: None,
            tail: None,
        }
    }

    fn ends(cuts: &[Cut]) -> Vec<(String, f64)> {
        cuts.iter()
            .map(|cut| (cut.label.clone(), cut.end_seconds))
            .collect()
    }

    #[test]
    fn a_whole_piece_is_reported_from_its_own_start() {
        assert_eq!(
            ends(&of(&two_sections(), 0.0)),
            vec![("verse".to_owned(), 2.0), ("chorus".to_owned(), 4.0)]
        );
    }

    #[test]
    fn a_window_moves_every_boundary_back_by_where_it_opened() {
        assert_eq!(
            ends(&of(&two_sections(), 1.5)),
            vec![("verse".to_owned(), 0.5), ("chorus".to_owned(), 2.5)]
        );
    }

    #[test]
    fn a_section_that_has_already_ended_gets_no_row() {
        assert_eq!(
            ends(&of(&two_sections(), 2.0)),
            vec![("chorus".to_owned(), 2.0)]
        );
    }
}
