//! The atlas's static-tile bookkeeping (#694): what re-bakes, and what a bake forgets.

use glam::{Mat4, Vec3};

use super::StaticTiles;
use crate::render::passes::shadows::atlas::Tile;
use crate::scene::SceneId;

/// A tile at `origin`, `size` texels across, seen from `eye`.
fn tile(origin: [u32; 2], size: u32, eye: Vec3) -> Tile {
    Tile {
        view_proj: Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
        origin,
        size,
        texel: 0.01,
    }
}

#[test]
fn a_baked_tile_stays_fresh_until_its_light_or_place_changes() {
    let scene = SceneId::next();
    let home = tile([0, 0], 256, Vec3::X);
    let mut cache = StaticTiles::default();
    assert_eq!(cache.stale(scene, &[home]), [0], "nothing baked yet");
    cache.commit(scene, [home]);
    assert!(cache.stale(scene, &[home]).is_empty(), "still: cached");

    let moved = tile([0, 0], 256, Vec3::new(1.0, 2.0, 3.0));
    let shifted = tile([256, 0], 256, Vec3::X);
    let shrunk = tile([0, 0], 128, Vec3::X);
    assert_eq!(cache.stale(scene, &[moved, shifted, shrunk]), [0, 1, 2]);
    assert_eq!(cache.stale(SceneId::next(), &[home]), [0], "another scene");
}

#[test]
fn a_bake_forgets_only_the_tiles_it_drew_over() {
    let scene = SceneId::next();
    let left = tile([0, 0], 256, Vec3::X);
    let right = tile([256, 0], 256, Vec3::Z);
    let mut cache = StaticTiles::default();
    cache.commit(scene, [left, right]);

    // A smaller tile inside `left` overwrites part of it: `left` must re-bake.
    let inside = tile([128, 128], 128, Vec3::new(1.0, 2.0, 3.0));
    cache.commit(scene, [inside]);
    assert_eq!(cache.stale(scene, &[left, right, inside]), [0]);

    // Edge-adjacent tiles share no texel.
    let below = tile([0, 256], 256, Vec3::NEG_X);
    cache.commit(scene, [below]);
    assert!(cache.stale(scene, &[right, inside, below]).is_empty());

    cache.clear();
    assert_eq!(cache.stale(scene, &[right]), [0], "cleared");
}
