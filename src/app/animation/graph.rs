//! src/app/animation/graph.rs — the animation-graph runtime evaluator (#316).
//!
//! Each fixed step, for every active entity whose [`AnimatorComponent`] references
//! an `AnimationGraph` (#315) with evaluation enabled: walk the **active node's**
//! outgoing edges in authored (priority) order and fire the **first** edge whose
//! conditions all hold against the component's parameter values (#314) — crossfade
//! into the target's clip over the edge's `transition_duration` (reusing
//! `AnimatorComponent::crossfade`), adopt the target's `is_loop` and per-node
//! speed, and consume any `Trigger` the fired edge read (read-and-clear, exactly
//! once). No satisfied edge keeps the current node playing. An unbound animator
//! (fresh, or whose active node vanished from the graph) binds first: declared
//! parameter defaults are seeded and the entry node starts.
//!
//! Purity / determinism: evaluation is a pure function of (graph, parameter
//! values, active state) — no wall clock, no RNG — and every container walked is
//! ordered (`Vec` in authored order, `BTreeMap`), so a fixed-dt replay is
//! byte-reproducible. Graph assets load lazily by path into the [`GraphCache`]
//! (cleared on each Play enter so asset edits are picked up per session); a
//! failing load is reported to the console once and cached as absent.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use crate::asset::animation_graph::{self, AnimationGraph, Condition, GraphEdge, NumericOp};
use crate::components::AnimatorComponent;
use crate::scripting::ConsoleLogs;

use super::super::{Resources, World};

/// Lazily-loaded `AnimationGraph` assets keyed by path, shared by every entity
/// referencing the same graph (the asset is path-referenced by design, #315).
/// `None` records a failed load so the error is surfaced once, not every step.
#[derive(Default)]
pub struct GraphCache {
    loaded: BTreeMap<String, Option<AnimationGraph>>,
}

impl GraphCache {
    /// The graph at `path`, loading (and validating) it on first request. A load
    /// failure is logged to `console` once and remembered as absent.
    pub fn get_or_load(
        &mut self,
        path: &str,
        console: &Rc<RefCell<ConsoleLogs>>,
    ) -> Option<&AnimationGraph> {
        if !self.loaded.contains_key(path) {
            let result = animation_graph::load(Path::new(path));
            if let Err(err) = &result {
                console
                    .borrow_mut()
                    .error(format!("Animation graph '{path}': {err}"));
            }
            self.loaded.insert(path.to_string(), result.ok());
        }
        self.loaded.get(path).and_then(Option::as_ref)
    }

    /// Drop every cached graph (called on Play enter), so per-session edits to
    /// the asset files are picked up on the next evaluation.
    pub fn clear(&mut self) {
        self.loaded.clear();
    }
}

/// The evaluator system (`FixedUpdate`, right before `animate` samples): step
/// every active entity's graph-driven animator one transition decision.
pub fn evaluate_graphs(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    for id in scene.entity_ids() {
        let Some(mut entity) = scene.get_entity_mut(id) else {
            continue;
        };
        if !entity.active {
            continue;
        }
        let Some(anim) = &mut entity.animator else {
            continue;
        };
        if !anim.graph_enabled {
            continue;
        }
        let Some(path) = anim.graph.clone() else {
            continue;
        };
        if let Some(graph) = res.animation_graphs.get_or_load(&path, &res.console) {
            step_graph(anim, graph);
        }
    }
}

/// One evaluation step for one animator: (re)bind when the active node is unknown
/// or no longer in the graph, otherwise fire the first satisfied outgoing edge.
pub(crate) fn step_graph(anim: &mut AnimatorComponent, graph: &AnimationGraph) {
    let bound = anim
        .current_node
        .as_deref()
        .is_some_and(|node| graph.node(node).is_some());
    if !bound {
        anim.bind_graph(graph);
        return;
    }
    let Some(active) = anim.current_node.clone() else {
        return;
    };
    let Some(edge) = graph.edges_from(&active).find(|e| edge_satisfied(anim, e)) else {
        return;
    };
    consume_edge_triggers(anim, edge);
    if let Some(target) = graph.node(&edge.to) {
        anim.enter_node(target, edge.transition_duration);
    }
}

/// All of `edge`'s conditions hold (AND). An edge with **no** conditions never
/// fires from data alone — it exists for the explicit `Animator.PlayNode` jump.
fn edge_satisfied(anim: &AnimatorComponent, edge: &GraphEdge) -> bool {
    !edge.conditions.is_empty() && edge.conditions.iter().all(|c| condition_holds(anim, c))
}

/// One condition against the animator's live parameter values. A parameter that is
/// absent (or holds another type) never satisfies — binding seeds the declared
/// defaults, so this only bites undeclared reads. Trigger checks are
/// non-consuming; only a FIRED edge consumes (see [`consume_edge_triggers`]).
fn condition_holds(anim: &AnimatorComponent, condition: &Condition) -> bool {
    match condition {
        Condition::Bool { parameter, value } => anim.get_bool(parameter) == Some(*value),
        Condition::Float {
            parameter,
            op,
            value,
        } => anim
            .get_float(parameter)
            .is_some_and(|v| compare(v, *op, *value)),
        Condition::Int {
            parameter,
            op,
            value,
        } => anim
            .get_int(parameter)
            .is_some_and(|v| compare(v, *op, *value)),
        Condition::Trigger { parameter } => anim.trigger_is_set(parameter),
    }
}

fn compare<T: PartialOrd>(lhs: T, op: NumericOp, rhs: T) -> bool {
    match op {
        NumericOp::Less => lhs < rhs,
        NumericOp::LessEqual => lhs <= rhs,
        NumericOp::Greater => lhs > rhs,
        NumericOp::GreaterEqual => lhs >= rhs,
        NumericOp::Equal => lhs == rhs,
        NumericOp::NotEqual => lhs != rhs,
    }
}

/// Read-and-clear every `Trigger` the fired edge's conditions read — Unity's
/// auto-reset: a trigger fires exactly one transition (#314).
fn consume_edge_triggers(anim: &mut AnimatorComponent, edge: &GraphEdge) {
    for condition in &edge.conditions {
        if let Condition::Trigger { parameter } = condition {
            anim.consume_trigger(parameter);
        }
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod graph_tests;
