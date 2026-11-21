//! How much of a piece a track is actually sounding for.
//!
//! The survey's other numbers are read straight off the page. This one is the
//! only arithmetic it does, and it exists because **written gain is a per-note
//! peak and says nothing about how much sound there is.** Percussion is written
//! loud precisely because it is short: a hat at `0.60` firing for a fifth of
//! each beat and a pad at `0.50` held throughout are not close, and ranking
//! them by gain alone crowns the hat.
//!
//! What is computed is the **union** of the intervals a track sounds over, not
//! the sum of them. A three-note chord held for a bar is one bar of sound, and
//! summing would call it three — which would hand the crown straight back to
//! whichever track is scored most thickly.

/// The stretches of the arrangement one track sounds over, in beats.
///
/// Collected unsorted and merged once at the end: notes arrive in pattern
/// order and patterns arrive in arrangement order, so the stream is nearly
/// sorted already but not reliably, and sorting once costs less than keeping
/// it ordered on every insert.
#[derive(Debug, Default)]
pub(super) struct Sounding(Vec<(f32, f32)>);

impl Sounding {
    /// Records one note sounding from `at` for `beats`.
    ///
    /// A note of no length is dropped rather than stored as a point: it
    /// occupies no time, and an empty interval would only survive to be merged
    /// with something.
    pub(super) fn add(&mut self, at: f32, beats: f32) {
        if beats > 0.0 {
            self.0.push((at, at + beats));
        }
    }

    /// The share of a `beats`-long arrangement this track sounds over, in
    /// `0.0..=1.0`.
    ///
    /// Anything ringing past the last beat is cut off there. A song's length is
    /// where its final pattern ends rather than where the audio stops, and a
    /// held final chord would otherwise push a track past 100% of a piece.
    pub(super) fn over(mut self, beats: f32) -> f32 {
        if beats <= 0.0 {
            return 0.0;
        }
        self.0.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut sounding = 0.0;
        let mut open: Option<(f32, f32)> = None;
        for (from, to) in self.0 {
            let (from, to) = (from.max(0.0), to.min(beats));
            if from >= to {
                continue;
            }
            match open {
                // Overlapping or touching: one stretch of sound, not two.
                Some((start, end)) if from <= end => open = Some((start, end.max(to))),
                Some((start, end)) => {
                    sounding += end - start;
                    open = Some((from, to));
                }
                None => open = Some((from, to)),
            }
        }
        if let Some((start, end)) = open {
            sounding += end - start;
        }
        (sounding / beats).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sounding(notes: &[(f32, f32)]) -> Sounding {
        let mut over = Sounding::default();
        for (at, beats) in notes {
            over.add(*at, *beats);
        }
        over
    }

    /// The case the whole module is for: a short, loud hit occupies a fraction
    /// of the bar it is written in.
    #[test]
    fn a_track_that_plays_briefly_sounds_briefly() {
        let hat = sounding(&[(0.0, 0.2), (1.0, 0.2), (2.0, 0.2), (3.0, 0.2)]);
        assert!((hat.over(4.0) - 0.2).abs() < 0.001);
    }

    /// A chord is one stretch of sound, not one per voice — otherwise the
    /// thickest-scored track wins whatever it is playing.
    #[test]
    fn simultaneous_notes_are_counted_once() {
        let chord = sounding(&[(0.0, 4.0), (0.0, 4.0), (0.0, 4.0)]);
        assert!((chord.over(4.0) - 1.0).abs() < 0.001);
    }

    /// Touching notes are one stretch too, so a legato line does not lose the
    /// instants between its notes.
    #[test]
    fn notes_that_meet_are_one_stretch() {
        let line = sounding(&[(0.0, 1.0), (1.0, 1.0), (2.0, 2.0)]);
        assert!((line.over(4.0) - 1.0).abs() < 0.001);
    }

    /// A note ringing past the last beat is cut off there: the arrangement's
    /// length is where its final pattern ends.
    #[test]
    fn nothing_sounds_for_more_than_all_of_it() {
        let over = sounding(&[(3.0, 40.0)]);
        assert!((over.over(4.0) - 0.25).abs() < 0.001);
    }

    #[test]
    fn a_track_that_never_plays_sounds_for_none_of_it() {
        assert_eq!(sounding(&[]).over(4.0), 0.0);
        assert_eq!(sounding(&[(0.0, 0.0)]).over(4.0), 0.0);
        assert_eq!(
            sounding(&[(0.0, 1.0)]).over(0.0),
            0.0,
            "no piece to be part of"
        );
    }
}
