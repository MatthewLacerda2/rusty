use super::*;
use crate::draw::batch::{BatchKey, RecordTraits};

fn batch(instances: Range<u32>, indices: Range<u32>, facing: Facing) -> DrawBatch {
    of_mesh(0, instances, indices, facing)
}

fn of_mesh(mesh: u32, instances: Range<u32>, indices: Range<u32>, facing: Facing) -> DrawBatch {
    DrawBatch {
        key: BatchKey {
            pipeline: 0,
            facing,
            mesh: MeshHandle(mesh),
            indices: indices.into(),
            material: 0,
            record: 0,
            traits: RecordTraits::of(&crate::MaterialData::flat([1.0; 4])),
            bone_base: 0,
        },
        uniform_slot: 0,
        instances,
    }
}

fn instances(n: u32) -> Vec<InstanceData> {
    (0..n)
        .map(|k| InstanceData {
            object: 100 + k,
            ..bytemuck::Zeroable::zeroed()
        })
        .collect()
}

#[test]
fn a_clustered_batch_gets_a_job_per_instance_with_its_own_room() {
    let batches = [
        batch(0..1, 0..36, Facing::Ccw),
        batch(1..3, 0..600, Facing::Ccw),
        batch(3..4, 0..600, Facing::Both),
    ];
    let all = instances(4);
    let odd = |i: usize| i % 2 == 1;
    let clusters_of = |b: &DrawBatch| (b.key.indices.len() == 600).then_some(4..9);
    let plan = Plan::of(batches.iter(), (&all, &odd), clusters_of);
    let first = Job {
        instance: 1,
        object: 101,
        tested: 1,
        out_first: 0,
        cluster_first: 4,
        state_first: 0,
        cone: 1,
        clusters: 5,
    };
    assert_eq!(plan.jobs[0], first);
    assert_eq!(plan.jobs[1].out_first, 600, "the next instance's room");
    assert_eq!((plan.jobs[1].tested, plan.jobs[1].state_first), (0, 11));
    assert_eq!(plan.jobs[2].cone, 0, "a two-sided batch takes no cone test");
    assert_eq!((plan.out_indices, plan.states, plan.tested), (1800, 33, 10));
    assert_eq!(plan.draws(3), vec![None, Some(0..2), Some(2..3)]);
    let one = Group {
        mesh: MeshHandle(0),
        jobs: 0..3,
        clusters: 5,
        pairs: 15,
    };
    assert_eq!(plan.groups, vec![one], "one mesh, one dispatch a stage");
}

#[test]
fn the_batches_of_one_mesh_share_a_dispatch_group() {
    let ccw = Facing::Ccw;
    let batches = [
        of_mesh(1, 0..2, 0..600, ccw),
        of_mesh(0, 2..3, 0..600, ccw),
        of_mesh(1, 3..4, 600..900, ccw),
    ];
    let all = instances(4);
    let clusters_of = |b: &DrawBatch| Some(if b.key.indices.start == 0 { 0..5 } else { 5..7 });
    let plan = Plan::of(batches.iter(), (&all, &|_| true), clusters_of);
    let group = |mesh, jobs, clusters, pairs| Group {
        mesh: MeshHandle(mesh),
        jobs,
        clusters,
        pairs,
    };
    // Mesh 1's 2- and 5-cluster batches join: 3 rows of 5 for 12 pairs.
    let groups = vec![group(0, 0..1, 5, 5), group(1, 1..4, 5, 12)];
    assert_eq!(plan.groups, groups, "by mesh, the most clusters of each");
    let own: Vec<_> = plan.jobs.iter().map(|j| j.clusters).collect();
    assert_eq!(own, [5, 2, 5, 5], "by cluster count; a job keeps its own");
    assert_eq!(plan.draws(3), vec![Some(2..4), Some(0..1), Some(1..2)]);
}

/// A group's jobs and clusters.
type Shape = (Range<u32>, u32);

