//! src/api/camera.rs — `Camera` namespace.
//!
//! Get/Set for position, yaw, pitch and fov of the main view, plus `GetForward`/`GetRight` basis vectors so a controller
//! script can move and aim relative to where the camera looks, and the world ↔
//! screen projections (`WorldToScreen`, `ScreenToWorldRay`, #429). The pose is the
//! main camera entity's Transform and the fov its lens when the scene has one, else
//! the free view camera's (`scene::camera::view`, #1619); the per-entity functions (`component`) tune a camera *entity's*
//! projection and render-texture target (#430), lens and stacking (`lens`, #1341).

pub mod component;
mod lens;

use std::cell::RefCell;

use glam::{Vec2, Vec3};
use mlua::Lua;

use super::{put, ApiScopedCtx, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::camera::{set_view_fov, set_view_orientation, set_view_position};
use crate::scene::{game_camera_from_scene, Camera, Scene};

/// Register the `Camera` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_position(scope, &table, ctx)?;
    register_orientation(scope, &table, ctx)?;
    register_fov(scope, &table, ctx)?;
    register_projection(scope, &table, ctx)?;
    component::register(scope, &table, ctx.scene)?;

    lua.globals()
        .set("Camera", table)
        .map_err(|e| e.to_string())
}

/// The main view as it stands: the main camera entity's pose, else the free camera.
fn view(camera: &RefCell<Camera>, scene: &RefCell<Scene>) -> Camera {
    game_camera_from_scene(&camera.borrow(), &scene.borrow())
}

/// `GetPosition` / `SetPosition` plus the `GetForward` / `GetRight` basis vectors.
fn register_position<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    put(
        table,
        "GetPosition",
        scope.create_function(move |_, ()| {
            let p = view(camera, scene).position;
            Ok((p.x, p.y, p.z))
        }),
    )?;

    put(
        table,
        "SetPosition",
        scope.create_function(move |_, (x, y, z): (f32, f32, f32)| {
            let at = Vec3::new(x, y, z);
            set_view_position(&mut camera.borrow_mut(), &mut scene.borrow_mut(), at);
            Ok(())
        }),
    )?;

    // Forward / right basis vectors derived from yaw+pitch. Pure engine math
    // (`Camera::forward`/`right`) exposed so a controller script can move and aim
    // relative to where the camera looks, instead of re-deriving the trig in Lua.
    put(
        table,
        "GetForward",
        scope.create_function(move |_, ()| {
            let f = view(camera, scene).forward();
            Ok((f.x, f.y, f.z))
        }),
    )?;

    put(
        table,
        "GetRight",
        scope.create_function(move |_, ()| {
            let r = view(camera, scene).right();
            Ok((r.x, r.y, r.z))
        }),
    )
}

/// `GetYaw` / `SetYaw` and `GetPitch` / `SetPitch` (pitch clamped to ±89°). Each
/// setter keeps the other angle the view has now.
fn register_orientation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    let turn = move |yaw: Option<f32>, pitch: Option<f32>| {
        let now = view(camera, scene);
        let (yaw, pitch) = (yaw.unwrap_or(now.yaw), pitch.unwrap_or(now.pitch));
        set_view_orientation(
            &mut camera.borrow_mut(),
            &mut scene.borrow_mut(),
            yaw,
            pitch,
        );
    };
    put(
        table,
        "GetYaw",
        scope.create_function(move |_, ()| Ok(view(camera, scene).yaw)),
    )?;
    put(
        table,
        "SetYaw",
        scope.create_function(move |_, yaw: f32| {
            turn(Some(yaw), None);
            Ok(())
        }),
    )?;

    put(
        table,
        "GetPitch",
        scope.create_function(move |_, ()| Ok(view(camera, scene).pitch)),
    )?;
    put(
        table,
        "SetPitch",
        scope.create_function(move |_, pitch: f32| {
            turn(None, Some(pitch));
            Ok(())
        }),
    )
}

/// `GetFov` / `SetFov` (fov clamped to 1..179°, the entity ops' bounds): the main
/// camera entity's lens when there is one, else the free camera's.
fn register_fov<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (camera, scene) = (ctx.camera, ctx.scene);
    put(
        table,
        "GetFov",
        scope.create_function(move |_, ()| Ok(view(camera, scene).fov)),
    )?;
    put(
        table,
        "SetFov",
        scope.create_function(move |_, fov: f32| {
            set_view_fov(&mut camera.borrow_mut(), &mut scene.borrow_mut(), fov);
            Ok(())
        }),
    )
}

/// `WorldToScreen(x, y, z [, canvas])` → `sx, sy, depth, onScreen` — UI screen
/// pixels (bottom-left, y-up), or the canvas's reference units when a canvas id is
/// given; `depth` ≤ 0 is behind the camera. `ScreenToWorldRay(x, y)` → the ray's
/// origin and unit direction through a UI screen pixel.
fn register_projection<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (camera, scene, screen, video) = (ctx.camera, ctx.scene, ctx.screen, ctx.video);
    let px = move || screen.borrow().pixels(&video.borrow());
    put(
        table,
        "WorldToScreen",
        scope.create_function(move |_, (x, y, z, canvas): (f32, f32, f32, EntityArg)| {
            let canvas = canvas.get();
            let screen = px();
            let p = view(camera, scene).world_to_screen(Vec3::new(x, y, z), screen);
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
        scope.create_function(move |_, (x, y): (f32, f32)| {
            let (o, d) = view(camera, scene).screen_ray(Vec2::new(x, y), px());
            Ok((o.x, o.y, o.z, d.x, d.y, d.z))
        }),
    )
}
