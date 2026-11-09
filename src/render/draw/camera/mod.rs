//! src/render/draw/camera/mod.rs — the per-camera pass list (#636). Every camera of a
//! stack runs [`CAMERA_PASSES`] in order over one [`CameraCtx`], built once by
//! [`CameraCtx::new`]. The order is the table's order, reviewable in one place, the
//! same way `app/registry.rs` orders the sim systems.
//!
//! **Adding a render stage:** write a `fn(&mut Renderer, &mut RenderView,
//! &mut CameraCtx)` (usually a one-line call into the feature's own module, taking
//! what it needs from the context) and add one `(name, fn)` row to [`CAMERA_PASSES`]
//! where it belongs in the frame. A value the stage needs from every camera goes in
//! [`CameraCtx`]; a new camera-uniform field is set once, in [`CameraCtx::new`].
//! No trait objects and no registration at runtime: a static, ordered table.
//!
//! Seven stages are not every camera's: `volumetric_fog` (#1703), `ssil_source`
//! (#1750) and `ssr` run only on the base camera, over its opaque image (#1526),
//! and `gizmo_depth` keeps its depth for a game frame's gizmos under a stacked
//! camera (#1882); `post_fx` runs only on the last
//! camera (it composites the whole stack) and returns early on the others, and the
//! editor's `outline` and `handles` follow it there, onto the finished image, in
//! Unity's order: scene, post-FX, outline, handles, then the UI (#1224). An
//! overlay camera ([`is_overlay`], a viewmodel) runs every stage but skips the
//! base camera's screen-space work: no decals in its clusters, no SSAO (#1497),
//! no transparent SSR ([`ssr_march`], #1570).

mod stages;

use super::resources::SolidResources;
use super::stack::StackPass;
use crate::render::clusters::ClusterGrid;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::lod::LodSelection;
use crate::render::passes::particles::ParticleDraws;
use crate::render::passes::ssao::SsaoPlan;
use crate::render::postfx::effects::ssil::{SsilFrame, SsilView};
use crate::render::postfx::effects::taa::jitter::jittered;
use crate::render::postfx::effects::volumetric_fog::{self, VolumetricPlan, VolumetricView};
use crate::render::{CameraUniform, Frustum, RenderView, Renderer};
use crate::scene::{Camera, ClearFlags, Scene};
use stages::*;

/// One stage of a camera's frame.
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

/// Everything one camera's stages share, built once per camera.
pub(crate) struct CameraCtx<'a> {
    pub scene: &'a Scene,
    pub cam: &'a Camera,
    pub stack: &'a StackPass<'a>,
    /// This camera's place in the stack: the first clears, the last runs post-FX.
    pub is_first: bool,
    pub is_last: bool,
    /// A `DepthOnly` camera stacked on another (a viewmodel): see [`is_overlay`].
    pub overlay: bool,
    /// This camera's view-projection as it draws: jittered on a TAA view (#1702).
    pub view_proj: glam::Mat4,
    /// The stack's base (first) camera's unjittered view-projection, for post-FX.
    pub base_view_proj: glam::Mat4,
    /// World-space frustum, for culling off-screen entities (#330).
    pub frustum: Frustum,
    /// This camera's light-cluster grid (#434).
    pub clusters: ClusterGrid,
    pub fog: FogUniform,
    /// The stack's SSAO, or `None` on an overlay camera, which samples no AO.
    pub ssao: Option<SsaoPlan>,
    /// The view's SSIL (#1750) on its base camera; `None` on every other camera.
    pub ssil: Option<SsilFrame>,
    /// The view's volumetric fog (#1703) on its base camera; `None` on every other.
    pub volumetric: Option<VolumetricView>,
    /// Whether this camera's `receive_ssr` translucents trace SSR (#1570): the base
    /// camera of a view running it ([`ssr_march`]).
    pub traces: bool,
    /// The LOD level each entity shows to this camera (#472).
    pub lod: LodSelection,
    pub uniform: CameraUniform,
    /// Filled by the `solids` stage; empty before it.
    pub solids: SolidResources,
    /// Filled by the `effects` stage, for the counters.
    pub effects: ParticleDraws,
}

/// The per-stack values every camera's context reads.
pub(crate) struct StackShared {
    pub aspect: f32,
    /// Each camera's view-projection, bottom to top, and its last frame's (#1435),
    /// both unjittered.
    pub view_projs: Vec<glam::Mat4>,
    pub prev_view_projs: Vec<glam::Mat4>,
    /// The game time the view drew at last frame (#1524), the shader time without one.
    pub prev_time: f32,
    /// TAA's jitter this frame, in NDC, and the material maps' mip bias while it
    /// upscales (#1702): zero for both without TAA.
    pub taa: crate::render::postfx::effects::taa::TaaFrame,
    pub fog: FogUniform,
    pub ssao: Option<SsaoPlan>,
    /// The view's SSIL (#1750) and what its history is valid for, when it runs.
    pub ssil: Option<SsilView>,
    /// The view's volumetric fog (#1703) and what its history is valid for.
    pub volumetric: Option<VolumetricView>,
    /// The base camera's transparent SSR march (#1570), `[steps, step, min
    /// smoothness, fade width]`; zero when the view runs no SSR.
    pub ssr_march: [f32; 4],
}

impl<'a> CameraCtx<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        stack: &'a StackPass<'a>,
        shared: &StackShared,
        idx: usize,
    ) -> Self {
        let cam = &stack.stack[idx];
        // Drawn jittered (TAA, #1702); culled through the unjittered matrix.
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
        }
    }
}

/// Whether a camera is an overlay: `DepthOnly` and stacked on another (a viewmodel,
/// a HUD camera). An overlay draws over its base camera's image and shares its
/// screen-space work: it bins no decals and runs no SSAO of its own (#1497, Unity
/// URP's rule: SSAO is a base-camera renderer feature).
pub(crate) fn is_overlay(is_first: bool, clear_flags: ClearFlags) -> bool {
    !is_first && clear_flags == ClearFlags::DepthOnly
}

/// The transparent SSR march `[steps, step, min smoothness, fade width]` a camera hands its
/// translucents, and whether it traces at all: the stack's `march` on its base
/// (first) camera, none on an overlay or any other stacked camera (SSR is the base
/// camera's alone).
pub(crate) fn ssr_march(is_first: bool, march: [f32; 4]) -> ([f32; 4], bool) {
    let march = if is_first { march } else { [0.0; 4] };
    (march, march[0] > 0.0)
}

#[cfg(test)]
mod tests;
