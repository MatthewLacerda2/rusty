//! Where a song's sections fall in the piece it renders to.
//!
//! The arrangement already says this — it is a list of patterns, and every
//! pattern knows how many beats its slot is. All that is left is to turn beats
//! into seconds, which is the one conversion a song defers to render time.
//!
//! It matters that the rows of a report are *these* boundaries rather than an
//! arbitrary grid: "the second chorus is the quiet one" is a finding an author
//! can act on, and "seconds 24 to 32 are quiet" is a fact they then have to
//! look up. A fixed interval is the fallback for a signal whose document does
//! not say — see [`crate::level::profile`].

use super::Song;
use super::shape::plan;
use crate::level::Cut;

/// Every arrangement entry's end, in seconds of the rendered piece.
///
/// One per entry, a layered one included: a row is a stretch of the piece, and
/// a solo over a groove is one stretch, labelled with both.
///
/// Taken from the *planned* tempo and pass count rather than from the written
/// ones, because `fit` can change both: a song stretched to land on a cut plays
/// at a tempo the document never states, and one looped to fill a bed plays its
/// arrangement several times over. Reporting the written positions would put
/// every row in the wrong place for exactly the songs whose length someone
/// cared about.
///
/// The last boundary is where the arrangement ends, not where the audio does —
/// a piece rings out past its final beat, and that ring-out is left to the
/// profiler's fixed-interval fallback rather than being folded into the last
/// pattern, which did not play it.
fn whole(song: &Song) -> Vec<Cut> {
    let (clock, passes) = plan(song);
    if clock.bpm() <= 0.0 {
        return Vec::new();
    }
    // Beats are summed and converted, rather than seconds summed, because that
    // is how the renderer walks the arrangement: a boundary is then the exact
    // number a note written on that downbeat was placed at — tempo map and
    // all, since the clock carries it.
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
    /// Where each entry of the arrangement ends, in seconds of the piece this
    /// song renders to — worked out from the document, without rendering it.
    ///
    /// The same boundaries a bake's section rows are cut at, and through the
    /// same conversion the notes are placed with, so a clip put on one of
    /// them lands on the music. Public for the caller that has a bake already
    /// on disk and still wants to know where its sections are: those are
    /// arithmetic on the document rather than a measurement, so answering
    /// costs nothing and needs no render.
    ///
    /// One per arrangement entry per pass, in order; each section starts where
    /// the one before it ends, and the first at zero. The ring-out past the
    /// last one is not a section — nothing in the arrangement played it.
    pub fn sections(&self) -> Vec<Cut> {
        whole(self)
    }
}

/// The same boundaries, measured from `start_seconds` into the piece — which
/// is where an excerpt's rows begin.
///
/// A section that has already finished by then is dropped rather than kept at
/// a negative end: it is not in what was rendered, and a row for it would send
/// its reader to look for music that is not in the file. The one it lands in
/// the middle of keeps its name and its own end, so a window opening halfway
/// through a chorus is reported as being in that chorus. Nothing clips the far
/// end, because nothing has to — the profiler already runs out of boundaries
/// before the buffer does, which is how a ring-out is reported.
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

    /// Two named blocks, four beats each, at a tempo where a beat is half a
    /// second — so a boundary is a round number and a shift is readable.
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

    /// A window opening inside the verse keeps the verse — with the time that
    /// is left of it — and every later boundary moves with it.
    #[test]
    fn a_window_moves_every_boundary_back_by_where_it_opened() {
        assert_eq!(
            ends(&of(&two_sections(), 1.5)),
            vec![("verse".to_owned(), 0.5), ("chorus".to_owned(), 2.5)]
        );
    }

    /// A window opening exactly on a boundary starts in the section *after*
    /// it. A row for the one that has just finished would be a stretch of no
    /// length, sending its reader to look for music the file does not carry.
    #[test]
    fn a_section_that_has_already_ended_gets_no_row() {
        assert_eq!(
            ends(&of(&two_sections(), 2.0)),
            vec![("chorus".to_owned(), 2.0)]
        );
    }
}
