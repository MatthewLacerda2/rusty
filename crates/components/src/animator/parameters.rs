use std::collections::BTreeMap;
use std::fmt;
use std::mem::discriminant;

use serde::{Deserialize, Serialize};

use super::AnimatorComponent;

pub type AnimatorParameters = BTreeMap<String, AnimatorParameter>;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnimatorParameter {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

impl fmt::Display for AnimatorParameter {
    /// `Type = value` — the inspector card's read-only listing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "Bool = {v}"),
            Self::Float(v) => write!(f, "Float = {v}"),
            Self::Int(v) => write!(f, "Int = {v}"),
            Self::Trigger(true) => write!(f, "Trigger = set"),
            Self::Trigger(false) => write!(f, "Trigger = clear"),
        }
    }
}

impl AnimatorComponent {
    pub fn set_parameter(&mut self, name: &str, value: AnimatorParameter) -> bool {
        match self.parameters.get(name) {
            Some(existing) if discriminant(existing) != discriminant(&value) => false,
            _ => {
                self.parameters.insert(name.to_string(), value);
                true
            }
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) -> bool {
        self.set_parameter(name, AnimatorParameter::Bool(value))
    }

    pub fn set_float(&mut self, name: &str, value: f32) -> bool {
        self.set_parameter(name, AnimatorParameter::Float(value))
    }

    pub fn set_int(&mut self, name: &str, value: i32) -> bool {
        self.set_parameter(name, AnimatorParameter::Int(value))
    }

    pub fn set_trigger(&mut self, name: &str) -> bool {
        self.set_parameter(name, AnimatorParameter::Trigger(true))
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_float(&self, name: &str) -> Option<f32> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Float(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Int(v)) => Some(*v),
            _ => None,
        }
    }

    /// Is the `Trigger` parameter `name` latched? A non-consuming read (condition
    /// *checks* use this; only a FIRED transition consumes).
    pub fn trigger_is_set(&self, name: &str) -> bool {
        matches!(
            self.parameters.get(name),
            Some(AnimatorParameter::Trigger(true))
        )
    }

    pub fn consume_trigger(&mut self, name: &str) -> bool {
        match self.parameters.get_mut(name) {
            Some(AnimatorParameter::Trigger(latched)) => std::mem::take(latched),
            _ => false,
        }
    }
}
