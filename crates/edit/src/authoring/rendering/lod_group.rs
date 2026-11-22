//! Screen heights never increase from one level to the next: a level whose threshold rose above its
//! finer neighbour's could never be shown.

use rusty_components::rendering::lod_group::{default_thresholds, LodGroupComponent, LodLevel};
use rusty_scene::rules::sanitize::{at_least, clamped, finite_or, unit};

/// Metres: a zero size would make every level cull.
pub const MIN_SIZE: f32 = rusty_scene::rules::sanitize::MIN_EXTENT;

pub fn set_size(g: &mut LodGroupComponent, size: f32) {
    g.size = at_least(finite_or(size, g.size), MIN_SIZE);
}

/// Neighbours a loaded file left out of range or out of order are first held to `[0, 1]`, and the
/// finer one wins.
pub fn set_level_height(g: &mut LodGroupComponent, level: usize, height: f32) {
    if level >= g.levels.len() {
        return;
    }
    let finer = level
        .checked_sub(1)
        .map_or(1.0, |i| unit(g.levels[i].screen_height));
    let coarser = g
        .levels
        .get(level + 1)
        .map_or(0.0, |l| unit(l.screen_height));
    let height = finite_or(height, g.levels[level].screen_height);
    g.levels[level].screen_height = clamped(height, coarser.min(finer), finer);
}

pub fn set_renderers(g: &mut LodGroupComponent, level: usize, renderers: Vec<u32>) {
    if let Some(l) = g.levels.get_mut(level) {
        l.renderers = renderers;
    }
}

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

pub fn remove_level(g: &mut LodGroupComponent, level: usize) {
    if level < g.levels.len() {
        g.levels.remove(level);
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
    fn level_height_stays_between_its_neighbours() {
        let mut g = LodGroupComponent {
            levels: vec![level(0.6, &[]), level(0.3, &[]), level(0.1, &[])],
            ..Default::default()
        };
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
