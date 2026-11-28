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
