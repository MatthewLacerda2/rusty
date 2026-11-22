use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::blocks::{find, Block};
use super::recipe::{BlockSel, ParamValue, PassKind};
use super::{engine_shader_dir, DEFAULT_OUT_DIR};

/// One `vec4` per slot. The three WGSL declarations below spell 16 literally: change
/// them together.
pub const PARAM_SLOTS: usize = 16;

/// The material group's binding of the param uniform, after the main maps and albedo's
/// sampler; the detail map sits at 15, below the samplers.
pub const PARAM_BINDING: u32 = 6;

pub type PackedParams = [[f32; 4]; PARAM_SLOTS];

pub fn uniform_decl() -> String {
    format!(
        "struct ShaderParams {{\n    v: array<vec4<f32>, 16>,\n}};\n\
         @group({}) @binding({PARAM_BINDING}) var<uniform> shader_params: ShaderParams;\n",
        super::groups::MATERIAL
    )
}

/// Mirrors render's `UiShadeUniform`, one per graphic at a dynamic offset.
pub const UI_UNIFORM_DECL: &str = "struct UiShade {\n    origin_x: vec4<f32>,\n    axis_y_size: vec4<f32>,\n    time: vec4<f32>,\n    v: array<vec4<f32>, 16>,\n};\n@group(3) @binding(0) var<uniform> shader_params: UiShade;\n";

pub const POSTFX_UNIFORM_DECL: &str = "struct ShaderParams {\n    v: array<vec4<f32>, 16>,\n};\n@group(1) @binding(0) var<uniform> shader_params: ShaderParams;\n";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamSlot {
    pub block: String,
    pub index: usize,
    pub param: String,
    pub slot: usize,
    pub arity: usize,
    pub default: Vec<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamLayout {
    pub params: Vec<ParamSlot>,
}

impl ParamLayout {
    pub fn from_instances(
        pass: PassKind,
        instances: &[(&'static Block, &BlockSel)],
    ) -> Result<Self, String> {
        let mut params = Vec::new();
        for (index, (block, sel)) in instances.iter().enumerate() {
            for p in block.params.iter().filter(|p| p.runtime) {
                if params.len() == PARAM_SLOTS {
                    return Err(format!(
                        "more than {PARAM_SLOTS} runtime params in one {} shader",
                        pass.name()
                    ));
                }
                let default = match sel.params.get(p.name) {
                    Some(ParamValue::Vector(v)) => v.clone(),
                    Some(ParamValue::Scalar(s)) => vec![*s; p.arity],
                    Some(ParamValue::Name(_)) | None => p.defaults(),
                };
                params.push(ParamSlot {
                    block: block.id.to_owned(),
                    index,
                    param: p.name.to_owned(),
                    slot: params.len(),
                    arity: p.arity,
                    default,
                });
            }
        }
        Ok(Self { params })
    }

    pub fn slot_of(&self, index: usize, param: &str) -> Option<&ParamSlot> {
        self.params
            .iter()
            .find(|s| s.index == index && s.param == param)
    }

    pub fn name(&self, s: &ParamSlot) -> String {
        if self.instances_of(&s.block).len() == 1 {
            format!("{}.{}", s.block, s.param)
        } else {
            format!("{}.{}.{}", s.block, s.index, s.param)
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.params.iter().map(|s| self.name(s)).collect()
    }

    fn instances_of(&self, block: &str) -> Vec<usize> {
        let mut ix: Vec<usize> = self
            .params
            .iter()
            .filter(|s| s.block == block)
            .map(|s| s.index)
            .collect();
        ix.dedup();
        ix
    }

    pub fn resolve(&self, name: &str) -> Result<&ParamSlot, String> {
        let parts: Vec<&str> = name.split('.').collect();
        let (block, index, param) = match parts[..] {
            [b, p] => (b, None, p),
            [b, i, p] => (
                b,
                Some(i.parse::<usize>().map_err(|_| self.unknown(name))?),
                p,
            ),
            _ => return Err(self.unknown(name)),
        };
        if index.is_none() && self.instances_of(block).len() > 1 {
            return Err(format!(
                "shader param {name:?} is ambiguous: {block} appears more than once; use one of {}",
                self.names().join(", ")
            ));
        }
        self.params
            .iter()
            .find(|s| s.block == block && s.param == param && index.is_none_or(|i| s.index == i))
            .ok_or_else(|| self.unknown(name))
    }

    fn unknown(&self, name: &str) -> String {
        unknown_param(name, &self.names())
    }

    pub fn coerce(&self, s: &ParamSlot, value: Vec<f32>) -> Result<Vec<f32>, String> {
        match value.len() {
            1 => Ok(vec![value[0]; s.arity]),
            n if n == s.arity => Ok(value),
            n => Err(format!(
                "shader param {:?} takes {} number(s) (or one to broadcast), got {n}",
                self.name(s),
                s.arity
            )),
        }
    }

    pub fn pack(&self, values: &BTreeMap<String, Vec<f32>>) -> PackedParams {
        let mut out = [[0.0; 4]; PARAM_SLOTS];
        for s in &self.params {
            out[s.slot][..s.arity].copy_from_slice(&s.default[..s.arity]);
        }
        for (name, v) in values {
            if let Some(s) = self.resolve(name).ok().filter(|s| v.len() == s.arity) {
                out[s.slot][..s.arity].copy_from_slice(v);
            }
        }
        out
    }

    pub fn read_beside(wgsl: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(sidecar_path(wgsl)) {
            Ok(json) => serde_json::from_str(&json)
                .map_err(|e| format!("{}: {e}", sidecar_path(wgsl).display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub(crate) fn unknown_param(name: &str, names: &[String]) -> String {
    let block = name.split('.').next().unwrap_or_default();
    let baked = PassKind::ALL
        .into_iter()
        .filter_map(|pass| find(pass, block))
        .flat_map(|b| b.params)
        .any(|p| !p.runtime && name.ends_with(&format!(".{}", p.name)));
    let what = if baked {
        "is baked, not a runtime param"
    } else {
        "is not a runtime param of this shader"
    };
    let list = if names.is_empty() {
        "(none)".to_owned()
    } else {
        names.join(", ")
    };
    format!("shader param {name:?} {what}; runtime params: {list}")
}

pub fn is_shader_file(path: &str) -> bool {
    path.ends_with(".wgsl") || path.ends_with(".params.json")
}

pub fn sidecar_path(wgsl: &Path) -> PathBuf {
    wgsl.with_extension("params.json")
}

pub fn resolve_module(dirs: &[&str], name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    dirs.iter()
        .map(|dir| PathBuf::from(dir).join(format!("{name}.wgsl")))
        .find(|p| p.is_file())
}

pub fn load(name: &str) -> Result<ParamLayout, String> {
    // The list, in order, render's `SurfaceShaders::new` and `ui::custom::source` search.
    let dirs = [DEFAULT_OUT_DIR, engine_shader_dir()];
    let path = resolve_module(&dirs, name)
        .ok_or_else(|| format!("no shader named {name:?} (bake it with Shader.Bake)"))?;
    ParamLayout::read_beside(&path)
}
