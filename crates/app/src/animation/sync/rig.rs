//! The #1896 sync tests' rig: a one-second Walk planting `FootL` at 0 and
//! `FootR` at 0.5, a 0.6 s Run planting them 0.15 of the way later (0.09 and
//! 0.39), both in the `Locomotion` group, on [`test_rig`](super::super::test_rig).

use rusty_asset::animation_graph::{
    AnimationGraph, BlendChild1D, BlendTree, ChildMotion, GraphNode, SyncMarker,
};
use rusty_components::{AnimatorComponent, MeshComponent};

use super::super::test_rig::*;
use super::markers::{position, SyncPos};

pub const WALK: f32 = 1.0;
pub const RUN: f32 = 0.6;
pub const DT: f32 = 1.0 / 60.0;

pub fn walk_marks() -> Vec<SyncMarker> {
    marks(&[("FootL", 0.0), ("FootR", 0.5)])
}

pub fn run_marks() -> Vec<SyncMarker> {
    marks(&[("FootL", 0.09), ("FootR", 0.39)])
}

pub fn marks(list: &[(&str, f32)]) -> Vec<SyncMarker> {
    list.iter()
        .map(|&(name, time)| SyncMarker {
            time,
            name: name.to_string(),
        })
        .collect()
}

/// Walk on the hips, Run on the leg, Idle on the spine.
pub fn rig_mesh() -> MeshComponent {
    mesh(vec![
        slide("Walk", &[HIPS], 0.0, 1.0, WALK),
        slide("Run", &[LEG], 0.0, 1.0, RUN),
        slide("Idle", &[SPINE], 0.0, 1.0, 1.0),
    ])
}

/// Walk slides the hips 1 m a cycle; Run, the hips and the leg 1 m over its
/// 0.6 s, so root motion reads both and the leg reads Run alone.
pub fn hips_mesh() -> MeshComponent {
    mesh(vec![
        slide("Walk", &[HIPS], 0.0, 1.0, WALK),
        slide("Run", &[HIPS, LEG], 0.0, 1.0, RUN),
        slide("Idle", &[SPINE], 0.0, 1.0, 1.0),
    ])
}

/// A looping node playing `clip` in `group` with `markers`.
pub fn grouped(name: &str, group: Option<&str>, markers: Vec<SyncMarker>) -> GraphNode {
    GraphNode {
        sync_group: group.map(String::from),
        sync_markers: markers,
        ..node(name, name)
    }
}

/// Walk, Run and Idle nodes; Walk and Run in `Locomotion` when `grouped_in`.
pub fn locomotion(grouped_in: bool) -> AnimationGraph {
    let group = grouped_in.then_some("Locomotion");
    graph(
        vec![
            grouped("Walk", group, walk_marks()),
            grouped("Run", group, run_marks()),
            node("Idle", "Idle"),
        ],
        Vec::new(),
    )
}

/// A 1D tree over `speed`: Walk at 0, Run at 1, each with its markers.
pub fn tree(walk: Vec<SyncMarker>, run: Vec<SyncMarker>) -> GraphNode {
    let child = |clip: &str, sync_markers, threshold| BlendChild1D {
        motion: ChildMotion {
            sync_markers,
            ..ChildMotion::clip(clip)
        },
        threshold,
    };
    GraphNode {
        clip: String::new(),
        blend_tree: Some(BlendTree::Simple1D {
            parameter: "speed".to_string(),
            children: vec![child("Walk", walk, 0.0), child("Run", run, 1.0)],
        }),
        ..node("Loco", "")
    }
}

/// An animator bound to `graph`'s entry.
pub fn bound(graph: &AnimationGraph) -> AnimatorComponent {
    let mut anim = AnimatorComponent::default();
    super::super::graph::step_graph(&mut anim, graph);
    anim
}

pub fn walk_at(t: f32) -> SyncPos<'static> {
    position(Box::leak(walk_marks().into_boxed_slice()), WALK, t, true)
}

pub fn run_at(t: f32) -> SyncPos<'static> {
    position(Box::leak(run_marks().into_boxed_slice()), RUN, t, true)
}

/// Two positions name the same pair and lie the same fraction along it.
pub fn assert_same(a: SyncPos, b: SyncPos, what: &str) {
    let (Some((a0, a1, af)), Some((b0, b1, bf))) = (a.pair, b.pair) else {
        panic!("{what}: no marker pair in {a:?} / {b:?}");
    };
    assert_eq!((a0, a1), (b0, b1), "{what}: {a:?} vs {b:?}");
    assert!((af - bf).abs() < 1e-3, "{what}: {a:?} vs {b:?}");
}
