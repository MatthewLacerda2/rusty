use super::*;
use crate::render::draw::batch::{BatchKey, RecordTraits};

fn batch(instances: Range<u32>, indices: Range<u32>, facing: Facing) -> DrawBatch {
    DrawBatch {
        key: BatchKey {
            pipeline: 0,
            facing,
            mesh: MeshHandle(0),
            indices: indices.into(),
            material: 0,
            record: 0,
            traits: RecordTraits::of(&crate::render::MaterialData::flat([1.0; 4])),
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
        state_first: 1,
        cone: 1,
        _pad: 0,
    };
    assert_eq!(plan.jobs[0], first);
    assert_eq!(plan.jobs[1].out_first, 600, "the next instance's room");
    assert_eq!((plan.jobs[1].tested, plan.jobs[1].state_first), (0, 6));
    assert_eq!(plan.jobs[2].cone, 0, "a two-sided batch takes no cone test");
    assert_eq!((plan.out_indices, plan.states, plan.tested), (1800, 16, 10));
    assert_eq!(plan.draws(3), vec![None, Some(0..2), Some(2..3)]);
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
