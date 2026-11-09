//! `NavMeshModifier` edits (#1959) rebake only the cells of the colliders they
//! cover, to exactly a full bake, in the editor's `sync` and in Play's
//! `sync_changes`, which must not skip a modifier on a non-static parent.

use super::super::super::test_support::assert_same_mesh;
use super::super::super::{CellRect, Rebake};
use super::object_modifier::{baked, not_walkable, room, set};
use crate::scene::Scene;

/// The table covers cells 9..=11 × 4..=6 (with slack); erosion is off.
fn inside(r: &CellRect) -> bool {
    r.x0 >= 8 && r.x1 <= 12 && r.z0 >= 3 && r.z1 <= 7
}

fn edits(table: u32, top: u32) -> [Box<dyn Fn(&mut Scene)>; 5] {
    [
        Box::new(move |s| set(s, table, not_walkable())),
        Box::new(move |s| s.world.nav_mesh_modifier_mut(table).unwrap().override_area = Some(3)),
        Box::new(move |s| {
            s.world
                .nav_mesh_modifier_mut(table)
                .unwrap()
                .ignore_from_build = true
        }),
        Box::new(move |s| set(s, top, not_walkable())),
        Box::new(move |s| {
            s.world.set_nav_mesh_modifier(table, None);
            s.world.set_nav_mesh_modifier(top, None);
        }),
    ]
}

#[test]
fn modifier_edits_dirty_only_the_tables_cells_and_match_a_full_bake() {
    for play in [false, true] {
        let (mut scene, table, top) = room();
        assert!(!scene.world.is_static(table), "the root is a plain empty");
        let mut g = baked(&scene);
        if play {
            g.start_tracked(&mut scene);
        }
        for (i, edit) in edits(table, top).iter().enumerate() {
            edit(&mut scene);
            let outcome = if play {
                g.sync_changes(&mut scene)
            } else {
                g.sync(&scene)
            };
            let Rebake::Incremental(rects) = outcome else {
                panic!("play {play}, edit {i}: expected an incremental rebake, got {outcome:?}");
            };
            assert!(rects.iter().all(inside), "play {play}, edit {i}: {rects:?}");
            assert_same_mesh(&g, &baked(&scene));
        }
        assert!(
            (g.spans_at(10, 5)[0].y - 0.8).abs() < 1e-4,
            "back to a walkable top"
        );
    }
}
