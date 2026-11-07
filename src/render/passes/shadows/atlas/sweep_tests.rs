//! Which atlas tiles cull by their light's PVS row (#1714, part 4), adapter-free:
//! a tile whose statics are drawn into it takes its light's row, and a
//! shadowmasked one (`statics: false`), whose walls are the mask's, takes none.

use glam::camera::rh::view::look_at_mat4;
use glam::Vec3;

use super::sweep::volume;
use super::Tile;
use crate::render::scene::occlusion_fixture::{baked, ROOM_1};
use crate::render::scene::OcclusionBinding;

/// A tile of a lamp at `light`, its statics drawn into it or not.
fn tile(light: Vec3, statics: bool) -> Tile {
    Tile {
        view_proj: look_at_mat4(light, light + Vec3::X, Vec3::Y),
        origin: [0, 0],
        size: 256,
        texel: 0.01,
        light,
        statics,
    }
}

#[test]
fn only_a_tile_whose_statics_are_drawn_culls_by_its_lights_row() {
    let pvs = OcclusionBinding::bound(&baked());
    let row = pvs.shadow_row(ROOM_1);
    assert!(row.is_some(), "the lamp stands in a cell of a usable bake");
    let drawn = volume(&pvs, &tile(ROOM_1, true));
    assert_eq!(drawn.row, row);
    assert_eq!(drawn.view_proj, tile(ROOM_1, true).view_proj);
    let masked = volume(&pvs, &tile(ROOM_1, false));
    assert_eq!(
        masked.row, None,
        "a shadowmasked tile's walls are not in its map"
    );
    // No bake at all: nothing culls, whatever the tile.
    let none = OcclusionBinding::default();
    assert_eq!(volume(&none, &tile(ROOM_1, true)).row, None);
}
