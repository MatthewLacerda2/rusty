use super::tests::{at, meshes, ordered as batched};
use super::{DrawItem, FrameDraws, OpaqueOrder};

fn lit(mesh: &str, tag: f32, depth: f32) -> DrawItem {
    let mut item = at(mesh, tag, depth);
    item.key.traits.lit = true;
    item
}

fn unlit(mesh: &str, tag: f32, depth: f32) -> DrawItem {
    let mut item = at(mesh, tag, depth);
    item.key.traits.lit = false;
    item
}

/// `opaque` batched instanced, its runs ranked by `order`.
fn ordered(opaque: Vec<DrawItem>, order: OpaqueOrder) -> FrameDraws {
    batched((opaque, Vec::new()), (true, order))
}

#[test]
fn over_a_primed_depth_the_unlit_panel_draws_first_whichever_mesh_was_interned_first() {
    for (wall, panel) in [("A", "B"), ("B", "A")] {
        let opaque = vec![lit(wall, 0.0, 10.5), unlit(panel, 1.0, 10.0)];
        let draws = ordered(opaque, OpaqueOrder::OwnDepthFirst);
        assert_eq!(meshes(&draws), [panel, wall], "wall {wall}, panel {panel}");
    }
    let opaque = vec![lit("Near", 0.0, 2.0), unlit("Far", 1.0, 80.0)];
    let draws = ordered(opaque, OpaqueOrder::OwnDepthFirst);
    assert_eq!(meshes(&draws), ["Far", "Near"]);
}

#[test]
fn an_unprimed_pass_keeps_the_key_order_inside_a_bucket() {
    let opaque = vec![lit("B", 0.0, 10.5), unlit("A", 1.0, 10.0)];
    assert_eq!(meshes(&ordered(opaque, OpaqueOrder::Unranked)), ["A", "B"]);
    let opaque = vec![lit("A", 0.0, 10.5), unlit("B", 1.0, 10.0)];
    assert_eq!(meshes(&ordered(opaque, OpaqueOrder::Unranked)), ["A", "B"]);
}

#[test]
fn the_rank_splits_no_run() {
    let opaque = || {
        vec![
            lit("Crate", 0.0, 50.0),
            unlit("Box", 1.0, 30.0),
            lit("Crate", 2.0, 3.0),
            lit("Wall", 3.0, 10.0),
            unlit("Box", 4.0, 60.0),
        ]
    };
    let primed = ordered(opaque(), OpaqueOrder::OwnDepthFirst);
    let unprimed = ordered(opaque(), OpaqueOrder::Unranked);
    assert_eq!(primed.opaque.len(), unprimed.opaque.len());
    assert_eq!(meshes(&primed), ["Box", "Crate", "Wall"]);
    assert_eq!(meshes(&unprimed), ["Crate", "Wall", "Box"]);
    assert!(primed.opaque.ranked_for(true) && !primed.opaque.ranked_for(false));
}

#[test]
fn without_instancing_the_rank_holds_and_the_given_order_inside_it() {
    // One draw per item, so no sort by key or depth: only the rank moves them,
    // and the batches record the order they are really in.
    let opaque = vec![
        lit("Wall", 0.0, 10.0),
        unlit("Box", 1.0, 30.0),
        lit("Crate", 2.0, 3.0),
        unlit("A", 3.0, 5.0),
    ];
    let draws = batched((opaque, Vec::new()), (false, OpaqueOrder::OwnDepthFirst));
    assert_eq!(meshes(&draws), ["Box", "A", "Wall", "Crate"]);
    assert!(draws.opaque.ranked_for(true) && !draws.opaque.ranked_for(false));
}
