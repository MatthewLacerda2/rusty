//! src/asset/animation_graph/ — the `AnimationGraph` asset (issue #315).
//!
//! The authored animation state machine, as pure data: **nodes** (each plays one
//! clip or a [`BlendTree`] of clips, #457), directed **edges** (transitions with AND-combined conditions and a linear
//! crossfade duration), the typed **parameter declarations** the conditions read,
//! and the designated **entry node**. Since #457 the top-level machine is the
//! *base layer*, and `layers` stacks further machines over it, each with a weight,
//! a blend mode and an optional bone mask ([`GraphLayer`]). This is Unity's AnimatorController *asset*,
//! named for what it is. The per-entity runtime — live parameter values, active
//! node, playhead — stays on the `AnimatorComponent` (#314/#316); the component
//! references a graph **by path** (`guard.animgraph`), exactly like a mesh
//! references its source file, so many entities share one graph and the scene
//! stores a reference, never the contents.
//!
//! Purity: serde + std only — no wgpu/egui/mlua and no evaluation logic (the
//! runtime evaluator is #316). Containers are ordered (`BTreeMap`/`Vec`) so the
//! serialized JSON is stable and the fixed-step evaluator's walk is deterministic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod blend_tree;
mod io;
mod layer;
mod validate;
mod validate_motion;

pub use blend_tree::{BlendChild1D, BlendChild2D, BlendTree};
pub use io::{is_graph_path, load, save};
pub use layer::{GraphLayer, LayerBlending};
pub use validate::GraphError;

/// The graph asset's file extension: `guard.animgraph` (rusty-only format, JSON
/// inside for diffability — matching the scene document and `.meta` sidecars).
pub const GRAPH_EXTENSION: &str = "animgraph";

/// Declaration of one graph parameter: its type plus the default value an entity's
/// `AnimatorComponent` starts from when it binds the graph (#316). The live values
/// mirror this shape (`AnimatorParameter`, #314) but live on the component; a
/// `Trigger` declares no default — it always starts clear.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterDeclaration {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger,
}

/// One state in the graph: the motion it plays — one clip, or a blend tree of
/// clips (#457), never both — and how it plays it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// The node's unique name — the identity `entry` and edges reference. Distinct
    /// from `clip`: two nodes may play the same clip differently.
    pub name: String,
    /// Name of the animation clip this node plays (matched against the entity's
    /// clip list at runtime, like `AnimatorComponent::current_clip`). Empty for a
    /// blend-tree node.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clip: String,
    /// The blend tree this node plays instead of a single clip (#457).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_tree: Option<BlendTree>,
    /// Loop the clip while this node is active (wraps the playhead, #313).
    #[serde(default)]
    pub is_loop: bool,
    /// Per-node playback-rate multiplier, stacked on the component's own `speed`.
    /// `None` means 1× (no override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
}

/// A directed transition `from → to`. It fires when **all** its conditions hold
/// (AND); an edge with no conditions never fires from data alone — it exists for
/// the explicit jumps #316's control API takes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node name.
    pub from: String,
    /// Target node name.
    pub to: String,
    /// AND-combined conditions over the declared parameters.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Linear crossfade length in seconds; `0` is a hard cut.
    #[serde(default)]
    pub transition_duration: f32,
}

/// One condition over a declared parameter. The operators form a small closed set
/// keyed by parameter type — the variant *is* the type, so a `Bool` parameter can
/// never carry a `<` and illegal combinations are unrepresentable. Validation
/// (`AnimationGraph::validate`) checks the referenced parameter is declared with
/// the matching type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// The `Bool` parameter equals `value` (is-true / is-false).
    Bool { parameter: String, value: bool },
    /// The `Float` parameter compares against `value` under `op`.
    Float {
        parameter: String,
        op: NumericOp,
        value: f32,
    },
    /// The `Int` parameter compares against `value` under `op`.
    Int {
        parameter: String,
        op: NumericOp,
        value: i32,
    },
    /// The `Trigger` parameter is latched (fired). The evaluator consumes the
    /// latch when the transition fires (#316).
    Trigger { parameter: String },
}

impl Condition {
    /// The declared-parameter name this condition reads.
    pub fn parameter(&self) -> &str {
        match self {
            Condition::Bool { parameter, .. }
            | Condition::Float { parameter, .. }
            | Condition::Int { parameter, .. }
            | Condition::Trigger { parameter } => parameter,
        }
    }
}

/// Comparison operator for the numeric (`Float`/`Int`) condition kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
}

/// One state machine: its states, its transitions and where it starts. The base
/// layer's machine sits at the top level of the document; every extra
/// [`GraphLayer`] carries its own. `nodes` and `edges` are `Vec`s because
/// *authored order is priority order* — the evaluator (#316) takes the first
/// satisfied outgoing edge.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateMachine {
    /// The states, in authored order.
    pub nodes: Vec<GraphNode>,
    /// The transitions, in authored (priority) order.
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
    /// Name of the entry/default node playback starts in.
    pub entry: String,
}

impl StateMachine {
    /// Find a node by name.
    pub fn node(&self, name: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

/// The whole graph, as one serde document: the shared parameter declarations,
/// the base layer's machine (flattened to the top level, so a pre-#457
/// single-layer graph is exactly a graph with no extra layers), and the extra
/// layers in composition order. `parameters` is a `BTreeMap` so the JSON stays
/// name-sorted and diffable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraph {
    /// The declared parameters: name → type + default value. Shared by every
    /// layer.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterDeclaration>,
    /// The base layer (layer 0): always full weight, whole body, override.
    #[serde(flatten)]
    pub base: StateMachine,
    /// Layers 1.. in composition order, each blended over everything below it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<GraphLayer>,
}

impl AnimationGraph {
    /// Find a base-layer node by name.
    pub fn node(&self, name: &str) -> Option<&GraphNode> {
        self.base.node(name)
    }

    /// The machine of layer `index` (0 is the base layer).
    pub fn machine(&self, index: usize) -> Option<&StateMachine> {
        match index {
            0 => Some(&self.base),
            i => self.layers.get(i - 1).map(|l| &l.machine),
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod v2_tests;
#[cfg(test)]
mod validate_tests;
