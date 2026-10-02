//! src/asset/animation_graph/blend_tree.rs — the blend-tree node kind (#457).
//!
//! A blend tree plays N clips at once, weighted by where one or two `Float`
//! parameters sit among the children's authored positions — Unity's
//! `BlendTree`. The kinds are Unity's: a 1D line of thresholds, and 2D
//! freeform directional (gradient bands in polar space, for locomotion where the
//! children are directions) or freeform cartesian (gradient bands in plain x/y,
//! for children that are not directions). The weights themselves are computed by
//! the runtime (`app::animation::blend_tree`); this is the authored data only.

use serde::{Deserialize, Serialize};

use super::AnimationEvent;

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

/// One child of a 1D tree: a clip and the parameter value it plays alone at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild1D {
    pub clip: String,
    pub threshold: f32,
    /// The child clip's animation events (#459), in its own seconds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
}

/// One child of a 2D tree: a clip and the `[x, y]` point it plays alone at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild2D {
    pub clip: String,
    pub position: [f32; 2],
    /// The child clip's animation events (#459), in its own seconds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
}

impl BlendTree {
    /// The `Float` parameters the tree reads, in axis order.
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

    /// The children's animation events (#459), in authored order.
    pub fn child_events(&self) -> Vec<&[AnimationEvent]> {
        match self {
            BlendTree::Simple1D { children, .. } => {
                children.iter().map(|c| c.events.as_slice()).collect()
            }
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => {
                children.iter().map(|c| c.events.as_slice()).collect()
            }
        }
    }

    /// The children's clip names, in authored order.
    pub fn clips(&self) -> Vec<&str> {
        match self {
            BlendTree::Simple1D { children, .. } => {
                children.iter().map(|c| c.clip.as_str()).collect()
            }
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => {
                children.iter().map(|c| c.clip.as_str()).collect()
            }
        }
    }
}
