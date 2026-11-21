use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;

use super::ui_look::{BlendAccess, GradientAccess};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_asset::MapRef;
use rusty_components::ImageComponent;
use rusty_components::{FillMethod, FillOrigin, ImageType};
use rusty_edit::authoring::ui::image as image_ops;
use rusty_scene::Scene;

type Vec4Get = fn(&ImageComponent) -> Vec4;
type Vec4Set = fn(&mut ImageComponent, Vec4);
type NameGet = fn(&ImageComponent) -> &'static str;
type NameSet = fn(&mut ImageComponent, &str);
type BoolGet = fn(&ImageComponent) -> bool;
type BoolSet = fn(&mut ImageComponent, bool);

const VEC4S: [(&str, Vec4Get, Vec4Set); 2] = [
    ("Color", |i| i.color, image_ops::set_color),
    ("Border", |i| i.border, image_ops::set_border),
];

const NAMES: [(&str, NameGet, NameSet); 3] = [
    (
        "Type",
        |i| i.image_type.name(),
        |i, n| {
            if let Some(t) = ImageType::parse(n) {
                image_ops::set_image_type(i, t);
            }
        },
    ),
    (
        "FillMethod",
        |i| i.fill_method.name(),
        |i, n| {
            if let Some(m) = FillMethod::parse(n) {
                image_ops::set_fill_method(i, m);
            }
        },
    ),
    (
        "FillOrigin",
        |i| i.fill_origin.name(),
        |i, n| {
            if let Some(o) = FillOrigin::parse(n) {
                image_ops::set_fill_origin(i, o);
            }
        },
    ),
];

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

const BLEND: BlendAccess = BlendAccess {
    get: |s, id| s.world.image(id).map(|i| i.blend),
    set: |s, id, b| {
        if let Some(mut i) = s.world.image_mut(id) {
            image_ops::set_blend(&mut i, b);
        }
    },
};

const GRADIENT: GradientAccess = GradientAccess {
    get: |s, id| s.world.image(id).map(|i| i.gradient.clone()),
    set: |s, id, g| {
        if let Some(mut i) = s.world.image_mut(id) {
            image_ops::set_gradient(&mut i, g);
        }
    },
};

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in VEC4S {
        register_vec4(lua, &table, scene, suffix, get, set)?;
    }
    for (suffix, get, set) in NAMES {
        register_name(lua, &table, scene, suffix, get, set)?;
    }
    for (suffix, get, set) in BOOLS {
        register_bool(lua, &table, scene, suffix, get, set)?;
    }
    register_texture_and_amount(lua, &table, scene)?;
    super::ui_look::register_blend(lua, &table, scene, ("Image", BLEND))?;
    super::ui_look::register_gradient(lua, &table, scene, ("Image", GRADIENT))?;
    lua.globals().set("Image", table).map_err(|e| e.to_string())
}

fn with_image(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut ImageComponent)) {
    if let Some(mut i) = scene.borrow_mut().world.image_mut(id) {
        f(&mut i);
    }
}

fn register_vec4(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: Vec4Get,
    set: Vec4Set,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let v = scene
                .world
                .image(id.read())
                .map(|i| get(&i))
                .unwrap_or(Vec4::ZERO);
            Ok((v.x, v.y, v.z, v.w))
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        lua.create_function(
            move |lua, (id, x, y, z, w): (EntityArg, f32, f32, f32, f32)| {
                let id = id.live("Image", &verb)?;
                with_image(&*scene.get(lua)?, id, |i| set(i, Vec4::new(x, y, z, w)));
                Ok(())
            },
        ),
    )
}

fn register_name(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: NameGet,
    set: NameSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .image(id.read())
                .map_or("None", |i| get(&i))
                .to_string())
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Image", &verb)?;
            with_image(&*scene.get(lua)?, id, |i| set(i, &name));
            Ok(())
        }),
    )
}

fn register_bool(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: BoolGet,
    set: BoolSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.image(id.read()).is_some_and(|i| get(&i)))
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        lua.create_function(move |lua, (id, value): (EntityArg, bool)| {
            let id = id.live("Image", &verb)?;
            with_image(&*scene.get(lua)?, id, |i| set(i, value));
            Ok(())
        }),
    )
}

fn register_texture_and_amount(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetTexture",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .image(id.read())
                .and_then(|i| i.texture.as_ref().map(MapRef::to_string)))
        }),
    )?;
    put(
        table,
        "SetTexture",
        lua.create_function(move |lua, (id, path): (EntityArg, Option<String>)| {
            let id = id.live("Image", "SetTexture")?;
            with_image(&*scene.get(lua)?, id, |i| image_ops::set_texture(i, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFillAmount",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.image(id.read()).map_or(0.0, |i| i.fill_amount))
        }),
    )?;
    put(
        table,
        "SetFillAmount",
        lua.create_function(move |lua, (id, amount): (EntityArg, f32)| {
            let id = id.live("Image", "SetFillAmount")?;
            with_image(&*scene.get(lua)?, id, |i| {
                image_ops::set_fill_amount(i, amount)
            });
            Ok(())
        }),
    )
}
