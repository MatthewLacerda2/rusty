//! How much of a piece a track is actually sounding for.

#[derive(Debug, Default)]
/// The `(from, to)` beats one track sounds over.
pub(super) struct Sounding(Vec<(f32, f32)>);

impl Sounding {
    pub(super) fn add(&mut self, at: f32, beats: f32) {
        if beats > 0.0 {
            self.0.push((at, at + beats));
        }
    }

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

    #[test]
    fn a_track_that_plays_briefly_sounds_briefly() {
        let hat = sounding(&[(0.0, 0.2), (1.0, 0.2), (2.0, 0.2), (3.0, 0.2)]);
        assert!((hat.over(4.0) - 0.2).abs() < 0.001);
    }

    #[test]
    fn simultaneous_notes_are_counted_once() {
        let chord = sounding(&[(0.0, 4.0), (0.0, 4.0), (0.0, 4.0)]);
        assert!((chord.over(4.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn notes_that_meet_are_one_stretch() {
        let line = sounding(&[(0.0, 1.0), (1.0, 1.0), (2.0, 2.0)]);
        assert!((line.over(4.0) - 1.0).abs() < 0.001);
    }

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
