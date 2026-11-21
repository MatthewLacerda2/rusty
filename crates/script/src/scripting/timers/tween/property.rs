use glam::Vec4;

use super::field::Field;
use super::shader_param::{self, ShaderParam};
use rusty_scene::Scene;

#[derive(Debug)]
pub(crate) enum Property {
    Field(Field),
    Shader(Box<ShaderParam>),
}

impl Property {
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

    pub(crate) fn path(&self) -> &str {
        match self {
            Self::Field(f) => f.path(),
            Self::Shader(p) => p.path(),
        }
    }

    pub(crate) fn arity(&self) -> usize {
        match self {
            Self::Field(f) => f.arity(),
            Self::Shader(p) => p.arity(),
        }
    }

    pub(crate) fn get(&self, scene: &Scene, id: u32) -> Option<Vec4> {
        match self {
            Self::Field(f) => f.get(scene, id),
            Self::Shader(p) => p.get(scene, id),
        }
    }

    pub(crate) fn is_held(&self) -> bool {
        matches!(self, Self::Field(f) if f.is_held())
    }

    pub(crate) fn set(&self, scene: &mut Scene, id: u32, v: Vec4) -> bool {
        match self {
            Self::Field(f) => f.set(scene, id, v),
            Self::Shader(p) => p.set(scene, id, v),
        }
    }
}
