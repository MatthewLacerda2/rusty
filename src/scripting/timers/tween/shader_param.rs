//! Runtime shader params a tween may drive (#661): `"Material.<param>"`,
//! `"UI.<param>"` and `"Graphics.<param>"`, where `<param>` is the name the
//! matching setter takes — `block.param` or `block.index.param`.
//!
//! None of the fixed `"Component.field"` paths starts with those three words, so a
//! path is one or the other. The shader's layout (its `<name>.params.json`
//! sidecar, or a volume's chain of them) is read **once**, at `Tween.To`; every
//! step then reads and writes through the same `scene::authoring` ops as
//! `Material.SetShaderParam` (the entity's own override, #670), `UI.SetShaderParam`
//! and `Graphics.SetPostParam`, so a name is checked and a value coerced in one
//! place.

use glam::Vec4;

use crate::scene::authoring::{material, ui_shader, visual_correction as vc_ops};
use crate::scene::Scene;
use crate::shadergen::params::ParamLayout;
use crate::shadergen::post_params::PostChain;

/// The three shader-param path shapes, for the error a typo gets.
pub(crate) const PATHS: &str = "Material.<block.param>, UI.<block.param>, \
    Graphics.<block.param>";

/// One runtime shader param of one entity, its layout already resolved.
#[derive(Debug)]
pub(crate) struct ShaderParam {
    path: String,
    /// The param name, as the setter takes it.
    name: String,
    arity: usize,
    source: Source,
}

#[derive(Debug)]
enum Source {
    /// The entity's own override of its material's shader param (#670).
    Material(ParamLayout),
    /// Every graphic of the entity drawn with the ui shader `shader` (#427).
    Ui { shader: String, layout: ParamLayout },
    /// The effects of the entity's post-processing volume (#671).
    Post(PostChain),
}

impl ShaderParam {
    /// Resolve `path` on entity `id`: `None` when it is not a shader-param path,
    /// else the param or why it cannot be tweened (no such component, no shader,
    /// a baked or unknown param — the op's own error, listing the runtime params).
    pub(crate) fn parse(scene: &Scene, id: u32, path: &str) -> Option<Result<Self, String>> {
        let (prefix, name) = path.split_once('.')?;
        let source = match prefix {
            "Material" => material_source(scene, id),
            "UI" => ui_source(scene, id),
            "Graphics" => post_source(scene, id),
            _ => return None,
        };
        let param = source.and_then(|source| {
            let arity = match &source {
                Source::Material(layout) | Source::Ui { layout, .. } => layout.resolve(name)?.arity,
                Source::Post(chain) => chain.resolve(name)?.1.arity,
            };
            Ok(Self {
                path: path.to_owned(),
                name: name.to_owned(),
                arity,
                source,
            })
        });
        Some(param.map_err(|e| format!("tween property `{path}`: {e}")))
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn arity(&self) -> usize {
        self.arity
    }

    /// The value the param draws with, or `None` once its component or shader is gone.
    pub(crate) fn get(&self, scene: &Scene, id: u32) -> Option<Vec4> {
        let w = &scene.world;
        let value = match &self.source {
            Source::Material(layout) => {
                w.material(id)?;
                material::entity_shader_param_with(scene, id, layout, &self.name)
            }
            Source::Ui { shader, layout } => {
                let slot = ui_shader::graphic_shader(w, id)?;
                if slot.as_ref()?.name != *shader {
                    return None;
                }
                ui_shader::param(&slot, layout, &self.name)
            }
            Source::Post(chain) => {
                vc_ops::post_param_with(&*w.visual_correction(id)?, chain, &self.name)
            }
        };
        value.ok().map(|v| lanes(&v))
    }

    /// Write `v`'s first `arity` lanes. `false` once its component or shader is gone.
    pub(crate) fn set(&self, scene: &mut Scene, id: u32, v: Vec4) -> bool {
        let value = v.to_array()[..self.arity].to_vec();
        match &self.source {
            Source::Material(layout) => {
                scene.world.material(id).is_some()
                    && material::set_entity_shader_param_with(scene, id, layout, &self.name, value)
                        .is_ok()
            }
            Source::Ui { shader, layout } => {
                let mut written = false;
                ui_shader::for_each_graphic(&mut scene.world, id, |slot| {
                    if slot.as_ref().is_some_and(|s| s.name == *shader) {
                        written |=
                            ui_shader::set_param(slot, layout, &self.name, value.clone()).is_ok();
                    }
                });
                written
            }
            Source::Post(chain) => scene.world.visual_correction_mut(id).is_some_and(|mut vc| {
                vc_ops::set_post_param_with(&mut vc, chain, &self.name, value).is_ok()
            }),
        }
    }
}

fn material_source(scene: &Scene, id: u32) -> Result<Source, String> {
    let key = scene
        .world
        .material(id)
        .map(|m| m.material.clone())
        .ok_or_else(|| format!("entity {id} has no Material"))?;
    material::shader_layout(&scene.materials, &key).map(Source::Material)
}

fn ui_source(scene: &Scene, id: u32) -> Result<Source, String> {
    let slot = ui_shader::graphic_shader(&scene.world, id)
        .ok_or_else(|| format!("entity {id} has no Image, Shape or Text"))?;
    let layout = ui_shader::layout(&slot)?;
    let shader = slot.map(|s| s.name).unwrap_or_default();
    Ok(Source::Ui { shader, layout })
}

fn post_source(scene: &Scene, id: u32) -> Result<Source, String> {
    let vc = scene
        .world
        .visual_correction(id)
        .ok_or_else(|| format!("entity {id} has no post-processing volume"))?;
    Ok(Source::Post(PostChain::load(&vc.custom_effects)))
}

/// A param value (1–4 numbers) as a tween's `Vec4`.
fn lanes(v: &[f32]) -> Vec4 {
    let mut out = Vec4::ZERO;
    for (lane, x) in out.as_mut().iter_mut().zip(v) {
        *lane = *x;
    }
    out
}
