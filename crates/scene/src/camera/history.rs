//! crates/scene/src/camera/history.rs — each camera's history cuts (#2003): Unity URP's
//! `UniversalAdditionalCameraData.resetHistory`, HDRP's `HDCamera.Reset`.
//!
//! The renderer keeps a history per view (motion vectors, TAA, SSIL, volumetric
//! fog, automatic exposure) and starts every one of them over when the view's
//! `render::motion::HistoryKey` changes. A cut asked for here is one more part of
//! that key: [`Camera.ResetHistory`](crate::api) bumps the camera's count, so the
//! next frame its view draws keys differently and starts over, exactly as on a
//! camera switch. That is how a teleport is told from fast motion: the caller says
//! so, as in Unity, and the engine guesses nothing from distance.

use std::collections::BTreeMap;

/// How many cuts each camera has been asked for: by its entity, `None` for the
/// free view camera (a scene with no screen camera, the editor's Scene view when
/// the scene has none). A camera never cut counts 0. Never saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryCuts(BTreeMap<Option<u32>, u32>);

impl HistoryCuts {
    /// Cut `camera`'s view: the next frame it draws starts every history over.
    pub fn cut(&mut self, camera: Option<u32>) {
        let n = self.0.entry(camera).or_default();
        *n = n.wrapping_add(1);
    }

    /// How many cuts `camera` has been asked for: a part of its view's history key.
    pub fn of(&self, camera: Option<u32>) -> u32 {
        self.0.get(&camera).copied().unwrap_or(0)
    }

    /// Keep only the entity cameras `keep` says, and the free camera's count.
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
