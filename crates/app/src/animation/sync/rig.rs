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

/// Each foot 0.15 of a cycle later than Walk's (0.09 and 0.39 of 0.6 s), so marker sync and
/// normalized sync differ.
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

/// Each clip keys its own joint, so a joint's x says which clip posed it.
pub fn rig_mesh() -> MeshComponent {
    mesh(vec![
        slide("Walk", &[HIPS], 0.0, 1.0, WALK),
        slide("Run", &[LEG], 0.0, 1.0, RUN),
        slide("Idle", &[SPINE], 0.0, 1.0, 1.0),
    ])
}

/// Run keys the hips too, so root motion reads both clips while the leg reads Run alone.
pub fn hips_mesh() -> MeshComponent {
    mesh(vec![
        slide("Walk", &[HIPS], 0.0, 1.0, WALK),
        slide("Run", &[HIPS, LEG], 0.0, 1.0, RUN),
        slide("Idle", &[SPINE], 0.0, 1.0, 1.0),
    ])
}

pub fn grouped(name: &str, group: Option<&str>, markers: Vec<SyncMarker>) -> GraphNode {
    GraphNode {
        sync_group: group.map(String::from),
        sync_markers: markers,
        ..node(name, name)
    }
}

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

pub fn assert_same(a: SyncPos, b: SyncPos, what: &str) {
    let (Some((a0, a1, af)), Some((b0, b1, bf))) = (a.pair, b.pair) else {
        panic!("{what}: no marker pair in {a:?} / {b:?}");
    };
    assert_eq!((a0, a1), (b0, b1), "{what}: {a:?} vs {b:?}");
    assert!((af - bf).abs() < 1e-3, "{what}: {a:?} vs {b:?}");
}
