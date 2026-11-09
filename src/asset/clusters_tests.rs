use super::*;

/// An `n`×`n` quad grid in the XZ plane facing +Y: its positions and indices.
pub(crate) fn grid(n: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = Vec::new();
    for z in 0..=n {
        for x in 0..=n {
            positions.push([x as f32, 0.0, z as f32]);
        }
    }
    let at = |x: u32, z: u32| z * (n + 1) + x;
    let mut indices = Vec::new();
    for z in 0..n {
        for x in 0..n {
            let (a, b, c, d) = (at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1));
            indices.extend([a, c, b, a, d, c]);
        }
    }
    (positions, indices)
}

/// Each triangle rotated to start at its least index (winding kept), sorted.
fn triangles(indices: &[u32]) -> Vec<[u32; 3]> {
    let mut out: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|t| {
            let k = (0..3).min_by_key(|&k| t[k]).unwrap();
            [t[k], t[(k + 1) % 3], t[(k + 2) % 3]]
        })
        .collect();
    out.sort_unstable();
    out
}

fn run(indices: Range<u32>) -> MaterialRange {
    MaterialRange {
        indices,
        material: None,
    }
}

#[test]
fn a_big_mesh_splits_into_small_clusters_over_the_same_triangles() {
    let (positions, mut indices) = grid(48);
    let before = indices.clone();
    let clusters = build(&positions, &mut indices, &[]).expect("4608 triangles");
    assert_eq!(
        triangles(&indices),
        triangles(&before),
        "same triangles, same winding"
    );
    let mut at = 0;
    for c in &clusters.list {
        assert_eq!(c.indices.start, at, "clusters are contiguous, in order");
        assert!(c.indices.len() <= 3 * MAX_TRIANGLES && !c.indices.is_empty());
        for &i in &indices[c.indices.start as usize..c.indices.end as usize] {
            let p = positions[i as usize];
            assert!(
                (0..3).all(|k| c.min[k] <= p[k] && p[k] <= c.max[k]),
                "{p:?}"
            );
        }
        at = c.indices.end;
    }
    assert_eq!(at as usize, indices.len(), "every index is in a cluster");
    let c = &clusters.list[0];
    assert!(
        c.cutoff < 1.0 && c.axis[1] > 0.99,
        "a flat grid's cone is +Y: {c:?}"
    );
}

#[test]
fn a_small_mesh_or_a_malformed_one_gets_none_and_keeps_its_order() {
    let (positions, mut small) = grid(8);
    let kept = small.clone();
    assert_eq!(build(&positions, &mut small, &[]), None);
    assert_eq!(small, kept);
    let (mut positions, mut indices) = grid(48);
    let kept = indices.clone();
    indices[7] = positions.len() as u32;
    assert_eq!(
        build(&positions, &mut indices, &[]),
        None,
        "a vertex it lacks"
    );
    indices[7] = kept[7];
    positions[3][1] = f32::NAN;
    assert_eq!(build(&positions, &mut indices, &[]), None, "a NaN position");
    assert_eq!(indices, kept);
    let (positions, mut indices) = grid(48);
    let gap = [run(0..300), run(303..indices.len() as u32)];
    assert_eq!(
        build(&positions, &mut indices, &gap),
        None,
        "runs with a gap"
    );
}

#[test]
fn no_cluster_crosses_a_material_run() {
    let (positions, mut indices) = grid(48);
    let half = (indices.len() / 2) as u32 + 3 * 7;
    let runs = [run(0..half), run(half..indices.len() as u32)];
    let before = indices.clone();
    let clusters = build(&positions, &mut indices, &runs).expect("big");
    let (lo, hi) = (half as usize, indices.len());
    assert_eq!(triangles(&indices[..lo]), triangles(&before[..lo]));
    assert_eq!(triangles(&indices[lo..hi]), triangles(&before[lo..hi]));
    for c in &clusters.list {
        assert!(c.indices.end <= half || c.indices.start >= half, "{c:?}");
    }
}
