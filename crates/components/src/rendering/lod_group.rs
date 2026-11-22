//! Unity's `LODGroup`: `levels` finest first, each shown down to its **screen-relative height**, the
//! fraction of the viewport the group's `size` covers. Below the last threshold the group is culled.
//!
//! Selection is **render-side only** (`render::lod`): the sim never reads the shown level, so
//! determinism holds. Renderer entity ids follow the entity through prefab save / stamp (`EntityRefs`).

use serde::{Deserialize, Serialize};

use crate::{EntityRefs, RefVisit};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LodLevel {
    pub screen_height: f32,
    pub renderers: Vec<u32>,
}

impl Default for LodLevel {
    fn default() -> Self {
        Self {
            screen_height: 0.01,
            renderers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LodGroupComponent {
    pub levels: Vec<LodLevel>,
    /// Metres, before the entity's scale: the height whose on-screen fraction picks the level.
    pub size: f32,
}

impl Default for LodGroupComponent {
    fn default() -> Self {
        Self {
            levels: default_thresholds(2)
                .into_iter()
                .map(|screen_height| LodLevel {
                    screen_height,
                    renderers: Vec::new(),
                })
                .collect(),
            size: 1.0,
        }
    }
}

pub fn default_thresholds(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| match i + 1 == count {
            true => 0.01,
            false => 0.5f32.powi(i as i32 + 1),
        })
        .collect()
}

impl LodGroupComponent {
    /// The first level whose threshold `height` reaches, or `None` (culled) below the last.
    pub fn level_at(&self, height: f32) -> Option<usize> {
        self.levels.iter().position(|l| height >= l.screen_height)
    }

    pub fn renderers(&self) -> impl Iterator<Item = u32> + '_ {
        self.levels.iter().flat_map(|l| l.renderers.iter().copied())
    }

    /// Whether `pointer` is `/levels/<n>/renderers/<m>`, how prefab write-back finds entity-id leaves.
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let parts: Vec<&str> = pointer.split('/').collect();
        let index = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        matches!(parts.as_slice(), ["", "levels", n, "renderers", m] if index(n) && index(m))
    }
}

impl EntityRefs for LodGroupComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        for (n, level) in self.levels.iter_mut().enumerate() {
            let mut kept = Vec::with_capacity(level.renderers.len());
            for (m, &r) in level.renderers.iter().enumerate() {
                let mut slot = Some(r);
                visit(&format!("/levels/{n}/renderers/{m}"), &mut slot);
                kept.extend(slot);
            }
            level.renderers = kept;
        }
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        for (n, level) in self.levels.iter().enumerate() {
            for (m, &r) in level.renderers.iter().enumerate() {
                visit(&format!("/levels/{n}/renderers/{m}"), r);
            }
        }
    }

    #[doc(hidden)]
    fn every_ref() -> Self {
        Self {
            levels: vec![LodLevel {
                renderers: vec![0],
                ..LodLevel::default()
            }],
            ..Self::default()
        }
    }
}

impl crate::FirstClass for LodGroupComponent {
    const NAME: &'static str = "LODGroup";
    const ALIASES: &'static [&'static str] = &["lod"];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(thresholds: &[f32]) -> LodGroupComponent {
        LodGroupComponent {
            levels: thresholds
                .iter()
                .enumerate()
                .map(|(i, &screen_height)| LodLevel {
                    screen_height,
                    renderers: vec![i as u32 + 10],
                })
                .collect(),
            size: 1.0,
        }
    }

    #[test]
    fn height_picks_the_first_level_it_reaches_and_culls_below_the_last() {
        let g = group(&[0.5, 0.2, 0.02]);
        assert_eq!(g.level_at(0.9), Some(0));
        assert_eq!(g.level_at(0.5), Some(0));
        assert_eq!(g.level_at(0.3), Some(1));
        assert_eq!(g.level_at(0.02), Some(2));
        assert_eq!(g.level_at(0.01), None);
        assert_eq!(LodGroupComponent::default().levels.len(), 2);
    }

    #[test]
    fn default_thresholds_halve_and_end_at_one_percent() {
        assert_eq!(default_thresholds(1), vec![0.01]);
        assert_eq!(default_thresholds(3), vec![0.5, 0.25, 0.01]);
    }

    #[test]
    fn remap_rewrites_and_drops_renderers() {
        let mut g = group(&[0.5, 0.1]);
        g.remap_refs(&|id| (id != 11).then_some(id + 100));
        assert_eq!(g.levels[0].renderers, vec![110]);
        assert!(g.levels[1].renderers.is_empty());
        assert_eq!(g.renderers().count(), 1);
    }

    #[test]
    fn ref_pointers_are_level_renderer_slots() {
        assert!(LodGroupComponent::is_ref_pointer("/levels/0/renderers/3"));
        assert!(!LodGroupComponent::is_ref_pointer(
            "/levels/0/screen_height"
        ));
        assert!(!LodGroupComponent::is_ref_pointer("/levels/x/renderers/0"));
        assert!(!LodGroupComponent::is_ref_pointer("/size"));
    }
}
