//! `Graphics.{Get,Set}Bloom{Active,Intensity,Threshold}` and the lens dirt (#1442),
//! `Graphics.{Get,Set}BloomDirt{Texture,Intensity}`, over the active volume.

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::rendering::post_processing::bloom;
use crate::scene::Scene;

/// The switch (`false` without an active volume), the dirt texture (`nil` without
/// one, or without an active volume) and the knobs.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let active = |vc: &crate::components::PostProcessingComponent| vc.bloom.active;
    super::knobs::register_flag(
        scope,
        (table, scene),
        "BloomActive",
        active,
        bloom::set_active,
    )?;
    put(
        table,
        "SetBloomDirtTexture",
        scope.create_function(|_, path: Option<String>| {
            with_vc_mut(scene, |e| bloom::set_dirt_texture(e, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomDirtTexture",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.bloom.dirt_texture.clone()).flatten())
        }),
    )?;
    super::knobs::register(scope, table, scene, bloom::EFFECT.knobs)
}
