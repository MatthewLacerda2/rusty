use super::GpuPass;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum GpuPart {
        OcclusionSelect = "select",
        /// Phase one: the visible set's depth-only draw.
        OcclusionPhaseOne = "phase_one",
        OcclusionPyramid = "pyramid",
        OcclusionTest = "test",
        OcclusionClusters = "clusters",
        OcclusionClustersTest = "clusters_test",
        OcclusionClustersScan = "clusters_scan",
        OcclusionClustersCopy = "clusters_copy",
    }
}

impl GpuPart {
    pub fn pass(self) -> GpuPass {
        match self {
            Self::OcclusionSelect
            | Self::OcclusionPhaseOne
            | Self::OcclusionPyramid
            | Self::OcclusionTest
            | Self::OcclusionClusters
            | Self::OcclusionClustersTest
            | Self::OcclusionClustersScan
            | Self::OcclusionClustersCopy => GpuPass::Occlusion,
        }
    }

    pub fn within(self) -> Option<GpuPart> {
        match self {
            Self::OcclusionClustersTest
            | Self::OcclusionClustersScan
            | Self::OcclusionClustersCopy => Some(Self::OcclusionClusters),
            Self::OcclusionSelect
            | Self::OcclusionPhaseOne
            | Self::OcclusionPyramid
            | Self::OcclusionTest
            | Self::OcclusionClusters => None,
        }
    }

    pub fn row(self) -> String {
        format!("{}.{}", self.pass().name(), self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pair {
    pub pass: GpuPass,
    pub part: Option<GpuPart>,
    pub submit: u64,
}

#[cfg(test)]
impl From<GpuPass> for Pair {
    fn from(pass: GpuPass) -> Self {
        Self {
            pass,
            part: None,
            submit: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuTimes {
    pub frame: u64,
    pub passes: Vec<(GpuPass, f64)>,
    pub parts: Vec<(GpuPart, f64)>,
    pub other_ms: f64,
    pub between_submits_ms: f64,
}

impl GpuTimes {
    pub fn total_ms(&self) -> f64 {
        self.passes.iter().map(|(_, ms)| ms).sum()
    }

    /// From the frame's earliest timed start to its latest timed end, in ms: the
    /// rows, [`Self::other_ms`] and [`Self::between_submits_ms`] together.
    pub fn span_ms(&self) -> f64 {
        self.total_ms() + self.other_ms + self.between_submits_ms
    }

    pub(crate) fn from_ticks(ticks: &[u64], pairs: &[Pair], period_ns: f64) -> Self {
        let to_ms = |t: u64| t as f64 * period_ns / 1e6;
        let mut charged = [None::<u64>; GpuPass::ALL.len()];
        let mut parts = [None::<u64>; GpuPart::ALL.len()];
        let mut done = None::<(u64, u64)>;
        let mut span = None::<(u64, u64)>;
        let mut between = 0;
        for (ends, pair) in ticks.chunks_exact(2).zip(pairs) {
            let (start, end) = (ends[0], ends[1]);
            if start == 0 || start > end {
                continue;
            }
            let begin = done.map_or(start, |(d, _)| start.max(d));
            let own = end.saturating_sub(begin);
            let slot = &mut charged[pair.pass as usize];
            *slot = Some(slot.unwrap_or(0) + own);
            let within = pair.part.and_then(GpuPart::within);
            for part in pair.part.into_iter().chain(within) {
                let slot = &mut parts[part as usize];
                *slot = Some(slot.unwrap_or(0) + own);
            }
            if let Some((d, _)) = done.filter(|&(_, s)| s != pair.submit) {
                between += start.saturating_sub(d);
            }
            done = Some((done.map_or(end, |(d, _)| d.max(end)), pair.submit));
            span = Some(span.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
        }
        let busy: u64 = charged.iter().flatten().sum();
        let span = span.map_or(0, |(a, b)| b.saturating_sub(a));
        let passes = GpuPass::ALL
            .iter()
            .zip(charged)
            .filter_map(|(pass, ticks)| Some((*pass, to_ms(ticks?))))
            .collect();
        let parts = GpuPart::ALL
            .iter()
            .zip(parts)
            .filter_map(|(part, ticks)| Some((*part, to_ms(ticks?))))
            .collect();
        Self {
            frame: 0,
            passes,
            parts,
            other_ms: to_ms(span.saturating_sub(busy).saturating_sub(between)),
            between_submits_ms: to_ms(between),
        }
    }
}

#[cfg(test)]
#[path = "times_tests.rs"]
mod tests;
