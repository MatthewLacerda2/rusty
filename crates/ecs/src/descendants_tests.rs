use crate::{Changes, World, WorldPose};

fn walk(w: &World, id: u32) -> Vec<u32> {
    w.self_and_descendants(id).collect()
}

#[test]
fn descendants_run_pre_order_in_declared_order() {
    let mut w = World::new();
    let [root, a, a1, b] = ["root", "a", "a1", "b"].map(|n| w.spawn(n.into()));
    for (child, parent) in [(a, root), (a1, a), (b, root)] {
        w.set_parent_id(child, Some(parent));
        w.add_child(parent, child);
    }
    assert_eq!(walk(&w, root), vec![root, a, a1, b]);
    assert_eq!(walk(&w, a), vec![a, a1]);
    let pruned: Vec<u32> = w.self_and_descendants_while(root, |id| id != a).collect();
    assert_eq!(
        pruned,
        vec![root, a, b],
        "a is yielded, its children are not"
    );
}

#[test]
fn a_children_cycle_visits_each_id_once() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.add_child(a, b);
    w.add_child(b, a);
    w.add_child(a, b);
    assert_eq!(walk(&w, a), vec![a, b]);
}

#[test]
fn dead_ids_are_skipped() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let gone = w.spawn("gone".into());
    w.add_child(a, gone);
    w.despawn(gone);
    assert_eq!(walk(&w, a), vec![a]);
    assert!(walk(&w, gone).is_empty());
}

#[test]
fn a_hierarchy_path_reads_back_to_its_entity_among_same_names() {
    let mut w = World::new();
    let arena = w.spawn("Arena".into());
    let crates = [0; 5].map(|_| w.spawn("Crate".into()));
    let slash = w.spawn("a/b".into());
    for child in crates.into_iter().chain([slash]) {
        w.set_parent_id(child, Some(arena));
        w.add_child(arena, child);
    }
    let dup = w.spawn("Arena".into());
    for id in crates.into_iter().chain([arena, slash, dup]) {
        let path = w.path_of(id).unwrap();
        assert_eq!(w.find_path(&path), Some(id), "{path:?}");
    }
    assert_eq!(w.find_child(arena, "Crate"), Some(crates[0]));
    assert_eq!(w.path_of(99), None);
}

fn reference(w: &World, id: u32, descend: impl Fn(u32) -> bool) -> Vec<u32> {
    let (mut stack, mut seen, mut out) = (vec![id], std::collections::BTreeSet::new(), vec![]);
    while let Some(id) = stack.pop() {
        if !w.contains(id) || !seen.insert(id) {
            continue;
        }
        if descend(id) {
            stack.extend(w.children(id).into_iter().rev());
        }
        out.push(id);
    }
    out
}

fn malformed(lists: &[Vec<usize>]) -> (World, Vec<u32>) {
    let mut w = World::new();
    let ids: Vec<u32> = (0..lists.len() + 3)
        .map(|i| w.spawn(format!("e{i}")))
        .collect();
    for (parent, children) in lists.iter().enumerate() {
        for &c in children {
            w.add_child(ids[parent], ids[c % ids.len()]);
        }
    }
    ids[lists.len()..].iter().for_each(|&gone| w.despawn(gone));
    (w, ids)
}

proptest::proptest! {
    #[test]
    fn the_walk_yields_what_the_reference_walk_yields(
        lists in proptest::collection::vec(proptest::collection::vec(0usize..11, 0..4), 1..9),
        prune in 1u32..5,
    ) {
        let (mut w, ids) = malformed(&lists);
        let mut poses = w.change_cursor::<WorldPose>();
        for &id in &ids {
            proptest::prop_assert_eq!(walk(&w, id), reference(&w, id, |_| true));
            let pruned: Vec<u32> = w.self_and_descendants_while(id, |e| e % prune != 0).collect();
            proptest::prop_assert_eq!(pruned, reference(&w, id, |e| e % prune != 0));
            w.stamp_pose_below(id);
            let mut sorted = reference(&w, id, |_| true);
            sorted.sort_unstable();
            proptest::prop_assert_eq!(w.changes(&mut poses), Changes::Ids(sorted));
        }
    }
}

#[test]
fn two_walks_alive_at_once_each_yield_their_whole_subtree() {
    let (w, ids) = malformed(&[vec![1, 2], vec![2, 0], vec![1]]);
    let (mut outer, mut inner) = (vec![], vec![]);
    for e in w.self_and_descendants(ids[0]) {
        outer.push(e);
        inner.push(walk(&w, ids[1]));
    }
    assert_eq!(outer, reference(&w, ids[0], |_| true));
    assert!(inner.iter().all(|v| *v == reference(&w, ids[1], |_| true)));
}
