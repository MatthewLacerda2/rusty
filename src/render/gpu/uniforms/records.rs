//! The material records one bind group's draws index (#1616), shared by the solids
//! (`FrameDraws::pack`), the editor overlays (`OverlayDraws`) and the shadow casters
//! (`CasterRecords`).

use super::{MaterialData, MATERIAL_DATA_WORDS};

/// A record as its packed words: compared bit for bit, so a NaN tint matches itself
/// and `-0.0` stays apart from `0.0`, as the GPU reads them.
pub(crate) type RecordWords = [u32; MATERIAL_DATA_WORDS];

/// Each distinct record once, sorted by its words: the array is the same whatever
/// order the draws name their records in, so a camera that moves and reorders its
/// draws uploads the same records while the materials hold still.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct RecordTable(Vec<RecordWords>);

impl RecordTable {
    /// The table of every record `words` names, duplicates folded.
    pub(crate) fn new(words: impl IntoIterator<Item = RecordWords>) -> Self {
        let mut words: Vec<_> = words.into_iter().collect();
        words.sort_unstable();
        words.dedup();
        Self(words)
    }

    /// Where `words` sits in the array; the table must hold it.
    pub(crate) fn index(&self, words: &RecordWords) -> u32 {
        let found = self.0.binary_search(words);
        found.expect("the table was built from every record its draws name") as u32
    }

    /// How many records the array holds.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// The array the bind group's material binding reads.
    pub(crate) fn records(&self) -> Vec<MaterialData> {
        self.0.iter().map(MaterialData::from_words).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tint(r: f32) -> RecordWords {
        MaterialData::flat([r, 0.0, 0.0, 1.0]).words()
    }

    #[test]
    fn the_array_is_independent_of_the_order_draws_name_records_in() {
        let a = RecordTable::new([tint(0.5), tint(0.1), tint(0.5)]);
        let b = RecordTable::new([tint(0.1), tint(0.5)]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
        assert_eq!(a.index(&tint(0.5)), b.index(&tint(0.5)));
    }

    #[test]
    fn records_match_by_their_bits() {
        let table = RecordTable::new([tint(f32::NAN), tint(f32::NAN), tint(0.0), tint(-0.0)]);
        assert_eq!(
            table.len(),
            3,
            "NaN folds with itself; -0.0 stays apart from 0.0"
        );
        assert_ne!(table.index(&tint(0.0)), table.index(&tint(-0.0)));
        let nan = table.index(&tint(f32::NAN)) as usize;
        assert!(table.records()[nan].color_tint[0].is_nan());
    }
}
