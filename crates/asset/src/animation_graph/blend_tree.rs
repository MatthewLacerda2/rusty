use serde::{Deserialize, Serialize};

use super::{AnimationEvent, SyncMarker};

/// Max nesting of trees, the node's own included; validation refuses deeper.
pub const MAX_TREE_DEPTH: usize = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlendTree {
    /// Thresholds ascending; past either end the end child plays alone.
    Simple1D {
        parameter: String,
        children: Vec<BlendChild1D>,
    },
    FreeformDirectional2D {
        parameter_x: String,
        parameter_y: String,
        children: Vec<BlendChild2D>,
    },
    FreeformCartesian2D {
        parameter_x: String,
        parameter_y: String,
        children: Vec<BlendChild2D>,
    },
}

/// Exactly one of `clip` or a nested `blend_tree`, written inline beside `threshold` or `position`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildMotion {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_tree: Option<Box<BlendTree>>,
    /// Events, in the child clip's own seconds; none on a nested tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<AnimationEvent>,
    /// Sync markers, in the child clip's own seconds; none on a nested tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync_markers: Vec<SyncMarker>,
}

impl ChildMotion {
    pub fn clip(clip: &str) -> Self {
        Self {
            clip: clip.to_string(),
            ..Self::default()
        }
    }

    pub fn tree(tree: BlendTree) -> Self {
        Self {
            blend_tree: Some(Box::new(tree)),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild1D {
    #[serde(flatten)]
    pub motion: ChildMotion,
    pub threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendChild2D {
    #[serde(flatten)]
    pub motion: ChildMotion,
    pub position: [f32; 2],
}

impl BlendTree {
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

    pub fn len(&self) -> usize {
        match self {
            BlendTree::Simple1D { children, .. } => children.len(),
            BlendTree::FreeformDirectional2D { children, .. }
            | BlendTree::FreeformCartesian2D { children, .. } => children.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

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

    pub fn children(&self) -> impl Iterator<Item = (&ChildMotion, [f32; 2])> + '_ {
        (0..self.len()).filter_map(|i| self.child(i))
    }
}
