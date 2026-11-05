//! src/api/camera/component.rs — the `Camera` functions that take an entity id and
//! tune its `CameraComponent` (#430): projection, culling mask (#827), occlusion
//! culling (#1714) and render-texture target, plus the lens and stacking fields
//! (`lens`, #1341). Every
//! setter routes through `scene::authoring::rendering::camera`, the ops the inspector's Camera
//! card uses. Getters return `nil` without a camera; setters are then no-ops.

use std::cell::RefCell;

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use crate::components::{CameraComponent, Projection};
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// Register the per-entity camera functions onto the `Camera` table.
pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetProjection",
        lua.create_function(move |lua, id: EntityArg| {
            let got = read(&*scene.get(lua)?, id.read(), |c| {
                let size = match c.projection {
                    Projection::Orthographic { size } => Some(size),
                    Projection::Perspective => None,
                };
                (camera_ops::projection_name(c.projection), size)
            });
            Ok(got.map_or((None, None), |(name, size)| (Some(name), size)))
        }),
    )?;
    put(
        table,
        "SetProjection",
        lua.create_function(
            move |lua, (id, name, size): (EntityArg, String, Option<f32>)| {
                let id = id.live("Camera", "SetProjection")?;
                write(&*scene.get(lua)?, id, |c| {
                    let current = match c.projection {
                        Projection::Orthographic { size } => size,
                        Projection::Perspective => 5.0,
                    };
                    if let Some(p) = camera_ops::parse_projection(&name, size.unwrap_or(current)) {
                        camera_ops::set_projection(c, p);
                    }
                });
                Ok(())
            },
        ),
    )?;
    register_culling_mask(lua, table, scene)?;
    register_occlusion_culling(lua, table, scene)?;
    super::lens::register(lua, table, scene)?;
    register_target(lua, table, scene)
}

/// `Get/SetCullingMask`: one bit per layer, Unity's `Camera.cullingMask`. The mask
/// is taken as any Lua integer and kept to its low 32 bits, so `-1` (`~0`) is
/// "everything", as in Unity.
fn register_culling_mask(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetCullingMask",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| c.culling_mask))
        }),
    )?;
    put(
        table,
        "SetCullingMask",
        lua.create_function(move |lua, (id, mask): (EntityArg, i64)| {
            let id = id.live("Camera", "SetCullingMask")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_culling_mask(c, mask as u32)
            });
            Ok(())
        }),
    )
}

/// `Get/SetUseOcclusionCulling` (#1714): Unity's `Camera.useOcclusionCulling`,
/// whether the camera skips what the scene's baked PVS hides from its cell.
fn register_occlusion_culling(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetUseOcclusionCulling",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| {
                c.use_occlusion_culling
            }))
        }),
    )?;
    put(
        table,
        "SetUseOcclusionCulling",
        lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let id = id.live("Camera", "SetUseOcclusionCulling")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_use_occlusion_culling(c, on)
            });
            Ok(())
        }),
    )
}

/// `Get/SetTargetTexture`, then the target's settings.
fn register_target(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetTargetTexture",
        lua.create_function(move |lua, id: EntityArg| {
            let target = read(&*scene.get(lua)?, id.read(), |c| c.target_texture.clone()).flatten();
            Ok(target.map_or((None, None, None), |t| {
                (Some(t.name), Some(t.width), Some(t.height))
            }))
        }),
    )?;
    put(
        table,
        "SetTargetTexture",
        lua.create_function(
            move |lua, (id, name, w, h): (EntityArg, Option<String>, Option<u32>, Option<u32>)| {
                let id = id.live("Camera", "SetTargetTexture")?;
                write(&*scene.get(lua)?, id, |c| {
                    let (dw, dh) = c
                        .target_texture
                        .as_ref()
                        .map_or((256, 256), |t| (t.width, t.height));
                    let name = name.unwrap_or_default();
                    camera_ops::set_target_texture(c, &name, w.unwrap_or(dw), h.unwrap_or(dh));
                });
                Ok(())
            },
        ),
    )?;
    register_target_settings(lua, table, scene)
}

/// `Get/SetTargetPostFx`, `Get/SetTargetUpdateEvery`.
fn register_target_settings(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetTargetPostFx",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| {
                c.target_texture.as_ref().map(|t| t.post_fx)
            })
            .flatten())
        }),
    )?;
    put(
        table,
        "SetTargetPostFx",
        lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let id = id.live("Camera", "SetTargetPostFx")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_target_post_fx(c, on)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTargetUpdateEvery",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| {
                c.target_texture.as_ref().map(|t| t.update_every)
            })
            .flatten())
        }),
    )?;
    put(
        table,
        "SetTargetUpdateEvery",
        lua.create_function(move |lua, (id, n): (EntityArg, u32)| {
            let id = id.live("Camera", "SetTargetUpdateEvery")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_target_update_every(c, n)
            });
            Ok(())
        }),
    )
}

/// `f` over `id`'s camera, `None` without one.
pub(super) fn read<R>(
    scene: &RefCell<Scene>,
    id: u32,
    f: impl FnOnce(&CameraComponent) -> R,
) -> Option<R> {
    scene.borrow().world.camera(id).map(|c| f(&c))
}

/// Run `f` on `id`'s camera when it has one.
pub(super) fn write(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut CameraComponent)) {
    if let Some(mut c) = scene.borrow_mut().world.camera_mut(id) {
        f(&mut c);
    }
}
