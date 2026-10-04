//! What a tween drives (#424): a fixed component field ([`Field`]) or a runtime
//! shader param ([`ShaderParam`], #661), resolved by path once at `Tween.To`.
//!
//! Every value travels as a `Vec4`; a property uses its first `arity` lanes.

use glam::Vec4;

use super::field::Field;
use super::shader_param::{self, ShaderParam};
use crate::scene::Scene;

#[derive(Debug)]
pub(crate) enum Property {
    Field(Field),
    Shader(Box<ShaderParam>),
}

impl Property {
    /// Resolve `path` on entity `id`, or say why it cannot be tweened there: an
    /// unknown path (listing the animatable ones), a missing component, or a
    /// shader param the shader does not expose at runtime.
    pub(crate) fn parse(scene: &Scene, id: u32, path: &str) -> Result<Self, String> {
        if let Some(field) = Field::parse(path) {
            if field.get(scene, id).is_none() {
                let component = path.split('.').next().unwrap_or_default();
                return Err(format!("entity {id} has no {component}"));
            }
            return Ok(Self::Field(field));
        }
        match ShaderParam::parse(scene, id, path) {
            Some(param) => param.map(|p| Self::Shader(Box::new(p))),
            None => Err(format!(
                "unknown tween property `{path}`; animatable: {}, {}",
                Field::paths(),
                shader_param::PATHS
            )),
        }
    }

    /// The path this property was parsed from.
    pub(crate) fn path(&self) -> &str {
        match self {
            Self::Field(f) => f.path(),
            Self::Shader(p) => p.path(),
        }
    }

    /// How many numbers the value has (1 = a number, 2..4 = a vector / colour).
    pub(crate) fn arity(&self) -> usize {
        match self {
            Self::Field(f) => f.arity(),
            Self::Shader(p) => p.arity(),
        }
    }

    /// The current value, or `None` once the component (or shader) is gone.
    pub(crate) fn get(&self, scene: &Scene, id: u32) -> Option<Vec4> {
        match self {
            Self::Field(f) => f.get(scene, id),
            Self::Shader(p) => p.get(scene, id),
        }
    }

    /// Write `v` through the property's authoring op; `false` once the component
    /// (or shader) is gone, which ends the tween.
    pub(crate) fn set(&self, scene: &mut Scene, id: u32, v: Vec4) -> bool {
        match self {
            Self::Field(f) => f.set(scene, id, v),
            Self::Shader(p) => p.set(scene, id, v),
        }
    }
}
