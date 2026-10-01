//! src/api/image.rs — `Image` namespace (#418).
//!
//! Get/Set over an entity's `ImageComponent`: tint colour, texture, image type,
//! 9-slice border, the fill (method, origin, amount, direction), preserve-aspect,
//! raycast target, and the look (#425): blend mode and gradient tint. Every setter
//! routes through the shared `scene::authoring::image` ops the inspector card
//! uses, so validation lives once.
//! Getters return a neutral default when the entity has no Image; setters are then
//! no-ops. Enum values travel as their names (case-insensitive; unknown ignored).

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;

use super::ui_look::{BlendAccess, GradientAccess};
use super::{put, Reg};
use crate::components::ImageComponent;
use crate::scene::authoring::image as image_ops;
use crate::scene::Scene;

type Vec4Get = fn(&ImageComponent) -> Vec4;
type Vec4Set = fn(&mut ImageComponent, Vec4);
type NameGet = fn(&ImageComponent) -> &'static str;
type NameSet = fn(&mut ImageComponent, &str);
type BoolGet = fn(&ImageComponent) -> bool;
type BoolSet = fn(&mut ImageComponent, bool);

/// `Get/SetColor` (`r, g, b, a`) and `Get/SetBorder` (`left, bottom, right, top`).
const VEC4S: [(&str, Vec4Get, Vec4Set); 2] = [
    ("Color", |i| i.color, image_ops::set_color),
    ("Border", |i| i.border, image_ops::set_border),
];

/// `Get/SetType`, `Get/SetFillMethod`, `Get/SetFillOrigin` — by name.
const NAMES: [(&str, NameGet, NameSet); 3] = [
    (
        "Type",
        |i| image_ops::image_type_name(i.image_type),
        |i, n| {
            if let Some(t) = image_ops::parse_image_type(n) {
                image_ops::set_image_type(i, t);
            }
        },
    ),
    (
        "FillMethod",
        |i| image_ops::fill_method_name(i.fill_method),
        |i, n| {
            if let Some(m) = image_ops::parse_fill_method(n) {
                image_ops::set_fill_method(i, m);
            }
        },
    ),
    (
        "FillOrigin",
        |i| image_ops::fill_origin_name(i.fill_origin),
        |i, n| {
            if let Some(o) = image_ops::parse_fill_origin(n) {
                image_ops::set_fill_origin(i, o);
            }
        },
    ),
];

/// `Get/SetFillClockwise`, `Get/SetPreserveAspect`, `Get/SetRaycastTarget`.
const BOOLS: [(&str, BoolGet, BoolSet); 3] = [
    (
        "FillClockwise",
        |i| i.fill_clockwise,
        image_ops::set_fill_clockwise,
    ),
    (
        "PreserveAspect",
        |i| i.preserve_aspect,
        image_ops::set_preserve_aspect,
    ),
    (
        "RaycastTarget",
        |i| i.raycast_target,
        image_ops::set_raycast_target,
    ),
];

/// `Get/SetBlend` (#425).
const BLEND: BlendAccess = BlendAccess {
    get: |s, id| s.world.image(id).map(|i| i.blend),
    set: |s, id, b| {
        if let Some(mut i) = s.world.image_mut(id) {
            image_ops::set_blend(&mut i, b);
        }
    },
};

/// `Get/SetGradient` (#425): the tint's gradient.
const GRADIENT: GradientAccess = GradientAccess {
    get: |s, id| s.world.image(id).map(|i| i.gradient.clone()),
    set: |s, id, g| {
        if let Some(mut i) = s.world.image_mut(id) {
            image_ops::set_gradient(&mut i, g);
        }
    },
};

/// Register the `Image` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in VEC4S {
        register_vec4(scope, &table, scene, suffix, get, set)?;
    }
    for (suffix, get, set) in NAMES {
        register_name(scope, &table, scene, suffix, get, set)?;
    }
    for (suffix, get, set) in BOOLS {
        register_bool(scope, &table, scene, suffix, get, set)?;
    }
    register_texture_and_amount(scope, &table, scene)?;
    super::ui_look::register_blend(scope, &table, scene, BLEND)?;
    super::ui_look::register_gradient(scope, &table, scene, GRADIENT)?;
    lua.globals().set("Image", table).map_err(|e| e.to_string())
}

/// Run `f` on `id`'s Image when it has one.
fn with_image(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut ImageComponent)) {
    if let Some(mut i) = scene.borrow_mut().world.image_mut(id) {
        f(&mut i);
    }
}

/// `Get<suffix>(id) -> x, y, z, w` (zeros without an Image) / `Set<suffix>(id, x, y, z, w)`.
fn register_vec4<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    suffix: &str,
    get: Vec4Get,
    set: Vec4Set,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let v = scene.world.image(id).map(|i| get(&i)).unwrap_or(Vec4::ZERO);
            Ok((v.x, v.y, v.z, v.w))
        }),
    )?;
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, x, y, z, w): (u32, f32, f32, f32, f32)| {
            with_image(scene, id, |i| set(i, Vec4::new(x, y, z, w)));
            Ok(())
        }),
    )
}

/// `Get<suffix>(id) -> name` (`"None"` without an Image) / `Set<suffix>(id, name)`.
fn register_name<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    suffix: &str,
    get: NameGet,
    set: NameSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            Ok(scene
                .world
                .image(id)
                .map_or("None", |i| get(&i))
                .to_string())
        }),
    )?;
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, name): (u32, String)| {
            with_image(scene, id, |i| set(i, &name));
            Ok(())
        }),
    )
}

/// `Get<suffix>(id) -> bool` (`false` without an Image) / `Set<suffix>(id, bool)`.
fn register_bool<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    suffix: &str,
    get: BoolGet,
    set: BoolSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.image(id).is_some_and(|i| get(&i)))
        }),
    )?;
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, value): (u32, bool)| {
            with_image(scene, id, |i| set(i, value));
            Ok(())
        }),
    )
}

/// `Get/SetTexture` (a path, or `nil` for a solid colour) and `Get/SetFillAmount`.
fn register_texture_and_amount<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetTexture",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.image(id).and_then(|i| i.texture.clone()))
        }),
    )?;
    put(
        table,
        "SetTexture",
        scope.create_function(|_, (id, path): (u32, Option<String>)| {
            with_image(scene, id, |i| image_ops::set_texture(i, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFillAmount",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.image(id).map_or(0.0, |i| i.fill_amount))
        }),
    )?;
    put(
        table,
        "SetFillAmount",
        scope.create_function(|_, (id, amount): (u32, f32)| {
            with_image(scene, id, |i| image_ops::set_fill_amount(i, amount));
            Ok(())
        }),
    )
}
