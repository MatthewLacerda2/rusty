//! `Graphics.{Get,Set}VolumetricFog{Active,Quality,Resolution,Slices,Albedo}` and
//! the knobs, `Graphics.{Get,Set}VolumetricFog{MeanFreePath,Anisotropy,BaseHeight,
//! MaximumHeight,DepthExtent,GlobalLightProbeDimmer}` (#1703), over the active volume.

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::{register_count, register_flag, register_quality};
use crate::api::ApiCell;
use rusty_components::VolumetricFog;
use rusty_scene::authoring::rendering::post_processing::volumetric_fog as fog;
use rusty_scene::Scene;

/// The switch, the quality level, its `Custom` counts, the albedo and the knobs (a
/// new volume's values without an active volume, `false` for the switch).
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let cx = (table, scene);
    let active = |vc: &rusty_components::PostProcessingComponent| vc.volumetric_fog.active;
    register_flag(lua, cx, "VolumetricFogActive", active, fog::set_active)?;
    let quality = |vc: &rusty_components::PostProcessingComponent| vc.volumetric_fog.quality;
    register_quality(lua, cx, "VolumetricFogQuality", quality, fog::set_quality)?;
    let resolution = |vc: &rusty_components::PostProcessingComponent| vc.volumetric_fog.resolution;
    register_count(
        lua,
        cx,
        "VolumetricFogResolution",
        resolution,
        fog::set_resolution,
    )?;
    let slices = |vc: &rusty_components::PostProcessingComponent| vc.volumetric_fog.slices;
    register_count(lua, cx, "VolumetricFogSlices", slices, fog::set_slices)?;
    register_albedo(lua, table, scene)?;
    super::knobs::register(lua, table, scene, fog::EFFECT.knobs)
}

/// `SetVolumetricFogAlbedo(r, g, b)` / `GetVolumetricFogAlbedo() -> r, g, b`.
fn register_albedo(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetVolumetricFogAlbedo",
        lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| fog::set_albedo(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVolumetricFogAlbedo",
        lua.create_function(move |lua, ()| {
            let fallback = VolumetricFog::default().albedo;
            let read = with_vc(&*scene.get(lua)?, |vc| vc.volumetric_fog.albedo);
            let [r, g, b] = read.unwrap_or(fallback);
            Ok((r, g, b))
        }),
    )
}
