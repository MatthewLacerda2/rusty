//! src/api/camera/mod.rs — `Camera` namespace.
//!
//! Get/Set for position, yaw, pitch and fov of the main view, plus `GetForward`/`GetRight` basis vectors so a controller
//! script can move and aim relative to where the camera looks, and the world ↔
//! screen projections (`WorldToScreen`, `ScreenToWorldRay`, #429). The pose is the
//! main camera entity's Transform and the fov its lens when the scene has one, else
//! the free view camera's (`scene::camera::view`, #1619); the per-entity functions (`component`) tune a camera *entity's*
//! projection and render-texture target (#430), lens and stacking (`lens`, #1341).
//! `ResetHistory` cuts a camera's view (#2003, `scene::camera::HistoryCuts`).

pub mod component;
mod lens;

use std::cell::RefCell;

use glam::{Vec2, Vec3};
use mlua::Lua;

use super::{put, ApiCtx, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::camera::{main_camera, set_view_fov, set_view_orientation, set_view_position};
use crate::scene::{game_camera_from_scene, Camera, Scene};

/// Register the `Camera` namespace onto `lua`.
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_position(lua, &table, ctx)?;
    register_orientation(lua, &table, ctx)?;
    register_fov(lua, &table, ctx)?;
    register_projection(lua, &table, ctx)?;
    register_reset_history(lua, &table, ctx)?;
    component::register(lua, &table, ctx.scene)?;

    lua.globals()
        .set("Camera", table)
        .map_err(|e| e.to_string())
}

/// The main view as it stands: the main camera entity's pose, else the free camera.
fn view(camera: &RefCell<Camera>, scene: &RefCell<Scene>) -> Camera {
    game_camera_from_scene(&camera.borrow(), &scene.borrow())
}

/// `GetPosition` / `SetPosition` plus the `GetForward` / `GetRight` basis vectors.
fn register_position(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    put(
        table,
        "GetPosition",
        lua.create_function(move |lua, ()| {
            let p = view(&*camera.get(lua)?, &*scene.get(lua)?).position;
            Ok((p.x, p.y, p.z))
        }),
    )?;

    put(
        table,
        "SetPosition",
        lua.create_function(move |lua, (x, y, z): (f32, f32, f32)| {
            let at = Vec3::new(x, y, z);
            set_view_position(
                &mut camera.get(lua)?.borrow_mut(),
                &mut scene.get(lua)?.borrow_mut(),
                at,
            );
            Ok(())
        }),
    )?;

    // Forward / right basis vectors derived from yaw+pitch. Pure engine math
    // (`Camera::forward`/`right`) exposed so a controller script can move and aim
    // relative to where the camera looks, instead of re-deriving the trig in Lua.
    put(
        table,
        "GetForward",
        lua.create_function(move |lua, ()| {
            let f = view(&*camera.get(lua)?, &*scene.get(lua)?).forward();
            Ok((f.x, f.y, f.z))
        }),
    )?;

    put(
        table,
        "GetRight",
        lua.create_function(move |lua, ()| {
            let r = view(&*camera.get(lua)?, &*scene.get(lua)?).right();
            Ok((r.x, r.y, r.z))
        }),
    )
}

/// `GetYaw` / `SetYaw` and `GetPitch` / `SetPitch` (pitch clamped to ±89°). Each
/// setter keeps the other angle the view has now.
fn register_orientation(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    let turn = move |lua: &mlua::Lua, yaw: Option<f32>, pitch: Option<f32>| {
        let (camera, scene) = (camera.get(lua)?, scene.get(lua)?);
        let now = view(&camera, &scene);
        let (yaw, pitch) = (yaw.unwrap_or(now.yaw), pitch.unwrap_or(now.pitch));
        set_view_orientation(
            &mut camera.borrow_mut(),
            &mut scene.borrow_mut(),
            yaw,
            pitch,
        );
        Ok(())
    };
    put(
        table,
        "GetYaw",
        lua.create_function(move |lua, ()| Ok(view(&*camera.get(lua)?, &*scene.get(lua)?).yaw)),
    )?;
    put(
        table,
        "SetYaw",
        lua.create_function(move |lua, yaw: f32| turn(lua, Some(yaw), None)),
    )?;

    put(
        table,
        "GetPitch",
        lua.create_function(move |lua, ()| Ok(view(&*camera.get(lua)?, &*scene.get(lua)?).pitch)),
    )?;
    put(
        table,
        "SetPitch",
        lua.create_function(move |lua, pitch: f32| turn(lua, None, Some(pitch))),
    )
}

