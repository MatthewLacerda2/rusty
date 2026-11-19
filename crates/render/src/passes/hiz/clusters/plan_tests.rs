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
    let group = |mesh, jobs, clusters| Group {
        mesh: MeshHandle(mesh),
        jobs,
        clusters,
    };
    let groups = vec![group(0, 0..1, 5), group(1, 1..3, 5), group(1, 3..4, 2)];
    assert_eq!(plan.groups, groups, "by mesh, then by cluster count");
    let own: Vec<_> = plan.jobs.iter().map(|j| j.clusters).collect();
    assert_eq!(own, [5, 5, 5, 2], "a job keeps its own batch's count");
    assert_eq!(plan.draws(3), vec![Some(1..3), Some(0..1), Some(3..4)]);
}

#[test]
fn batches_of_one_range_join_and_unequal_counts_start_no_padding() {
    let (ccw, both) = (Facing::Ccw, Facing::Both);
    // Two material slots of one mesh: 1,000 instances of 2 clusters, one of 500,
    // and a two-sided copy of the small range, sorted in beside it.
    let batches = [
        of_mesh(0, 0..1000, 0..60, ccw),
        of_mesh(0, 1000..1001, 60..15060, ccw),
        of_mesh(0, 1001..1003, 0..60, both),
    ];
    let all = instances(1003);
    let clusters_of = |b: &DrawBatch| {
        Some(if b.key.indices.start == 0 {
            0..2
        } else {
            2..502
        })
    };
    let plan = Plan::of(batches.iter(), (&all, &|_| true), clusters_of);
    let shape: Vec<_> = plan
        .groups
        .iter()
        .map(|g| (g.jobs.clone(), g.clusters))
        .collect();
    assert_eq!(shape, [(0..1002, 2), (1002..1003, 500)], "one range joins");
    // The test's threads and the copy's workgroups at most: a job's row a group.
    let rows: u32 = plan
        .groups
        .iter()
        .map(|g| g.jobs.len() as u32 * g.clusters)
        .sum();
    assert_eq!(
        rows,
        1002 * 2 + 500,
        "no job's row padded past its clusters"
    );
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
