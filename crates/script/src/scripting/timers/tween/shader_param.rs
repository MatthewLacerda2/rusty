use glam::Vec4;

use rusty_edit::authoring::{
    material, rendering::camera as camera_ops, rendering::post_processing as vc_ops,
    ui::shader as ui_shader,
};
use rusty_scene::Scene;
use rusty_shadergen::params::ParamLayout;
use rusty_shadergen::post_params::PostChain;

pub(crate) const PATHS: &str = "Material.<block.param>, UI.<block.param>, \
    Graphics.<block.param>";

#[derive(Debug)]
pub(crate) struct ShaderParam {
    path: String,
    name: String,
    arity: usize,
    source: Source,
}

#[derive(Debug)]
enum Source {
    Material(ParamLayout),
    Ui { shader: String, layout: ParamLayout },
    Post(PostChain),
    Knob(&'static vc_ops::Knob),
    CameraKnob(&'static camera_ops::CameraKnob),
}

impl ShaderParam {
    pub(crate) fn parse(scene: &Scene, id: u32, path: &str) -> Option<Result<Self, String>> {
        let (prefix, name) = path.split_once('.')?;
        let source = match prefix {
            "Material" => material_source(scene, id),
            "UI" => ui_source(scene, id),
            "Graphics" => post_source(scene, id, name),
            _ => return None,
        };
        let param = source.and_then(|source| {
            let arity = match &source {
                Source::Material(layout) | Source::Ui { layout, .. } => layout.resolve(name)?.arity,
                Source::Post(chain) => chain.resolve(name)?.1.arity,
                Source::Knob(_) | Source::CameraKnob(_) => 1,
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
                vc_ops::custom_effects::param_with(&*w.post_processing(id)?, chain, &self.name)
            }
            Source::Knob(knob) => Ok(vec![(knob.get)(&*w.post_processing(id)?)]),
            Source::CameraKnob(knob) => Ok(vec![(knob.get)(&*w.camera(id)?)]),
        };
        value.ok().map(|v| lanes(&v))
    }

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
            Source::Post(chain) => scene.world.post_processing_mut(id).is_some_and(|mut vc| {
                vc_ops::custom_effects::set_param_with(&mut vc, chain, &self.name, value).is_ok()
            }),
            Source::Knob(knob) => scene
                .world
                .post_processing_mut(id)
                .map(|mut vc| (knob.set)(&mut vc, v.x))
                .is_some(),
            Source::CameraKnob(knob) => scene
                .world
                .camera_mut(id)
                .map(|mut c| (knob.set)(&mut c, v.x))
                .is_some(),
        }
    }
}

fn material_source(scene: &Scene, id: u32) -> Result<Source, String> {
    let key = scene
        .world
        .material(id)
        .map(|m| m.first().to_string())
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

fn post_source(scene: &Scene, id: u32, name: &str) -> Result<Source, String> {
    if let Some(knob) = camera_ops::camera_knob(name) {
        return scene
            .world
            .camera(id)
            .map(|_| Source::CameraKnob(knob))
            .ok_or_else(|| format!("entity {id} has no Camera"));
    }
    let vc = scene
        .world
        .post_processing(id)
        .ok_or_else(|| format!("entity {id} has no post-processing volume"))?;
    if let Some(knob) = vc_ops::knob(name) {
        return Ok(Source::Knob(knob));
    }
    Ok(Source::Post(PostChain::load(&vc.custom_effects.names)))
}

fn lanes(v: &[f32]) -> Vec4 {
    let mut out = Vec4::ZERO;
    for (lane, x) in out.as_mut().iter_mut().zip(v) {
        *lane = *x;
    }
    out
}
