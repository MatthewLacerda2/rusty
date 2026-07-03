//! src/components/animator/graph_state.rs — graph-driven state transitions (#316).
//!
//! The two writes the animation-graph runtime makes to an [`AnimatorComponent`]:
//! **binding** a graph (seed the declared parameter defaults, enter the entry
//! node) and **entering** a node (crossfade into its clip, adopt its loop flag and
//! per-node speed, make it the active state). Both the fixed-step evaluator
//! (`app::animation::graph`) and the explicit `Animator.PlayNode` jump route
//! through here, so a graph state change behaves identically wherever it comes
//! from. The graph *asset* stays pure data (#315); this is the component-side
//! runtime of it.

use crate::asset::animation_graph::{AnimationGraph, GraphNode, ParameterDeclaration};

use super::{AnimatorComponent, AnimatorParameter};

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
    /// Enter `node`: crossfade into its clip over `blend` seconds (a non-positive
    /// `blend` — or a fade into the already-current clip — hard-cuts, exactly like
    /// [`crossfade`](Self::crossfade)), adopt the node's `is_loop` flag and
    /// per-node speed, and make it the active graph state.
    pub fn enter_node(&mut self, node: &GraphNode, blend: f32) {
        self.crossfade(node.clip.clone(), blend);
        self.loop_clip = node.is_loop;
        self.node_speed = node.speed.unwrap_or(1.0);
        self.current_node = Some(node.name.clone());
    }

    /// Bind `graph`: seed every declared parameter the store doesn't hold yet with
    /// its declared default (values a script already wrote win), then hard-cut into
    /// the entry node. The evaluator calls this on its first step over a graph and
    /// whenever the active node no longer resolves (e.g. the reference changed).
    pub fn bind_graph(&mut self, graph: &AnimationGraph) {
        for (name, decl) in &graph.parameters {
            if !self.parameters.contains_key(name) {
                self.parameters
                    .insert(name.clone(), AnimatorParameter::from_declaration(decl));
            }
        }
        if let Some(entry) = graph.node(&graph.entry) {
            self.enter_node(entry, 0.0);
        }
    }
}
