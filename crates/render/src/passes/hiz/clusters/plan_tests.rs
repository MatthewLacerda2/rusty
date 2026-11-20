use super::*;
use crate::draw::batch::{BatchKey, RecordTraits};
use crate::scene::MeshHandle;

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
    let chunks = [[0, 0], [1, 0], [2, 0]];
    assert_eq!(plan.chunks, chunks, "five clusters: a chunk a job");
}

#[test]
fn meshes_of_unequal_counts_plan_one_chunk_list_with_no_padding() {
    let ccw = Facing::Ccw;
    // Three meshes, 2, 64 and 150 clusters each, the shared table's ranges.
    let batches = [
        of_mesh(2, 0..2, 0..9000, ccw),
        of_mesh(0, 2..3, 0..60, ccw),
        of_mesh(1, 3..4, 0..3000, ccw),
    ];
    let all = instances(4);
    let clusters_of = |b: &DrawBatch| {
        Some(match b.key.mesh.0 {
            0 => 10..12,
            1 => 12..76,
            _ => 200..350,
        })
    };
    let plan = Plan::of(batches.iter(), (&all, &|_| true), clusters_of);
    let own: Vec<_> = plan
        .jobs
        .iter()
        .map(|j| (j.cluster_first, j.clusters))
        .collect();
    assert_eq!(
        own,
        [(200, 150), (200, 150), (10, 2), (12, 64)],
        "in batch order"
    );
    // A chunk a 64 clusters of a job: 3 + 3 + 1 + 1, none for a cluster past it.
    let chunks = [
        [0, 0],
        [0, 64],
        [0, 128],
        [1, 0],
        [1, 64],
        [1, 128],
        [2, 0],
        [3, 0],
    ];
    assert_eq!(plan.chunks, chunks);
    let covered: u32 = plan.jobs.iter().map(|j| j.clusters.div_ceil(CHUNK)).sum();
    assert_eq!(covered as usize, plan.chunks.len());
    assert_eq!(plan.draws(3), vec![Some(0..2), Some(2..3), Some(3..4)]);
}

#[test]
fn as_many_instances_as_a_dispatch_row_holds_and_more_plan_whole() {
    // Past a dispatch dimension's 65,535 workgroups, the dispatch takes a
    // second (`record::workgroups`): no batch is dropped for it.
    let n = 65_536;
    let batches = [batch(0..n, 0..3, Facing::Ccw)];
    let all = instances(n);
    let plan = Plan::of(batches.iter(), (&all, &|_| true), |_| Some(0..1));
    assert_eq!(
        (plan.jobs.len(), plan.chunks.len()),
        (n as usize, n as usize)
    );
    assert_eq!(plan.draws(1), vec![Some(0..n)]);
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
