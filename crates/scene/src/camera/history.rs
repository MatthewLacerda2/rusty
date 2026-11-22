use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryCuts(BTreeMap<Option<u32>, u32>);

impl HistoryCuts {
    pub fn cut(&mut self, camera: Option<u32>) {
        let n = self.0.entry(camera).or_default();
        *n = n.wrapping_add(1);
    }

    pub fn of(&self, camera: Option<u32>) -> u32 {
        self.0.get(&camera).copied().unwrap_or(0)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(u32) -> bool) {
        self.0.retain(|id, _| id.is_none_or(&mut keep));
    }
}

#[cfg(test)]
mod tests {
    use super::HistoryCuts;

    #[test]
    fn a_cut_changes_only_its_cameras_count() {
        let mut cuts = HistoryCuts::default();
        assert_eq!((cuts.of(None), cuts.of(Some(1))), (0, 0));
        cuts.cut(Some(1));
        assert_eq!(
            (cuts.of(None), cuts.of(Some(1)), cuts.of(Some(2))),
            (0, 1, 0)
        );
        cuts.cut(None);
        cuts.cut(Some(1));
        assert_eq!((cuts.of(None), cuts.of(Some(1))), (1, 2));
    }

    #[test]
    fn retain_drops_entity_cameras_and_keeps_the_free_one() {
        let mut cuts = HistoryCuts::default();
        cuts.cut(None);
        cuts.cut(Some(1));
        cuts.cut(Some(2));
        cuts.retain(|id| id == 2);
        assert_eq!(
            (cuts.of(None), cuts.of(Some(1)), cuts.of(Some(2))),
            (1, 0, 1)
        );
    }
}
