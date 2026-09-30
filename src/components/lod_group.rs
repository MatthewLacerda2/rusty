//! src/components/lod_group.rs — LODGroup: show one level of detail at a time (#472).
//!
//! Unity's `LODGroup`. It sits on a parent entity and lists its **levels**, finest
//! first. Each level names the entities that render it (`renderers`, usually the
//! group's children) and the **screen-relative height** down to which it is shown:
//! the fraction of the viewport's height the group's `size` covers at its distance
//! from the camera. Level `i` shows while that height is at least
//! `levels[i].screen_height` (and below the previous level's); below the last
//! level's threshold the whole group is culled. So `[0.5, 0.2, 0.02]` draws LOD0
//! while the object fills half the screen or more, LOD1 down to a fifth, LOD2 down
//! to 2%, and nothing smaller.
//!
//! Selection is **render-side only** (`render::lod`): the sim never reads which
//! level is shown, so it cannot break determinism. The renderer references are
//! entity ids that follow the entity through prefab save / stamp
//! ([`LodGroupComponent::remap_refs`]). glTF files whose sub-objects are suffixed
//! `_LOD0`, `_LOD1`, … build one of these on instantiate (`scene::lod_instance`).

use serde::{Deserialize, Serialize};

/// One level of detail: which entities draw it, and down to what screen height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LodLevel {
    /// The smallest screen-relative height (0..1) this level is shown at.
    pub screen_height: f32,
    /// The entities that render this level. An entity listed in no level is never
    /// hidden by the group.
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

/// A level-of-detail group. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LodGroupComponent {
    /// The levels, finest (LOD0) first; thresholds strictly decreasing.
    pub levels: Vec<LodLevel>,
    /// The group's world-space size in metres (before the entity's scale) — the
    /// height whose on-screen fraction picks the level. Unity's `LODGroup.size`.
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

/// The screen heights a fresh `count`-level group starts with: each level half the
/// last (0.5, 0.25, …), and the coarsest shown down to 1% of the screen.
pub fn default_thresholds(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| match i + 1 == count {
            true => 0.01,
            false => 0.5f32.powi(i as i32 + 1),
        })
        .collect()
}

impl LodGroupComponent {
    /// The level shown at screen-relative `height`: the first whose threshold it
    /// reaches, or `None` (culled) below the last. An empty group shows nothing of
    /// its own — it has no renderers to hide either.
    pub fn level_at(&self, height: f32) -> Option<usize> {
        self.levels.iter().position(|l| height >= l.screen_height)
    }

    /// Every renderer the group manages, across all levels.
    pub fn renderers(&self) -> impl Iterator<Item = u32> + '_ {
        self.levels.iter().flat_map(|l| l.renderers.iter().copied())
    }

    /// Whether `pointer` (inside the component) names a renderer reference —
    /// `/levels/<n>/renderers/<m>` — how prefab write-back finds the override leaves
    /// that hold entity ids.
    pub fn is_ref_pointer(pointer: &str) -> bool {
        let parts: Vec<&str> = pointer.split('/').collect();
        let index = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        matches!(parts.as_slice(), ["", "levels", n, "renderers", m] if index(n) && index(m))
    }

    /// Rewrite every renderer reference through `map`; a reference `map` drops leaves
    /// its level — how the group follows a prefab save / stamp.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        for level in &mut self.levels {
            level.renderers = level.renderers.iter().filter_map(|&r| map(r)).collect();
        }
    }
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
