pub(crate) mod buffer;
mod stages;

use super::resources::SolidResources;
use super::stack::StackPass;
use crate::clusters::ClusterGrid;
use crate::gpu::commands::Commands;
use crate::gpu::uniforms::FogUniform;
use crate::lod::LodSelection;
use crate::passes::particles::ParticleDraws;
use crate::passes::ssao::SsaoPlan;
use crate::postfx::effects::ssil::{SsilFrame, SsilView};
use crate::postfx::effects::taa::jitter::jittered;
use crate::postfx::effects::volumetric_fog::{self, VolumetricPlan, VolumetricView};
use crate::{CameraUniform, Frustum, RenderView, Renderer};
use rusty_scene::{Camera, ClearFlags, Scene};
use stages::*;

pub(crate) const EARLY_FLUSHES: [&str; 2] = ["occlusion", "scene"];

pub(crate) fn flushes_after(flushes: crate::EarlyFlushes, name: &str) -> bool {
    EARLY_FLUSHES[..flushes.count()].contains(&name)
}

pub(crate) type CameraPass = fn(&mut Renderer, &mut RenderView, &mut CameraCtx);

/// The per-camera stages, in draw order. Append a feature's stage here (see the
/// module doc); the name is for tests and reading, not lookup.
pub(crate) const CAMERA_PASSES: &[(&str, CameraPass)] = &[
    ("camera_uniform", upload_camera),
    ("clusters", bin_clusters),
    ("volumetric_fog", volumetric_fog),
    ("solids", batch_solids),
    ("occlusion", occlusion),
    ("scene", scene_pass),
    ("ssil_source", ssil_source),
    ("ssr", ssr),
    ("opaque_copy", opaque_copy),
    ("transparent", transparent),
    ("depth_postpass", depth_postpass),
    ("world_ui", world_ui),
    ("effects", effects),
    ("gizmo_depth", gizmo_depth),
    ("count", count),
    ("post_fx", post_fx),
    ("outline", outline),
    ("handles", handles),
];

pub(crate) struct CameraCtx<'a> {
    pub scene: &'a Scene,
    pub cam: &'a Camera,
    pub stack: &'a StackPass<'a>,
    pub is_first: bool,
    pub is_last: bool,
    pub overlay: bool,
    pub view_proj: glam::Mat4,
    pub base_view_proj: glam::Mat4,
    pub frustum: Frustum,
    pub clusters: ClusterGrid,
    pub fog: FogUniform,
    pub ssao: Option<SsaoPlan>,
    pub ssil: Option<SsilFrame>,
    pub volumetric: Option<VolumetricView>,
    pub traces: bool,
    pub lod: LodSelection,
    pub uniform: CameraUniform,
    pub solids: SolidResources,
    pub effects: ParticleDraws,
    pub commands: Commands,
}

pub(crate) struct StackShared {
    pub aspect: f32,
    pub view_projs: Vec<glam::Mat4>,
    pub prev_view_projs: Vec<glam::Mat4>,
    pub prev_time: f32,
    pub taa: crate::postfx::effects::taa::TaaFrame,
    pub fog: FogUniform,
    pub ssao: Option<SsaoPlan>,
    pub ssil: Option<SsilView>,
    pub volumetric: Option<VolumetricView>,
    pub ssr_march: [f32; 4],
}

impl<'a> CameraCtx<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        stack: &'a StackPass<'a>,
        shared: &StackShared,
        (idx, commands): (usize, Commands),
    ) -> Self {
        let cam = &stack.stack[idx];
        let unjittered = shared.view_projs[idx];
        let view_proj = jittered(unjittered, shared.taa.jitter);
        let clusters = ClusterGrid::new(cam, shared.aspect);
        let overlay = is_overlay(idx == 0, cam.clear_flags);
        let (march, traces) = ssr_march(idx == 0, shared.ssr_march);
        let ssil = SsilFrame::for_camera(shared.ssil.filter(|_| idx == 0), view_proj, cam);
        let volumetric = volumetric_fog::for_camera(shared.volumetric, idx == 0);
        let plan = volumetric.as_ref().map(|(plan, _)| plan);
        Self {
            scene,
            cam,
            stack,
            is_first: idx == 0,
            is_last: idx + 1 == stack.stack.len(),
            overlay,
            view_proj,
            base_view_proj: shared.view_projs[0],
            frustum: Frustum::from_view_proj(unjittered),
            fog: shared.fog,
            ssao: shared.ssao.filter(|_| !overlay),
            traces,
            ssil,
            volumetric,
            lod: LodSelection::for_camera(scene, cam),
            uniform: CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                time: scene.shader_time,
                fog: shared.fog,
                clusters: clusters.uniform(),
                prev_view_proj: shared.prev_view_projs[idx].to_cols_array(),
                stack: [
                    idx as f32,
                    shared.prev_time,
                    shared.taa.jitter.x,
                    shared.taa.jitter.y,
                ],
                inv_view_proj: view_proj.inverse().to_cols_array(),
                ssr: march,
                ssil: [ssil.as_ref().map_or(0.0, |_| 1.0), 0.0, 0.0, 0.0],
                lod: [shared.taa.mip_bias, 0.0, 0.0, 0.0],
                volumetric: VolumetricPlan::camera_slot(plan),
            },
            clusters,
            solids: SolidResources::default(),
            effects: ParticleDraws::default(),
            commands,
        }
    }

    pub(crate) fn screen_space(&self) -> bool {
        self.ssao.is_some() || self.ssil.is_some()
    }
}

pub(crate) fn is_overlay(is_first: bool, clear_flags: ClearFlags) -> bool {
    !is_first && clear_flags == ClearFlags::DepthOnly
}

pub(crate) fn ssr_march(is_first: bool, march: [f32; 4]) -> ([f32; 4], bool) {
    let march = if is_first { march } else { [0.0; 4] };
    (march, march[0] > 0.0)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "encoder_gpu_tests.rs"]
mod encoder_gpu_tests;

#[cfg(test)]
thread_local! {
    pub(crate) static SPLIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn split_stages() -> bool {
    SPLIT.with(std::cell::Cell::get)
}
