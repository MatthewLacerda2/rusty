//! src/asset/animation_graph/validate_motion.rs — a node's motion is sound (#457).
//!
//! A node plays exactly one motion: a clip, or a blend tree. A tree needs at
//! least one child, every child a clip, every axis a declared `Float` parameter,
//! and finite, distinct positions — two children on one point would split the
//! weight arbitrarily, so that is an authoring error, not something to guess at.

use super::event::check_events;
use super::{AnimationGraph, BlendTree, GraphNode, ParameterDeclaration};

impl AnimationGraph {
    pub(super) fn check_motion(&self, node: &GraphNode, problems: &mut Vec<String>) {
        let name = &node.name;
        match (&node.blend_tree, node.clip.is_empty()) {
            (None, true) => problems.push(format!("node '{name}' has an empty clip name")),
            (Some(_), false) => {
                problems.push(format!("node '{name}' has both a clip and a blend tree"))
            }
            (Some(tree), true) => self.check_tree(name, tree, problems),
            (None, false) => {}
        }
        let label = format!("node '{name}'");
        if node.blend_tree.is_some() && !node.events.is_empty() {
            problems.push(format!(
                "{label} is a blend tree with events of its own; put them on its children"
            ));
        }
        check_events(&label, &node.events, problems);
    }

    fn check_tree(&self, node: &str, tree: &BlendTree, problems: &mut Vec<String>) {
        let label = format!("node '{node}' blend tree");
        for parameter in tree.parameters() {
            if !matches!(
                self.parameters.get(parameter),
                Some(ParameterDeclaration::Float(_))
            ) {
                problems.push(format!(
                    "{label} reads '{parameter}', which is not a declared Float parameter"
                ));
            }
        }
        let clips = tree.clips();
        if clips.is_empty() {
            problems.push(format!("{label} has no children"));
        }
        if clips.iter().any(|c| c.is_empty()) {
            problems.push(format!("{label} has a child with an empty clip name"));
        }
        let points = positions(tree);
        if points.iter().flatten().any(|v| !v.is_finite()) {
            problems.push(format!("{label} has a non-finite child position"));
        }
        let duplicated = points
            .iter()
            .enumerate()
            .any(|(i, p)| points[..i].contains(p));
        if duplicated {
            problems.push(format!("{label} has two children at the same position"));
        }
        for (clip, events) in clips.iter().zip(tree.child_events()) {
            check_events(&format!("{label} child '{clip}'"), events, problems);
        }
    }
}

/// Every child's position as an `[x, y]` point (a 1D threshold sits on the x axis).
fn positions(tree: &BlendTree) -> Vec<[f32; 2]> {
    match tree {
        BlendTree::Simple1D { children, .. } => {
            children.iter().map(|c| [c.threshold, 0.0]).collect()
        }
        BlendTree::FreeformDirectional2D { children, .. }
        | BlendTree::FreeformCartesian2D { children, .. } => {
            children.iter().map(|c| c.position).collect()
        }
    }
}
