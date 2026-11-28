use super::*;

#[test]
fn new_is_one_flat_open_span_per_cell() {
    let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    assert_eq!((g.width, g.height), (11, 11));
    assert_eq!(g.spans.len(), 121);
    assert_eq!(g.span_count(), 121);
    assert!(g
        .spans
        .iter()
        .all(|s| s.y == 0.0 && s.ceiling.is_infinite()));
    assert_eq!(g.spans_at(3, 2).len(), 1);
    assert!(g.spans_at(-1, 0).is_empty(), "out of bounds holds nothing");
}

#[test]
fn span_world_carries_the_floor_height() {
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    let r = g.span_range(4, 5);
    g.spans[r.start].y = 2.5;
    let s = SpanRef {
        gx: 4,
        gz: 5,
        index: r.start as u32,
    };
    assert_eq!(g.span_world(s), Vec3::new(4.0, 2.5, 5.0));
}

#[test]
fn world_to_grid_rounds_to_nearest_cell() {
    let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    assert_eq!(g.world_to_grid(Vec3::new(3.0, 0.0, 5.0)), (3, 5));
    assert_eq!(g.world_to_grid(Vec3::new(3.6, 0.0, 5.4)), (4, 5));
    assert_eq!(g.world_to_grid(Vec3::new(3.4, 0.0, 4.6)), (3, 5));
    let g2 = NavigationGraph::new(2.0, 12.0, 2.0, 12.0, 1.0);
    assert_eq!(g2.world_to_grid(Vec3::new(5.0, 0.0, 7.0)), (3, 5));
}

#[test]
fn index_is_row_major() {
    let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    assert_eq!(g.index(0, 0), 0);
    assert_eq!(g.index(1, 0), 1);
    assert_eq!(g.index(0, 1), 11);
    assert_eq!(g.index(3, 2), 25);
}

#[test]
fn span_refs_walks_every_span_with_its_cell() {
    let g = NavigationGraph::new(0.0, 2.0, 0.0, 1.0, 1.0);
    let refs: Vec<_> = g.span_refs().collect();
    assert_eq!(refs.len(), 6);
    assert_eq!((refs[4].gx, refs[4].gz, refs[4].index), (1, 1, 4));
}

fn span(y: f32) -> NavSpan {
    NavSpan {
        y,
        ..NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 1.0).spans[0]
    }
}

/// Two 16-column tiles side by side, one span a column.
fn two_tiles() -> NavigationGraph {
    let g = NavigationGraph::new(0.0, 31.0, 0.0, 0.0, 1.0);
    let mut g = NavigationGraph {
        tiles: TileGrid::new(g.tiles.origin, g.width, g.height, 16),
        spans: Vec::new(),
        ..g
    };
    (0..32).for_each(|gx| g.set_column(gx, 0, &[span(gx as f32)]));
    g
}

#[test]
fn a_tile_that_outgrows_its_range_moves_and_leaves_its_neighbour_be() {
    let mut g = two_tiles();
    assert_eq!((g.tile_count(), g.span_count(), g.tiles.dead), (2, 32, 0));
    g.set_column(3, 0, &[span(3.0), span(9.0)]);
    assert_eq!(
        g.tiles.tile((0, 0)).unwrap().base,
        32,
        "moved past the other tile"
    );
    assert_eq!(
        g.tiles.tile((1, 0)).unwrap().base,
        16,
        "the other kept its ids"
    );
    assert_eq!((g.span_count(), g.tiles.dead as usize), (33, 16));
    assert_eq!(g.spans_at(3, 0), &[span(3.0), span(9.0)]);
    g.set_column(3, 0, &[]);
    assert_eq!(
        g.tiles.tile((0, 0)).unwrap().base,
        32,
        "a shrink stays in place"
    );
    assert_eq!((g.span_count(), g.tiles.dead as usize), (31, 18));
    assert!(!g.tiles.wasteful(&g.spans), "18 dead of 49");
    g.tiles.compact(&mut g.spans);
    assert_eq!((g.spans.len(), g.tiles.dead), (31, 0));
    assert_eq!(
        g.tiles.tile((0, 0)).unwrap().base,
        0,
        "key order once compacted"
    );
    assert_eq!(g.spans_at(4, 0), &[span(4.0)]);
    assert!(g.spans_at(3, 0).is_empty());
}

#[test]
fn removing_the_last_tile_returns_its_ids() {
    let mut g = two_tiles();
    assert!(g.remove_tile((1, 0)));
    assert_eq!(
        (g.spans.len(), g.tiles.dead),
        (16, 0),
        "truncated, nothing dead"
    );
    assert!(g.spans_at(20, 0).is_empty());
    assert!(!g.remove_tile((1, 0)), "nothing left to remove");
    assert!(g.remove_tile((0, 0)));
    assert_eq!((g.spans.len(), g.tile_count()), (0, 0));
}

#[test]
fn the_locator_names_each_id_s_cell() {
    let mut g = two_tiles();
    g.set_column(20, 0, &[span(1.0), span(5.0)]);
    let locate = g.tiles.locator();
    for s in g.span_refs() {
        let offset = s.index - g.span_range(s.gx, s.gz).start as u32;
        assert_eq!(locate(s.index), Some((s.gx, s.gz, offset)));
    }
    let base = g.tiles.tile((1, 0)).unwrap().base;
    assert_eq!(
        locate(base),
        Some((16, 0, 0)),
        "a tile's base is its first column's"
    );
    assert_eq!(locate(u32::MAX), None);
}

#[test]
fn a_tile_side_is_a_whole_number_of_cells_from_16() {
    assert_eq!(tile_side(32.0, 0.25), 128);
    assert_eq!(tile_side(1.0, 0.25), 16, "Unity's floor");
    assert_eq!(tile_side(f32::NAN, 0.25), 128, "the default");
    assert_eq!(tile_side(-3.0, 1.0), 32);
    assert_eq!(tile_side(1e9, 0.25), MAX_TILE_SIDE, "Unity's ceiling");
}
