//! crates/asset/src/animation_graph/sync.rs — sync groups and sync markers (#1896).
//!
//! Unreal's model, authored on the graph beside a clip's `events` (glTF can't
//! carry them either). A **sync marker** names a phase point on a clip
//! (`"FootL"` at 0.02 s): it fires nothing, it only says where in the cycle
//! the clip is. A node naming a **sync group** keeps step with the motion it
//! crossfades from when that one is in the same group (within a layer): the
//! incoming motion starts at the matching phase and the lighter of the two
//! follows the heavier while they blend. Inside a blend tree whose every leaf
//! carries markers, the leaves are matched marker to marker. Playing it is the
//! runtime's job (`app::animation::sync`); this is the authored data and its
//! load checks.

use serde::{Deserialize, Serialize};

use super::{AnimationGraph, BlendTree, MAX_TREE_DEPTH};

/// One phase point on a clip: a time in the clip's own seconds and its name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncMarker {
    /// Seconds into the clip (unscaled by any playback speed).
    pub time: f32,
    pub name: String,
}

/// Every problem with `markers`, each message led by `label`: an empty name, a
/// negative or non-finite time, and two markers sharing a name *and* a time.
pub(super) fn check_markers(label: &str, markers: &[SyncMarker], problems: &mut Vec<String>) {
    for (i, marker) in markers.iter().enumerate() {
        if marker.name.is_empty() {
            problems.push(format!("{label} has a sync marker with an empty name"));
        }
        if !marker.time.is_finite() || marker.time < 0.0 {
            problems.push(format!(
                "{label} sync marker '{}' has a negative or non-finite time",
                marker.name
            ));
        }
        if markers[..i].contains(marker) {
            problems.push(format!(
                "{label} has two sync markers '{}' at {}",
                marker.name, marker.time
            ));
        }
    }
}

/// `markers` in time order; a stable sort, so markers sharing a time keep
/// their authored order.
fn sort(markers: &mut [SyncMarker]) {
    markers.sort_by(|a, b| a.time.total_cmp(&b.time));
}

impl BlendTree {
    /// Whether every leaf clip of this tree, down through nested trees, carries
    /// sync markers: only then are its leaves matched marker to marker (#1896);
    /// otherwise they share one normalized phase.
    pub fn marker_synced(&self) -> bool {
        let (mut marked, mut bare) = (0, 0);
        count_leaves(self, 1, &mut marked, &mut bare);
        marked > 0 && bare == 0
    }

    /// Whether some leaves carry markers and others don't: the tree falls back
    /// to normalized sync, which the load warns about.
    fn partly_marked(&self) -> bool {
        let (mut marked, mut bare) = (0, 0);
        count_leaves(self, 1, &mut marked, &mut bare);
        marked > 0 && bare > 0
    }

    fn sort_markers(&mut self) {
        let visit = |motion: &mut super::ChildMotion| {
            sort(&mut motion.sync_markers);
            if let Some(nested) = &mut motion.blend_tree {
                nested.sort_markers();
            }
        };
        match self {
            BlendTree::Simple1D { children, .. } => {
                children.iter_mut().for_each(|c| visit(&mut c.motion))
            }
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => {
                children.iter_mut().for_each(|c| visit(&mut c.motion))
            }
        }
    }
}

/// Count `tree`'s leaf clips with and without sync markers, to [`MAX_TREE_DEPTH`].
fn count_leaves(tree: &BlendTree, depth: usize, marked: &mut usize, bare: &mut usize) {
    for (motion, _) in tree.children() {
        match &motion.blend_tree {
            Some(nested) if depth < MAX_TREE_DEPTH => count_leaves(nested, depth + 1, marked, bare),
            Some(_) => {}
            None if motion.sync_markers.is_empty() => *bare += 1,
            None => *marked += 1,
        }
    }
}

impl AnimationGraph {
    /// Sort every marker list by time (the load does, so the runtime and a
    /// saved file see them in order).
    pub fn sort_sync_markers(&mut self) {
        let machines =
            std::iter::once(&mut self.base).chain(self.layers.iter_mut().map(|l| &mut l.machine));
        for machine in machines {
            for node in &mut machine.nodes {
                sort(&mut node.sync_markers);
                if let Some(tree) = &mut node.blend_tree {
                    tree.sort_markers();
                }
            }
        }
    }

    /// What loads but plays other than authored: a blend tree with sync
    /// markers on some leaves and not others, which falls back to normalized
    /// sync. One message each, for the console.
    pub fn warnings(&self) -> Vec<String> {
        let machines = std::iter::once(("", &self.base))
            .chain(self.layers.iter().map(|l| (l.name.as_str(), &l.machine)));
        let mut out = Vec::new();
        for (layer, machine) in machines {
            for node in &machine.nodes {
                if node
                    .blend_tree
                    .as_ref()
                    .is_some_and(BlendTree::partly_marked)
                {
                    let prefix = if layer.is_empty() {
                        String::new()
                    } else {
                        format!("layer '{layer}': ")
                    };
                    out.push(format!(
                        "{prefix}node '{}' blend tree has sync markers on some leaves only; it syncs by normalized time",
                        node.name
                    ));
                }
            }
        }
        out
    }
}
