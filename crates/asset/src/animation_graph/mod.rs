//! Unity's AnimatorController asset as pure data; the top level is the base layer and `layers`
//! stack further machines over it.
//! stored-format: the `.animgraph` document

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod blend_tree;
mod event;
mod io;
mod layer;
mod montage;
mod sync;
mod validate;
mod validate_motion;

pub use blend_tree::{BlendChild1D, BlendChild2D, BlendTree, ChildMotion, MAX_TREE_DEPTH};
pub use event::AnimationEvent;
pub use io::{load, save};
pub use layer::{GraphLayer, LayerBlending};
pub use montage::{GraphSlot, Montage, MontageSection, DEFAULT_MONTAGE_BLEND, DEFAULT_SLOT};
pub use sync::SyncMarker;
pub use validate::GraphError;

#[cfg(test)]
pub const GRAPH_EXTENSION: &str = "animgraph";

/// A `Trigger` declares no default: it always starts clear.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterDeclaration {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_tree: Option<BlendTree>,
    #[serde(default)]
    pub is_loop: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// Events, in the clip's own seconds; none on a blend-tree node (each child carries its
    /// clip's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_group: Option<String>,
    /// Sync markers, in the clip's own seconds; none on a blend-tree node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync_markers: Vec<SyncMarker>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,
}

/// Fires when all its conditions hold; with none, only explicit jumps fire it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Linear crossfade length in seconds; `0` is a hard cut.
    #[serde(default)]
    pub transition_duration: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Is-true / is-false.
    Bool { parameter: String, value: bool },
    Float {
        parameter: String,
        op: NumericOp,
        value: f32,
    },
    Int {
        parameter: String,
        op: NumericOp,
        value: i32,
    },
    /// Latched; consumed when the transition fires.
    Trigger { parameter: String },
}

impl Condition {
    pub fn parameter(&self) -> &str {
        match self {
            Condition::Bool { parameter, .. }
            | Condition::Float { parameter, .. }
            | Condition::Int { parameter, .. }
            | Condition::Trigger { parameter } => parameter,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
}

/// Authored order is priority: the first satisfied outgoing edge wins.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateMachine {
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
    pub entry: String,
}

impl StateMachine {
    pub fn node(&self, name: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AnimationGraph {
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterDeclaration>,
    #[serde(flatten)]
    pub base: StateMachine,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<GraphLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<GraphSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub montages: Vec<Montage>,
}

impl AnimationGraph {
    pub fn node(&self, name: &str) -> Option<&GraphNode> {
        self.base.node(name)
    }

    pub fn machine(&self, index: usize) -> Option<&StateMachine> {
        match index {
            0 => Some(&self.base),
            i => self.layers.get(i - 1).map(|l| &l.machine),
        }
    }
}

#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod montage_tests;
#[cfg(test)]
mod nested_tests;
#[cfg(test)]
mod sync_tests;
#[cfg(test)]
mod tag_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod v2_tests;
#[cfg(test)]
mod validate_tests;