/// `GetFov` / `SetFov` (fov clamped to 1..179°, the entity ops' bounds): the main
/// camera entity's lens when there is one, else the free camera's.
fn register_fov(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    put(
        table,
        "GetFov",
        lua.create_function(move |lua, ()| Ok(view(&*camera.get(lua)?, &*scene.get(lua)?).fov)),
    )?;
    put(
        table,
        "SetFov",
        lua.create_function(move |lua, fov: f32| {
            set_view_fov(
                &mut camera.get(lua)?.borrow_mut(),
                &mut scene.get(lua)?.borrow_mut(),
                fov,
            );
            Ok(())
        }),
    )
}

/// `WorldToScreen(x, y, z [, canvas])` → `sx, sy, depth, onScreen` — UI screen
/// pixels (bottom-left, y-up), or the canvas's reference units when a canvas id is
/// given; `depth` ≤ 0 is behind the camera. `ScreenToWorldRay(x, y)` → the ray's
/// origin and unit direction through a UI screen pixel.
fn register_projection(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let (camera, scene, screen, video) = (ctx.camera, ctx.scene, ctx.screen, ctx.video);
    let px = move |lua: &mlua::Lua| -> mlua::Result<_> {
        let (screen, video) = (screen.get(lua)?, video.get(lua)?);
        let px = screen.borrow().pixels(&video.borrow());
        Ok(px)
    };
    put(
        table,
        "WorldToScreen",
        lua.create_function(move |lua, (x, y, z, canvas): (f32, f32, f32, EntityArg)| {
            let scene = &*scene.get(lua)?;
            let canvas = canvas.get();
            let screen = px(lua)?;
            let p = view(&*camera.get(lua)?, scene).world_to_screen(Vec3::new(x, y, z), screen);
            let on_screen = p.is_on_screen(screen);
            let scale = canvas
                .and_then(|id| {
                    scene
                        .borrow()
                        .world
                        .canvas(id)
                        .map(|c| c.scale_factor(screen))
                })
                .unwrap_or(1.0);
            let at = p.position / scale;
            Ok((at.x, at.y, p.depth, on_screen))
        }),
    )?;
    put(
        table,
        "ScreenToWorldRay",
        lua.create_function(move |lua, (x, y): (f32, f32)| {
            let (o, d) =
                view(&*camera.get(lua)?, &*scene.get(lua)?).screen_ray(Vec2::new(x, y), px(lua)?);
            Ok((o.x, o.y, o.z, d.x, d.y, d.z))
        }),
    )
}

/// `ResetHistory([id])`: the next frame of camera `id`'s view (the screen's base
/// camera, `Camera.main`, when omitted or `nil`) starts every history over, URP's
/// `resetHistory`. A dead id or an entity with no camera does nothing.
fn register_reset_history(lua: &mlua::Lua, table: &mlua::Table, ctx: &ApiCtx) -> Reg {
    let scene = ctx.scene;
    put(
        table,
        "ResetHistory",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let camera = match id.get() {
                None => main_camera(&scene),
                Some(id) if scene.world.camera(id).is_some() => Some(id),
                Some(_) => return Ok(()),
            };
            scene.history_cuts.cut(camera);
            Ok(())
        }),
    )
}
