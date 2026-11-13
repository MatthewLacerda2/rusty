//! crates/asset/src/animation_graph/blend_tree.rs — the blend-tree node kind (#457).
//!
//! A blend tree plays N motions at once, weighted by where one or two `Float`
//! parameters sit among the children's authored positions — Unity's
//! `BlendTree`. The kinds are Unity's: a 1D line of thresholds, and 2D
//! freeform directional (gradient bands in polar space, for locomotion where the
//! children are directions) or freeform cartesian (gradient bands in plain x/y,
//! for children that are not directions). A child plays a clip or, since #1746,
//! another blend tree of any kind, which may read different parameters from its
//! parent's; weights multiply down the tree. A tree holds its children by
//! value, so a graph cannot form a cycle, and [`MAX_TREE_DEPTH`] bounds how deep
//! one nests. The weights themselves are computed by the runtime
//! (`app::animation::blend_tree`); this is the authored data only.

use serde::{Deserialize, Serialize};

use super::{AnimationEvent, SyncMarker};

/// How many trees may nest inside one another, the node's own tree included.
/// Validation refuses a deeper graph; the runtime stops descending there.
pub const MAX_TREE_DEPTH: usize = 8;

/// A blend tree: its kind, the parameter(s) it reads and its children. The
/// variant *is* the kind, so a 1D tree can never carry a second axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlendTree {
    /// Children on a line by threshold, driven by one `Float` parameter. Between
    /// two neighbouring thresholds the two children cross-blend linearly; past
    /// either end the end child plays alone.
    Simple1D {
        parameter: String,
        children: Vec<BlendChild1D>,
    },
    /// Children in a plane (`parameter_x`, `parameter_y`), read as directions and
    /// speeds: forward, back, strafes and diagonals, optionally an idle at the
    /// origin and run rings further out. Gradient-band interpolation in polar
    /// space.
    FreeformDirectional2D {
        parameter_x: String,
        parameter_y: String,
        children: Vec<BlendChild2D>,
    },
    /// Children in a plane, gradient-band interpolation in cartesian space — for
    /// two axes that are not a direction (e.g. aim pitch × lean).
    FreeformCartesian2D {
        parameter_x: String,
        parameter_y: String,
        children: Vec<BlendChild2D>,
    },
}

/// What one child plays: a `clip` or a nested `blend_tree` (#1746), exactly
/// one of them. Written inline in the child's JSON object, beside its
/// `threshold` or `position`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildMotion {
    /// The clip this child plays; empty when it plays a nested tree.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clip: String,
    /// The tree this child plays instead of a clip (#1746).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_tree: Option<Box<BlendTree>>,
    /// The child clip's animation events (#459), in its own seconds. A nested
    /// tree carries none of its own: its leaves carry their clips'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
    /// The child clip's sync markers (#1896), in its own seconds; a nested
    /// tree carries none of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync_markers: Vec<SyncMarker>,
}

impl ChildMotion {
    /// A child that plays `clip`, with no events.
    pub fn clip(clip: &str) -> Self {
        Self {
            clip: clip.to_string(),
            ..Self::default()
        }
    }

    /// A child that plays the nested `tree`.
    pub fn tree(tree: BlendTree) -> Self {
        Self {
            blend_tree: Some(Box::new(tree)),
            ..Self::default()
        }
    }
}

/// One child of a 1D tree: a motion and the parameter value it plays alone at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild1D {
    #[serde(flatten)]
    pub motion: ChildMotion,
    pub threshold: f32,
}

/// One child of a 2D tree: a motion and the `[x, y]` point it plays alone at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild2D {
    #[serde(flatten)]
    pub motion: ChildMotion,
    pub position: [f32; 2],
}

impl BlendTree {
    /// The `Float` parameters this tree reads (not its nested trees'), in axis
    /// order.
    pub fn parameters(&self) -> Vec<&str> {
        match self {
            BlendTree::Simple1D { parameter, .. } => vec![parameter],
            BlendTree::FreeformDirectional2D {
                parameter_x,
                parameter_y,
                ..
            }
            | BlendTree::FreeformCartesian2D {
                parameter_x,
                parameter_y,
                ..
            } => vec![parameter_x, parameter_y],
        }
    }

    /// How many children the tree has (not counting nested trees' children).
    pub fn len(&self) -> usize {
        match self {
            BlendTree::Simple1D { children, .. } => children.len(),
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => children.len(),
        }
    }

    /// Whether the tree has no children.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Child `i`'s motion and its point (a 1D threshold sits on the x axis).
    pub fn child(&self, i: usize) -> Option<(&ChildMotion, [f32; 2])> {
        match self {
            BlendTree::Simple1D { children, .. } => {
                children.get(i).map(|c| (&c.motion, [c.threshold, 0.0]))
            }
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => {
                children.get(i).map(|c| (&c.motion, c.position))
            }
        }
    }

    /// The children's motions and points, in authored order, without allocating.
    pub fn children(&self) -> impl Iterator<Item = (&ChildMotion, [f32; 2])> + '_ {
        (0..self.len()).filter_map(|i| self.child(i))
    }
}
