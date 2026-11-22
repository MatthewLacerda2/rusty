mod postfx;
mod surface;
mod ui;

use super::recipe::PassKind;

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub default: f32,
    pub arity: usize,
    pub runtime: bool,
    pub lanes: &'static [f32],
    pub choices: &'static [&'static str],
}

impl Param {
    pub const fn baked(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: false,
            lanes: &[],
            choices: &[],
        }
    }

    pub const fn live(name: &'static str, default: f32, arity: usize) -> Self {
        Self {
            name,
            default,
            arity,
            runtime: true,
            lanes: &[],
            choices: &[],
        }
    }

    pub const fn choice(name: &'static str, choices: &'static [&'static str]) -> Self {
        Self {
            choices,
            ..Self::baked(name, 0.0, 1)
        }
    }

    pub const fn with_lanes(self, lanes: &'static [f32]) -> Self {
        Self { lanes, ..self }
    }

    pub fn defaults(&self) -> Vec<f32> {
        match self.lanes.is_empty() {
            true => vec![self.default; self.arity],
            false => self.lanes.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Color,
    Uv,
    Vertex,
}

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub id: &'static str,
    pub desc: &'static str,
    pub params: &'static [Param],
    pub helper: &'static str,
    pub call: &'static str,
    pub textures: &'static [&'static str],
    pub stage: Stage,
    pub cut: Option<&'static str>,
    pub reflection: Reflection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reflection {
    Kept,
    Scaled(&'static str),
    Dropped,
}

pub fn catalog(pass: PassKind) -> &'static [Block] {
    match pass {
        PassKind::Surface => surface::BLOCKS,
        PassKind::Postfx => postfx::BLOCKS,
        PassKind::Ui => ui::BLOCKS,
    }
}

pub fn find(pass: PassKind, id: &str) -> Option<&'static Block> {
    catalog(pass).iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests;
