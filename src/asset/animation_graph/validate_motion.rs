//! src/asset/animation_graph/validate_motion.rs — a node's motion is sound (#457).
//!
//! A node plays exactly one motion: a clip, or a blend tree. A tree needs at
//! least one child, every child exactly one of a clip and a nested tree (#1746),
//! every axis a declared `Float` parameter, and finite, distinct positions —
//! two children on one point would split the weight arbitrarily, so that is an
//! authoring error, not something to guess at. The checks recurse into nested
//! trees, each problem naming the tree's path (`Loco/children[2]`), down to
//! [`MAX_TREE_DEPTH`]; no cycle check is needed, since a tree holds its
//! children by value.

use super::event::check_events;
use super::{AnimationGraph, BlendTree, GraphNode, ParameterDeclaration, MAX_TREE_DEPTH};

impl AnimationGraph {
    pub(super) fn check_motion(&self, node: &GraphNode, problems: &mut Vec<String>) {
        let name = &node.name;
        match (&node.blend_tree, node.clip.is_empty()) {
            (None, true) => problems.push(format!("node '{name}' has an empty clip name")),
            (Some(_), false) => {
                problems.push(format!("node '{name}' has both a clip and a blend tree"))
            }
            (Some(tree), true) => {
                let label = format!("node '{name}' blend tree");
                self.check_tree(&label, name, tree, 1, problems)
            }
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

    /// Check `tree`, reported under `label`, at `path` and nesting `depth` (the
    /// node's own tree is depth 1).
    fn check_tree(
        &self,
        label: &str,
        path: &str,
        tree: &BlendTree,
        depth: usize,
        problems: &mut Vec<String>,
    ) {
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
        if tree.is_empty() {
            problems.push(format!("{label} has no children"));
        }
        let points: Vec<[f32; 2]> = tree.children().map(|(_, p)| p).collect();
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
        for (i, (motion, _)) in tree.children().enumerate() {
            let child = format!("{path}/children[{i}]");
            match (&motion.blend_tree, motion.clip.is_empty()) {
                (None, true) => problems.push(format!(
                    "{label} child '{child}' has neither a clip nor a blend tree"
                )),
                (Some(_), false) => problems.push(format!(
                    "{label} child '{child}' has both a clip and a blend tree"
                )),
                (Some(_), true) if depth >= MAX_TREE_DEPTH => problems.push(format!(
                    "{label} child '{child}' nests trees deeper than {MAX_TREE_DEPTH}"
                )),
                (Some(nested), true) => {
                    let nested_label = format!("blend tree '{child}'");
                    self.check_tree(&nested_label, &child, nested, depth + 1, problems);
                    if !motion.events.is_empty() {
                        problems.push(format!(
                            "{nested_label} has events of its own; put them on its children"
                        ));
                    }
                }
                (None, false) => check_events(
                    &format!("{label} child '{}'", motion.clip),
                    &motion.events,
                    problems,
                ),
            }
        }
    }
}
