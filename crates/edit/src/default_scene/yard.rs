use glam::Vec3;

use super::layout::*;
use super::{block, looks, Scene};

pub(super) fn add_deck(scene: &mut Scene) {
    let ((yx, yz), (px, pz)) = (YARD_HALF, POOL_HALF);
    let (lo, hi) = (SLAB_BOTTOM, DECK_TOP);
    let slabs = [
        (
            "Deck_South",
            Vec3::new(-yx, lo, -yz),
            Vec3::new(yx, hi, -pz),
        ),
        ("Deck_North", Vec3::new(-yx, lo, pz), Vec3::new(yx, hi, yz)),
        ("Deck_West", Vec3::new(-yx, lo, -pz), Vec3::new(-px, hi, pz)),
        ("Deck_East", Vec3::new(px, lo, -pz), Vec3::new(yx, hi, pz)),
    ];
    for (name, min, max) in slabs {
        block(scene, name, min, max, looks::DECK);
    }
}

pub(super) fn add_walls(scene: &mut Scene) {
    let (yx, yz) = YARD_HALF;
    let (t, top) = (WALL_THICK, DECK_TOP + WALL_HEIGHT);
    let walls = [
        (
            "Wall_South",
            Vec3::new(-yx, DECK_TOP, -yz),
            Vec3::new(yx, top, t - yz),
        ),
        (
            "Wall_North",
            Vec3::new(-yx, DECK_TOP, yz - t),
            Vec3::new(yx, top, yz),
        ),
        (
            "Wall_West",
            Vec3::new(-yx, DECK_TOP, t - yz),
            Vec3::new(t - yx, top, yz - t),
        ),
        (
            "Wall_East",
            Vec3::new(yx - t, DECK_TOP, t - yz),
            Vec3::new(yx, top, yz - t),
        ),
    ];
    for (name, min, max) in walls {
        block(scene, name, min, max, looks::WALL);
    }
}

const CRATES: &[(&str, f32, f32, f32, bool)] = &[
    ("Crate_South_1", -4.5, -15.5, 1.2, false),
    ("Crate_South_2", 5.0, -14.0, 1.2, false),
    ("Crate_South_3", -10.0, -19.5, 1.4, false),
    ("Crate_South_4", -10.0, -19.5, 1.0, true),
    ("Crate_North_Cover", 0.0, 18.0, 1.0, false),
    ("Crate_North_2", 6.0, 15.5, 1.2, false),
    ("Crate_North_3", -6.5, 18.0, 1.4, false),
    ("Crate_North_4", 10.0, 20.0, 1.2, false),
];

pub(super) fn add_crates(scene: &mut Scene) {
    for &(name, x, z, size, stacked) in CRATES {
        let floor = if stacked { DECK_TOP + 1.4 } else { DECK_TOP };
        let half = Vec3::new(size, 0.0, size) * 0.5;
        let min = Vec3::new(x, floor, z) - half;
        let max = Vec3::new(x, floor + size, z) + half;
        block(scene, name, min, max, looks::CRATE);
    }
}
