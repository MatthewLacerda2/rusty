//! src/shadergen/params.rs — the runtime params of a baked surface or ui shader
//! (#399, #427): Unity's `material.SetFloat`.
//!
//! A surface (or ui) block param marked [`Param::runtime`](super::blocks::Param) is not
//! baked as a WGSL `const`: the assembler reads it from slot `n` of the material's
//! param uniform (`@group(2) @binding(6)`, a fixed `array<vec4<f32>, 16>`), so a
//! script changes it with a buffer write — no re-bake, no pipeline rebuild. The
//! assembler records `param → slot` in a [`ParamLayout`], baked beside the module as
//! `<name>.params.json`, so the API and the renderer resolve names without parsing
//! WGSL.
//!
//! **Names** reuse the instance scheme (#393): `"<block>.<param>"` when the block
//! appears once in the recipe, `"<block>.<index>.<param>"` always (and required when
//! it appears more than once). Values are stored on the material by the canonical
//! name [`ParamLayout::name`] gives.
//!
//! A **ui** shader (#427) packs the same slots into its per-graphic uniform
//! ([`UI_UNIFORM_DECL`]) behind a small header (the graphic's rect and the clock),
//! and the values live on the graphic instead of a material.
//!
//! A **postfx** shader (#671) reads the same 16 slots from its own uniform at
//! [`POSTFX_UNIFORM_DECL`] (group 1, beside the chain's shared IO group), and the
//! values live on the post-processing volume — see [`super::post_params`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::blocks::{find, Block};
use super::recipe::{BlockSel, ParamValue, PassKind};
use super::{DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// Runtime param slots per material: one `vec4` each, so 16 slots = 64 floats.
pub const PARAM_SLOTS: usize = 16;

/// The group-2 binding the param uniform sits at (after the five maps + sampler).
pub const PARAM_BINDING: u32 = 6;

/// A material's param uniform, as the GPU reads it.
pub type PackedParams = [[f32; 4]; PARAM_SLOTS];

/// The WGSL a surface variant with runtime params declares (binding = [`PARAM_BINDING`]).
pub const UNIFORM_DECL: &str = "struct ShaderParams {\n    v: array<vec4<f32>, 16>,\n};\n@group(2) @binding(6) var<uniform> shader_params: ShaderParams;\n";

/// The WGSL every ui variant declares (#427): its graphic's rect in canvas NDC
/// (origin, x and y axes), the rect's size in target pixels, the clock, and the 16
/// runtime-param slots — one per graphic, at a dynamic offset.
pub const UI_UNIFORM_DECL: &str = "struct UiShade {\n    origin_x: vec4<f32>,\n    axis_y_size: vec4<f32>,\n    time: vec4<f32>,\n    v: array<vec4<f32>, 16>,\n};\n@group(3) @binding(0) var<uniform> shader_params: UiShade;\n";

/// The WGSL a postfx variant with runtime params declares (#671): the same slots as
/// [`UNIFORM_DECL`], in the effect's own bind group after the chain's IO group.
pub const POSTFX_UNIFORM_DECL: &str = "struct ShaderParams {\n    v: array<vec4<f32>, 16>,\n};\n@group(1) @binding(0) var<uniform> shader_params: ShaderParams;\n";

/// One runtime param: which block instance and param it is, its uniform slot, its
/// lane count, and its baked default (the recipe's value, else the block's).
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

/// A baked surface shader's runtime params, in slot order (the sidecar document).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamLayout {
    pub params: Vec<ParamSlot>,
}

impl ParamLayout {
    /// Allocate a slot per runtime param of `instances`, in recipe then catalog
    /// order. Errors past [`PARAM_SLOTS`].
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
                        pass.tag()
                    ));
                }
                let default = match sel.params.get(p.name) {
                    Some(ParamValue::Vector(v)) => v.clone(),
                    Some(ParamValue::Scalar(s)) => vec![*s; p.arity],
                    None => vec![p.default; p.arity],
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

    /// The slot of instance `index`'s param `param`, if it is runtime.
    pub fn slot_of(&self, index: usize, param: &str) -> Option<&ParamSlot> {
        self.params
            .iter()
            .find(|s| s.index == index && s.param == param)
    }

    /// `s`'s canonical name: `block.param` if its block appears once, else
    /// `block.index.param`.
    pub fn name(&self, s: &ParamSlot) -> String {
        if self.instances_of(&s.block).len() == 1 {
            format!("{}.{}", s.block, s.param)
        } else {
            format!("{}.{}.{}", s.block, s.index, s.param)
        }
    }

    /// Every runtime param's canonical name, in slot order.
    pub fn names(&self) -> Vec<String> {
        self.params.iter().map(|s| self.name(s)).collect()
    }

    /// The distinct instance indices of `block` holding runtime params.
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

    /// Resolve a script-facing name to its slot; the error names the culprit and
    /// lists the runtime params there are.
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

    /// Fit `value` to `s`: exactly `arity` numbers, or one broadcast to every lane.
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

    /// The uniform for a material: each slot's default, overridden by the
    /// material's stored values (names that no longer resolve are skipped).
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

    /// Read the sidecar beside `wgsl`; a module baked before #399 (no sidecar) has
    /// no runtime params.
    pub fn read_beside(wgsl: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(sidecar_path(wgsl)) {
            Ok(json) => serde_json::from_str(&json)
                .map_err(|e| format!("{}: {e}", sidecar_path(wgsl).display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The error for a name no runtime param answers to: whether the catalog bakes a
/// param of that name, and the runtime params there are (`names`).
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

/// `<dir>/<name>.params.json` for `<dir>/<name>.wgsl`.
pub fn sidecar_path(wgsl: &Path) -> PathBuf {
    wgsl.with_extension("params.json")
}

/// Where shader `name` resolves, searched in the renderer's order (authored
/// workspace, then the engine set); `None` for a missing or non-bare name.
pub fn resolve_module(dirs: &[&str], name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    dirs.iter()
        .map(|dir| PathBuf::from(format!("{dir}/{name}.wgsl")))
        .find(|p| p.is_file())
}

/// The runtime params of shader `name` (surface or ui), as the renderer will resolve it.
pub fn load(name: &str) -> Result<ParamLayout, String> {
    let dirs = [DEFAULT_OUT_DIR, ENGINE_SHADER_DIR];
    let path = resolve_module(&dirs, name)
        .ok_or_else(|| format!("no shader named {name:?} (bake it with Shader.Bake)"))?;
    ParamLayout::read_beside(&path)
}
