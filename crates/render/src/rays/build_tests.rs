use glam::Vec3;

use super::{build, Node, MAX_DEPTH};

fn boxes(n: usize, at: impl Fn(usize) -> Vec3) -> Vec<(Vec3, Vec3)> {
    (0..n).map(|i| (at(i), at(i) + Vec3::splat(0.5))).collect()
}

fn holds(nodes: &[Node], order: &[u32], prims: &[(Vec3, Vec3)]) {
    let mut seen = vec![0u32; prims.len()];
    let mut stack = vec![(0usize, 0usize)];
    while let Some((at, depth)) = stack.pop() {
        assert!(depth <= MAX_DEPTH, "depth {depth}");
        let n = nodes[at];
        let (lo, hi) = (Vec3::from_array(n.lo), Vec3::from_array(n.hi));
        let inside = |(l, h): (Vec3, Vec3)| l.cmpge(lo).all() && h.cmple(hi).all();
        if n.count == 0 {
            for child in [n.first as usize, n.first as usize + 1] {
                let c = nodes[child];
                assert!(inside((Vec3::from_array(c.lo), Vec3::from_array(c.hi))));
                stack.push((child, depth + 1));
            }
            continue;
        }
        for i in n.first..n.first + n.count {
            let prim = order[i as usize] as usize;
            assert!(inside(prims[prim]), "a leaf's bounds hold its items");
            seen[prim] += 1;
        }
    }
    assert!(
        seen.iter().all(|&s| s == 1),
        "every item in exactly one leaf"
    );
}

#[test]
fn a_tree_holds_every_item_once_inside_its_bounds() {
    for prims in [
        boxes(1, |_| Vec3::ZERO),
        boxes(3, |i| Vec3::X * i as f32),
        boxes(500, |i| {
            Vec3::new((i % 17) as f32, (i % 5) as f32, (i / 85) as f32)
        }),
        boxes(300, |i| Vec3::X * (1.7f32).powi(i as i32 % 60)),
    ] {
        let (nodes, order) = build(&prims);
        holds(&nodes, &order, &prims);
    }
}

#[test]
fn items_at_one_point_still_split_and_end() {
    let prims = boxes(1000, |_| Vec3::ONE);
    let (nodes, order) = build(&prims);
    holds(&nodes, &order, &prims);
    let biggest = nodes.iter().map(|n| n.count).max();
    assert!(biggest <= Some(8), "a median split keeps leaves small");
}

#[test]
fn nothing_builds_no_node() {
    assert_eq!(build(&[]), (Vec::new(), Vec::new()));
}
