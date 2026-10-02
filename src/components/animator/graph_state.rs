//! src/components/animator/graph_state.rs — graph-driven state transitions (#316).
//!
//! The two writes the animation-graph runtime makes to an [`AnimatorComponent`]:
//! **binding** a graph (seed the declared parameter defaults, enter the entry
//! node — and, since #457, each extra layer's) and **entering** a node (crossfade into its clip, adopt its loop flag and
//! per-node speed, make it the active state). Both the fixed-step evaluator
//! (`app::animation::graph`) and the explicit `Animator.PlayNode` jump route
//! through here, so a graph state change behaves identically wherever it comes
//! from. The graph *asset* stays pure data (#315); this is the component-side
//! runtime of it.

use crate::asset::animation_graph::{AnimationGraph, GraphNode, ParameterDeclaration};

use super::{AnimatorComponent, AnimatorParameter, LayerState};

impl AnimatorParameter {
    /// The live value a declared parameter starts from when a graph binds: its
    /// declared default; a `Trigger` declares none — it always starts clear.
    pub fn from_declaration(decl: &ParameterDeclaration) -> Self {
        match decl {
            ParameterDeclaration::Bool(v) => AnimatorParameter::Bool(*v),
            ParameterDeclaration::Float(v) => AnimatorParameter::Float(*v),
            ParameterDeclaration::Int(v) => AnimatorParameter::Int(*v),
            ParameterDeclaration::Trigger => AnimatorParameter::Trigger(false),
        }
    }
}

impl AnimatorComponent {
    /// Enter base-layer `node`: crossfade into its motion over `blend` seconds (a
    /// non-positive `blend` — or a fade into the motion already playing —
    /// hard-cuts), adopt the node's `is_loop` flag and per-node speed, and make it
    /// the active graph state. Starts playback.
    pub fn enter_node(&mut self, node: &GraphNode, blend: f32) {
        self.base.enter_node(node, blend);
        self.is_playing = true;
        self.freeze = false;
    }

    /// Bind `graph`: seed every declared parameter the store doesn't hold yet with
    /// its declared default (values a script already wrote win), then hard-cut
    /// into the entry node. The evaluator calls this on its first step over a
    /// graph and whenever the base layer's active node no longer resolves (e.g.
    /// the reference changed). Extra layers bind on their own
    /// ([`sync_layers`](Self::sync_layers)).
    pub fn bind_graph(&mut self, graph: &AnimationGraph) {
        for (name, decl) in &graph.parameters {
            if !self.parameters.contains_key(name) {
                self.parameters
                    .insert(name.clone(), AnimatorParameter::from_declaration(decl));
            }
        }
        if let Some(entry) = graph.node(&graph.base.entry) {
            self.enter_node(entry, 0.0);
        }
    }

    /// Match the extra layers' state to `graph`'s layers (#457): one state per
    /// layer, each named after its layer, and any layer whose active node does not
    /// resolve (new, or the graph changed) hard-cut into its entry node. A weight a
    /// script set before the bind survives it.
    pub fn sync_layers(&mut self, graph: &AnimationGraph) {
        self.layers
            .resize_with(graph.layers.len(), LayerState::default);
        for (state, layer) in self.layers.iter_mut().zip(&graph.layers) {
            if state.name != layer.name {
                state.name.clone_from(&layer.name);
            }
            let machine = &layer.machine;
            let bound = state
                .playback
                .current_node
                .as_deref()
                .is_some_and(|n| machine.node(n).is_some());
            if let (false, Some(entry)) = (bound, machine.node(&machine.entry)) {
                state.playback.enter_node(entry, 0.0);
            }
        }
    }

    /// The index of the extra layer named `name` (1-based, as scripts count
    /// layers; 0 is the base layer), once the graph has been bound.
    pub fn layer_index(&self, name: &str) -> Option<usize> {
        self.layers
            .iter()
            .position(|l| l.name == name)
            .map(|i| i + 1)
    }
}
