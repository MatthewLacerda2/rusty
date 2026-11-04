//! src/ecs/despawn/index_tests.rs — the reverse index against the full scan it
//! replaced (#1708): after any sequence of edits (`changes::ops_tests`'s, plus
//! writes to every reference-holding column, a skinned mesh's bone slots, a
//! reference to the holder itself or to an id nobody has), the holders it names for
//! any dead set are exactly the scan's, and a despawn leaves no live holder naming
//! a dead id.

use std::collections::BTreeSet;

use proptest::prelude::*;

use crate::components::{with_ref_components, EntityRefs, MeshComponent};
use crate::ecs::changes::ops_tests::{self, apply};
use crate::ecs::{Channel, World};

/// Give `id` a `T` whose every slot names `target`, or take its `T` away.
fn hold<T: EntityRefs + Channel + Send + Sync>(w: &mut World, id: u32, target: Option<u32>) {
    let held = target.map(|t| {
        let mut c = T::every_ref();
        c.visit_refs(&mut |_, r| *r = Some(t));
        c
    });
    w.set_component::<T>(id, held);
}

macro_rules! kinds {
    ($($field:ident: $ty:ty,)*) => {
        const KINDS: &[fn(&mut World, u32, Option<u32>)] = &[$(hold::<$ty>,)*];
    };
}
with_ref_components!(kinds);

#[derive(Clone, Debug)]
enum Target {
    At(usize),
    Itself,
    /// An id no entity has had yet.
    Unborn,
    Drop,
}

#[derive(Clone, Debug)]
enum Op {
    Base(ops_tests::Op),
    Hold(usize, usize, Target),
    /// A skinned mesh whose bone slots name these, at their indices.
    Bones(usize, Vec<usize>),
    /// A batch: these entities and every descendant of the first.
    Despawn(Vec<usize>),
}

fn op() -> impl Strategy<Value = Op> {
    let at = 0usize..10;
    let target = prop_oneof![
        4 => at.clone().prop_map(Target::At),
        1 => Just(Target::Itself),
        1 => Just(Target::Unborn),
        1 => Just(Target::Drop),
    ];
    let bones = proptest::collection::vec(at.clone(), 0..3);
    prop_oneof![
        6 => ops_tests::op().prop_map(Op::Base),
        5 => (at.clone(), 0..KINDS.len(), target).prop_map(|(h, k, t)| Op::Hold(h, k, t)),
        2 => (at.clone(), bones).prop_map(|(h, b)| Op::Bones(h, b)),
        2 => proptest::collection::vec(at, 1..3).prop_map(Op::Despawn),
    ]
}

fn pick(w: &World, i: usize) -> Option<u32> {
    w.ids().get(i).copied()
}

/// The index and the scan agree on every single live id, and on `extra`.
fn agree(w: &mut World, extra: &BTreeSet<u32>) -> Result<(), TestCaseError> {
    let mut sets: Vec<BTreeSet<u32>> = w.ids().iter().map(|&id| BTreeSet::from([id])).collect();
    sets.push(extra.clone());
    for dead in sets {
        let by_index = w.with_ref_index(|index, _| index.naming(&dead));
        prop_assert_eq!(by_index, w.naming_by_scan(&dead), "dead {:?}", dead);
    }
    Ok(())
}

fn run(w: &mut World, op: &Op) -> Result<(), TestCaseError> {
    match op {
        Op::Base(op) => apply(w, op),
        Op::Hold(h, kind, target) => {
            let Some(holder) = pick(w, *h) else {
                return Ok(());
            };
            let target = match *target {
                Target::At(i) => pick(w, i),
                Target::Itself => Some(holder),
                Target::Unborn => Some(w.next_id() + 5),
                Target::Drop => None,
            };
            KINDS[*kind](w, holder, target);
        }
        Op::Bones(h, bones) => {
            let Some(holder) = pick(w, *h) else {
                return Ok(());
            };
            let mut mesh = MeshComponent::every_ref();
            mesh.skeleton.bones = bones.iter().map(|&i| pick(w, i)).collect();
            w.set_component(holder, Some(mesh));
        }
        Op::Despawn(at) => {
            let mut dead: BTreeSet<u32> = at.iter().filter_map(|&i| pick(w, i)).collect();
            let below = dead.first().map(|&root| w.self_and_descendants(root));
            dead.extend(below.into_iter().flatten().collect::<Vec<_>>());
            agree(w, &dead)?;
            w.despawn_all(&dead);
            for &holder in w.ids() {
                let held = w.held_by(holder);
                prop_assert!(
                    held.iter().all(|r| !dead.contains(r)),
                    "{} kept a dead id",
                    holder
                );
            }
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn the_index_names_what_the_scan_names(ops in proptest::collection::vec(op(), 1..60)) {
        let mut w = World::new();
        for op in &ops {
            run(&mut w, op)?;
            if matches!(op, Op::Base(ops_tests::Op::Read)) {
                agree(&mut w, &BTreeSet::new())?;
            }
        }
        agree(&mut w, &BTreeSet::new())?;
    }
}
