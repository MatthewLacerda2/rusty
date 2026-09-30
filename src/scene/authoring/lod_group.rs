//! src/scene/authoring/lod_group.rs — Shared LODGroup-authoring ops (#472).
//!
//! The ONE place the engine mutates an entity's first-class `LodGroupComponent`.
//! The editor's LOD Group card and the Lua `LODGroup.*` namespace both route every
//! write through these, so the invariants live once: screen heights stay within
//! `0..=1` and never increase from one level to the next (a level whose threshold
//! rose above its finer neighbour's could never be shown), the size is positive,
//! and a level index out of range is a no-op.
//!
//! Allowed deps: components (the `LodGroupComponent` data). Pure.

use crate::components::lod_group::{default_thresholds, LodGroupComponent, LodLevel};

/// The smallest group size, in metres: a zero size would make every level cull.
pub const MIN_SIZE: f32 = 0.001;

/// Set the group's world-space size, floored at [`MIN_SIZE`]. A non-finite size is
/// ignored.
pub fn set_size(g: &mut LodGroupComponent, size: f32) {
    if size.is_finite() {
        g.size = size.max(MIN_SIZE);
    }
}

/// Replace every level, then order the thresholds (see [`normalize`]).
pub fn set_levels(g: &mut LodGroupComponent, levels: Vec<LodLevel>) {
    g.levels = levels;
    normalize(g);
}

/// Set level `level`'s screen height, clamped between its neighbours' so the order
/// holds. Out of range: no-op.
pub fn set_level_height(g: &mut LodGroupComponent, level: usize, height: f32) {
    if level >= g.levels.len() || !height.is_finite() {
        return;
    }
    let finer = level
        .checked_sub(1)
        .map_or(1.0, |i| g.levels[i].screen_height);
    let coarser = g.levels.get(level + 1).map_or(0.0, |l| l.screen_height);
    g.levels[level].screen_height = height.clamp(coarser, finer);
}

/// Set the entities that render level `level`. Out of range: no-op.
pub fn set_renderers(g: &mut LodGroupComponent, level: usize, renderers: Vec<u32>) {
    if let Some(l) = g.levels.get_mut(level) {
        l.renderers = renderers;
    }
}

/// Append a coarser level with no renderers, at half the current last threshold,
/// and return its index.
pub fn add_level(g: &mut LodGroupComponent) -> usize {
    let screen_height = match g.levels.last() {
        Some(l) => l.screen_height * 0.5,
        None => default_thresholds(1)[0],
    };
    g.levels.push(LodLevel {
        screen_height,
        renderers: Vec::new(),
    });
    g.levels.len() - 1
}

/// Remove level `level` (its renderers are no longer managed). Out of range: no-op.
pub fn remove_level(g: &mut LodGroupComponent, level: usize) {
    if level < g.levels.len() {
        g.levels.remove(level);
    }
}

/// Clamp every threshold into `0..=1` and to at most its finer neighbour's, so the
/// levels read finest-first; a non-finite threshold becomes its neighbour's.
fn normalize(g: &mut LodGroupComponent) {
    let mut ceiling = 1.0f32;
    for level in &mut g.levels {
        let h = match level.screen_height.is_finite() {
            true => level.screen_height,
            false => ceiling,
        };
        level.screen_height = h.clamp(0.0, ceiling);
        ceiling = level.screen_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(screen_height: f32, renderers: &[u32]) -> LodLevel {
        LodLevel {
            screen_height,
            renderers: renderers.to_vec(),
        }
    }

    #[test]
    fn set_levels_orders_and_clamps_the_thresholds() {
        let mut g = LodGroupComponent::default();
        let levels = vec![level(1.5, &[1]), level(0.2, &[2]), level(0.4, &[3])];
        set_levels(&mut g, levels);
        let heights: Vec<f32> = g.levels.iter().map(|l| l.screen_height).collect();
        assert_eq!(heights, vec![1.0, 0.2, 0.2]);
        assert_eq!(g.levels[2].renderers, vec![3]);
    }

    #[test]
    fn level_height_stays_between_its_neighbours() {
        let mut g = LodGroupComponent::default();
        set_levels(
            &mut g,
            vec![level(0.6, &[]), level(0.3, &[]), level(0.1, &[])],
        );
        set_level_height(&mut g, 1, 0.9);
        assert_eq!(g.levels[1].screen_height, 0.6);
        set_level_height(&mut g, 1, 0.0);
        assert_eq!(g.levels[1].screen_height, 0.1);
        set_level_height(&mut g, 7, 0.5);
        set_level_height(&mut g, 0, f32::NAN);
        assert_eq!(g.levels[0].screen_height, 0.6);
    }

    #[test]
    fn levels_add_remove_and_take_renderers() {
        let mut g = LodGroupComponent::default();
        let last = g.levels[1].screen_height;
        assert_eq!(add_level(&mut g), 2);
        assert_eq!(g.levels[2].screen_height, last * 0.5);
        set_renderers(&mut g, 2, vec![4, 5]);
        assert_eq!(g.renderers().collect::<Vec<_>>(), vec![4, 5]);
        remove_level(&mut g, 0);
        remove_level(&mut g, 9);
        assert_eq!(g.levels.len(), 2);
        set_size(&mut g, -3.0);
        assert_eq!(g.size, MIN_SIZE);
        set_size(&mut g, f32::INFINITY);
        assert_eq!(g.size, MIN_SIZE);
    }
}
