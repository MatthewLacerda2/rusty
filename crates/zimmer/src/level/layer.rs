//! Which layer of a mix owns the energy.

use super::profile::{Cut, Profiler, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub level: Span,
    pub sections: Vec<Span>,
}

impl Layer {
    pub fn of(
        name: impl Into<String>,
        samples: &[f32],
        channels: usize,
        rate: u32,
        cuts: Vec<Cut>,
    ) -> Self {
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

    #[test]
    fn a_layer_that_never_played_is_silent_rather_than_absent() {
        let layer = Layer::of("arp", &[], 1, 48_000, Vec::new());
        assert!(layer.level.loudness.is_silent());
        assert!(layer.level.bands.is_none(), "a silence has no balance");
    }

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
