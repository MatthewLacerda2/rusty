//! Which layer of a mix owns the energy.
//!
//! [`super::profile`] cuts a signal by **time**; this cuts it by **part**. They
//! answer questions that look alike and are not: a section row says *when* a
//! piece is muddy, and a layer row says *which of the five things playing* is
//! muddying it. A mix report without the second is a diagnosis with no address
//! — "87% of the energy is below 250 Hz" is correct, actionable by nobody, and
//! is fixed by changing four instruments at once and hoping.
//!
//! A layer is measured with the same two meters, over the same window, as the
//! sum it sits under, which is what lets the rows be read as a column: the
//! numbers are comparable because they are the same numbers.
//!
//! **Including how wide it is**, which is the column that most needs an
//! address. A mix that reads negative — cancelling, and about to collapse the
//! moment anything folds it to mono — is a finding with nowhere to go until
//! something says *which of the five things playing* is doing it; that is the
//! same argument this file exists for. The column is free besides: a layer row
//! and a section row are printed by one formatter precisely so the two can be
//! read as one column of numbers, and suppressing it here would take a flag
//! and would break that.
//!
//! **Whole first, and by section beside it.** A layer's row is the whole
//! piece, one line per instrument, so the table stays as tall as the mix has
//! parts. The same layer is also cut at the arrangement's sections, because
//! "which instrument is quiet *in the trio*" is a question neither a section
//! row (the whole mix) nor a layer row (the whole piece) can answer — and the
//! answer used to be an inference from which tracks play where. How much of
//! that a report prints is the formatter's decision; measuring it here costs a
//! second pair of meters over samples already in hand.

use super::profile::{Cut, Profiler, Span};

/// One layer of a mix, measured on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// What the document calls it — in a song, a track's name.
    pub name: String,
    /// How it came out: the same statistics, in the same columns, as the
    /// summary above it.
    ///
    /// A [`Span`] rather than a type of its own precisely because it must line
    /// up with the rows around it — a layer measured differently from the sum
    /// could not be compared with it, which is the only thing anyone does with
    /// one. Its `label` is `None`; the name a reader wants is beside it.
    pub level: Span,
    /// The same layer a section at a time, cut exactly where the sum's own
    /// section rows are — so the n-th entry here and the n-th section row are
    /// the same stretch of the piece.
    ///
    /// Empty whenever the sum has no section rows, by the same rule
    /// [`super::Profile::sections`] follows.
    pub sections: Vec<Span>,
}

impl Layer {
    /// Measures `samples` whole, as one named span, and cut at `cuts` the way
    /// the sum it sits under is.
    ///
    /// The samples are handed over complete rather than fed a run at a time,
    /// because a layer only exists at the moment a mix is summed — there is no
    /// streaming caller for it, and the buffer is in hand and about to be
    /// dropped.
    pub fn of(
        name: impl Into<String>,
        samples: &[f32],
        channels: usize,
        rate: u32,
        cuts: Vec<Cut>,
    ) -> Self {
        // The profiler the sum is measured with, so a layer's sections are
        // cut by the same boundary arithmetic and its whole is the same
        // number a lone meter would have produced.
        let mut profiler = Profiler::sectioned(channels, rate, cuts);
        profiler.feed(samples);
        let profile = profiler.finish();
        Self {
            name: name.into(),
            level: profile.whole,
            sections: profile.sections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layer is measured exactly as the whole file is, which is the only
    /// reason its row can be read against the summary above it.
    #[test]
    fn a_layer_reports_the_same_statistics_as_a_whole_signal() {
        let square: Vec<f32> = (0..1_000)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let layer = Layer::of("sub", &square, 1, 1_000, Vec::new());
        assert_eq!(layer.name, "sub");
        let mean = layer.level.loudness.mean_dbfs.expect("it is not silent");
        assert!(
            (mean + 6.0).abs() < 0.05,
            "half scale is -6 dBFS, not {mean}"
        );
        assert!((layer.level.to_seconds - 1.0).abs() < 1e-9);
    }

    /// A track that never played is said to be silent rather than left out: a
    /// missing row reads as an oversight, and "the arp is not in this mix" is a
    /// finding.
    #[test]
    fn a_layer_that_never_played_is_silent_rather_than_absent() {
        let layer = Layer::of("arp", &[], 1, 48_000, Vec::new());
        assert!(layer.level.loudness.is_silent());
        assert!(layer.level.bands.is_none(), "a silence has no balance");
    }

    /// A layer is cut where the sum is, so its n-th section and the sum's n-th
    /// row are the same stretch — which is what lets a reader say the brass
    /// is the quiet part of the trio rather than infer it.
    #[test]
    fn a_layer_is_cut_where_the_arrangement_says() {
        let cut = |label: &str, end_seconds| Cut {
            label: label.to_owned(),
            end_seconds,
        };
        // Silent for the first second, half scale for the second.
        let mut samples = vec![0.0; 1_000];
        samples.extend((0..1_000).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }));
        let layer = Layer::of(
            "brass",
            &samples,
            1,
            1_000,
            vec![cut("verse", 1.0), cut("trio", 2.0)],
        );
        assert_eq!(layer.sections.len(), 2);
        assert!(layer.sections[0].loudness.is_silent());
        assert_eq!(layer.sections[1].label.as_deref(), Some("trio"));
        let mean = layer.sections[1].loudness.mean_dbfs.expect("it played");
        assert!((mean + 6.0).abs() < 0.05, "{mean}");
    }
}
