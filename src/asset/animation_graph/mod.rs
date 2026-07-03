//! src/asset/animation_graph/ — the `AnimationGraph` asset (issue #315).
//!
//! The authored animation state machine, as pure data: **nodes** (each plays one
//! clip), directed **edges** (transitions with AND-combined conditions and a linear
//! crossfade duration), the typed **parameter declarations** the conditions read,
//! and the designated **entry node**. This is Unity's AnimatorController *asset*,
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

mod io;
mod validate;

pub use io::{is_graph_path, load, save};
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

/// One state in the graph: the clip it plays and how it plays it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// The node's unique name — the identity `entry` and edges reference. Distinct
    /// from `clip`: two nodes may play the same clip differently.
    pub name: String,
    /// Name of the animation clip this node plays (matched against the entity's
    /// clip list at runtime, like `AnimatorComponent::current_clip`).
    pub clip: String,
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

/// The whole state machine, as one serde document. `nodes` and `edges` are `Vec`s
/// because *authored order is priority order* — the evaluator (#316) takes the
/// first satisfied outgoing edge — and `parameters` is a `BTreeMap` so the JSON
/// stays name-sorted and diffable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraph {
    /// The declared parameters: name → type + default value.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterDeclaration>,
    /// The states, in authored order.
    pub nodes: Vec<GraphNode>,
    /// The transitions, in authored (priority) order.
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
    /// Name of the entry/default node playback starts in.
    pub entry: String,
}

impl AnimationGraph {
    /// Find a node by name.
    pub fn node(&self, name: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// The outgoing edges of `from`, in authored (priority) order.
    pub fn edges_from<'a>(&'a self, from: &'a str) -> impl Iterator<Item = &'a GraphEdge> {
        self.edges.iter().filter(move |e| e.from == from)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod validate_tests;