/// The groups `batches` of one mesh plan into, as (jobs, clusters), and the
/// rows they start against their real pairs; `clusters_of` maps a batch's
/// first index to its cluster range.
fn shape(
    batches: &[DrawBatch],
    clusters_of: &dyn Fn(u32) -> Range<u32>,
) -> (Vec<Shape>, (u64, u64)) {
    let all = instances(batches.iter().map(|b| b.instances.end).max().unwrap_or(0));
    let ranges = |b: &DrawBatch| Some(clusters_of(b.key.indices.start));
    let plan = Plan::of(batches.iter(), (&all, &|_| true), ranges);
    let groups = plan.groups.iter().map(|g| (g.jobs.clone(), g.clusters));
    (groups.collect(), plan.rows_and_pairs())
}

#[test]
fn unequal_counts_join_only_within_the_padding_bound() {
    let (ccw, both) = (Facing::Ccw, Facing::Both);
    // Two material slots of one mesh: 1,000 instances of 2 clusters, one of 500,
    // and a two-sided copy of the small range, sorted in beside it.
    let batches = [
        of_mesh(0, 0..1000, 0..60, ccw),
        of_mesh(0, 1000..1001, 60..15060, ccw),
        of_mesh(0, 1001..1003, 0..60, both),
    ];
    let (groups, (rows, pairs)) = shape(&batches, &|i| if i == 0 { 0..2 } else { 2..502 });
    assert_eq!(groups, [(0..1002, 2), (1002..1003, 500)], "one range joins");
    assert_eq!(pairs, 1002 * 2 + 500);
    // The test's threads and the copy's workgroups at most: a job's row a group.
    assert!(rows <= MAX_PADDING * pairs, "{rows} rows for {pairs} pairs");
}

#[test]
fn near_equal_counts_join_into_one_dispatch_a_stage() {
    let ccw = Facing::Ccw;
    // Two material slots of one mesh, 40 and 60 clusters, 10 instances each.
    let batches = [
        of_mesh(0, 0..10, 900..1500, ccw),
        of_mesh(0, 10..20, 0..900, ccw),
    ];
    let (groups, (rows, pairs)) = shape(&batches, &|i| if i == 0 { 0..60 } else { 60..100 });
    assert_eq!(groups, [(0..20, 60)], "one group: one dispatch a stage");
    assert_eq!((rows, pairs), (20 * 60, 10 * 60 + 10 * 40));
}

#[test]
fn a_group_past_a_dispatch_limit_splits() {
    let max = MAX_DIMENSION;
    let ccw = Facing::Ccw;
    let batches = [batch(0..max, 0..3, ccw), batch(max..max + 1, 3..6, ccw)];
    let all = instances(max + 1);
    let plan = Plan::of(batches.iter(), (&all, &|_| true), |_| Some(0..1));
    assert_eq!(plan.runs.len(), 2, "each batch fits on its own");
    assert_eq!(plan.groups.len(), 2, "together past the scan's workgroups");
}

#[test]
fn a_batch_past_the_budget_is_drawn_whole() {
    let big = (MAX_OUT_INDICES / 2) as u32 + 3;
    let batches = [
        batch(0..1, 0..big, Facing::Ccw),
        batch(1..2, 0..big, Facing::Ccw),
    ];
    let all = instances(2);
    let plan = Plan::of(batches.iter(), (&all, &|_| true), |_| Some(0..1));
    assert_eq!(plan.runs.len(), 1, "the second would pass the budget");
    assert_eq!(plan.draws(2), vec![Some(0..1), None]);
    let none = Plan::of([].iter(), (&all, &|_| true), |_| Some(0..1));
    assert!(
        none.jobs.is_empty() && none.runs.is_empty(),
        "no batches, no jobs"
    );
}

#[test]
fn a_batch_none_of_whose_instances_is_judged_keeps_one_instanced_draw() {
    let batches = [
        batch(0..2, 0..600, Facing::Ccw),
        batch(2..4, 0..600, Facing::Ccw),
    ];
    let all = instances(4);
    let plan = Plan::of(batches.iter(), (&all, &|i| i == 3), |_| Some(0..5));
    assert_eq!(
        plan.draws(2),
        vec![None, Some(0..2)],
        "particles stay instanced"
    );
    assert_eq!((plan.jobs.len(), plan.tested), (2, 5));
}
