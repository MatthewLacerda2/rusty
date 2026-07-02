//! src/components/animator/parameters.rs — typed animator parameters (#314).
//!
//! Unity-style named, typed variables — `Bool` / `Float` / `Int` / `Trigger` — that
//! scripts write (`Animator.Set*`) and the `AnimationGraph`'s edge conditions
//! (#315) will read each fixed step to decide transitions (#316). The VALUES live
//! here on the per-entity [`AnimatorComponent`] and serialize with the entity; the
//! declarations (names/types/defaults) belong to the graph asset (#315) — that
//! split stays clean. The store is a `BTreeMap` so iteration order is name-sorted
//! and deterministic, as the fixed-step evaluator requires.

use std::collections::BTreeMap;
use std::fmt;
use std::mem::discriminant;

use serde::{Deserialize, Serialize};

use super::AnimatorComponent;

/// The name → value parameter store carried by [`AnimatorComponent::parameters`].
pub type AnimatorParameters = BTreeMap<String, AnimatorParameter>;

/// One typed parameter value.
///
/// `Trigger` is a one-shot bool: [`AnimatorComponent::set_trigger`] latches it and
/// the graph evaluator (#316) clears it via [`AnimatorComponent::consume_trigger`]
/// the moment it fires a transition. An unconsumed trigger stays latched — the
/// script never has to reset it.
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
    /// Write `value` under `name`: insert it when absent, overwrite it when present
    /// with the same type. A name already bound to a DIFFERENT type is left
    /// untouched and `false` is returned — a no-op the caller may log, never a
    /// panic (#314).
    pub fn set_parameter(&mut self, name: &str, value: AnimatorParameter) -> bool {
        match self.parameters.get(name) {
            Some(existing) if discriminant(existing) != discriminant(&value) => false,
            _ => {
                self.parameters.insert(name.to_string(), value);
                true
            }
        }
    }

    /// Set a `Bool` parameter (`Animator.SetBool`).
    pub fn set_bool(&mut self, name: &str, value: bool) -> bool {
        self.set_parameter(name, AnimatorParameter::Bool(value))
    }

    /// Set a `Float` parameter (`Animator.SetFloat`).
    pub fn set_float(&mut self, name: &str, value: f32) -> bool {
        self.set_parameter(name, AnimatorParameter::Float(value))
    }

    /// Set an `Int` parameter (`Animator.SetInt`).
    pub fn set_int(&mut self, name: &str, value: i32) -> bool {
        self.set_parameter(name, AnimatorParameter::Int(value))
    }

    /// Latch the one-shot `Trigger` parameter `name` (`Animator.SetTrigger`). It
    /// stays latched until [`consume_trigger`](Self::consume_trigger) clears it.
    pub fn set_trigger(&mut self, name: &str) -> bool {
        self.set_parameter(name, AnimatorParameter::Trigger(true))
    }

    /// The `Bool` parameter `name`, or `None` when absent or another type.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    /// The `Float` parameter `name`, or `None` when absent or another type.
    pub fn get_float(&self, name: &str) -> Option<f32> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Float(v)) => Some(*v),
            _ => None,
        }
    }

    /// The `Int` parameter `name`, or `None` when absent or another type.
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

    /// Read-and-clear the `Trigger` parameter `name`: returns whether it was
    /// latched and resets it to clear. The graph evaluator (#316) calls this when a
    /// trigger condition fires a transition — Unity's auto-reset semantics. `false`
    /// (and no write) when the parameter is absent or another type.
    pub fn consume_trigger(&mut self, name: &str) -> bool {
        match self.parameters.get_mut(name) {
            Some(AnimatorParameter::Trigger(latched)) => std::mem::take(latched),
            _ => false,
        }
    }
}
