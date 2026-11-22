use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use rusty_asset::animation_graph::{
    self, AnimationGraph, Condition, GraphEdge, NumericOp, StateMachine,
};
use rusty_components::{AnimatorComponent, Playback};
use rusty_script::scripting::ConsoleLogs;

use super::super::{Resources, World};

#[derive(Default)]
pub struct GraphCache {
    /// `None` records a failed load, so its error is logged once, not every step.
    loaded: BTreeMap<String, Option<AnimationGraph>>,
}

impl GraphCache {
    pub fn get_or_load(
        &mut self,
        path: &str,
        console: &Rc<RefCell<ConsoleLogs>>,
    ) -> Option<&AnimationGraph> {
        if !self.loaded.contains_key(path) {
            let result = animation_graph::load(Path::new(path));
            match &result {
                Err(err) => console
                    .borrow_mut()
                    .error(format!("Animation graph '{path}': {err}")),
                Ok(graph) => {
                    for warning in graph.warnings() {
                        console
                            .borrow_mut()
                            .warn(format!("Animation graph '{path}': {warning}"));
                    }
                }
            }
            self.loaded.insert(path.to_string(), result.ok());
        }
        self.loaded.get(path).and_then(Option::as_ref)
    }

    /// Emptied on each Play enter (`enter_play_steps`), so a session sees edits to the graph files.
    pub fn clear(&mut self) {
        self.loaded.clear();
    }
}

pub fn evaluate_graphs(world: &mut World, res: &mut Resources) {
    let mut scene = world.scene.borrow_mut();
    for id in scene.world.ids_with_animator() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let Some(path) = scene
            .world
            .animator(id)
            .filter(|a| a.graph_enabled)
            .and_then(|a| a.graph.clone())
        else {
            continue;
        };
        if let Some(graph) = res.animation_graphs.get_or_load(&path, &res.console) {
            if let Some(mut anim) = scene.world.animator_mut(id) {
                step_graph(&mut anim, graph);
            }
        }
    }
}

pub(crate) fn step_graph(anim: &mut AnimatorComponent, graph: &AnimationGraph) {
    let bound = anim
        .base
        .current_node
        .as_deref()
        .is_some_and(|node| graph.node(node).is_some());
    let mut fired = Vec::new();
    if bound {
        if let Some(edge) = next_edge(anim, &anim.base, &graph.base) {
            if let Some(target) = graph.node(&edge.to) {
                anim.enter_node(target, edge.transition_duration);
            }
            fired.push(edge);
        }
    } else {
        anim.bind_graph(graph);
    }
    anim.sync_layers(graph);
    for (i, layer) in graph.layers.iter().enumerate() {
        let Some(edge) = next_edge(anim, &anim.layers[i].playback, &layer.machine) else {
            continue;
        };
        if let Some(target) = layer.machine.node(&edge.to) {
            anim.layers[i]
                .playback
                .enter_node(target, edge.transition_duration);
        }
        fired.push(edge);
    }
    // After every layer decided: one trigger fires a transition in each layer that reads it.
    for edge in fired {
        consume_edge_triggers(anim, edge);
    }
}

fn next_edge<'g>(
    anim: &AnimatorComponent,
    playback: &Playback,
    machine: &'g StateMachine,
) -> Option<&'g GraphEdge> {
    let active = playback.current_node.as_deref()?;
    machine
        .edges
        .iter()
        .find(|e| e.from == active && edge_satisfied(anim, e))
}

fn edge_satisfied(anim: &AnimatorComponent, edge: &GraphEdge) -> bool {
    !edge.conditions.is_empty() && edge.conditions.iter().all(|c| condition_holds(anim, c))
}

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

fn consume_edge_triggers(anim: &mut AnimatorComponent, edge: &GraphEdge) {
    for condition in &edge.conditions {
        if let Condition::Trigger { parameter } = condition {
            anim.consume_trigger(parameter);
        }
    }
}

#[cfg(test)]
#[path = "graph_layer_tests.rs"]
mod graph_layer_tests;
#[cfg(test)]
#[path = "graph_tests.rs"]
mod graph_tests;
