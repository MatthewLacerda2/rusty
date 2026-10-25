//! src/api/shape.rs — `Shape` namespace (#425).
//!
//! Get/Set over an entity's `ShapeComponent`, the texture-free SDF graphic: its
//! kind and corner style (by name), the per-kind numbers (`Radius`, `InnerRadius`,
//! `Arc`, `Thickness`, `Dash`), the fill (`Color`, `Gradient`), `Border`, the
//! `Shadow` and `Glow` effects, `Blend` and `RaycastTarget`. Multi-value accessors
//! travel as flat number lists, colours last. Every setter routes through the
//! shared `scene::authoring::ui::shape` ops the inspector card uses. Getters return a
//! neutral default without a Shape; setters are then no-ops. Whether a shape exists
//! is `Scene.AddComponent(id, "Shape")` / `RemoveComponent`.

use std::cell::RefCell;

use glam::{Vec2, Vec4};
use mlua::{Lua, Table, Variadic};

use super::ui_look::{register_blend, register_gradient, BlendAccess, GradientAccess};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::ShapeComponent;
use crate::components::{ShapeCorner, ShapeKind};
use crate::scene::authoring::ui::shape as ops;
use crate::scene::Scene;

type Scoped<'s> = &'s RefCell<Scene>;
type NumsGet = fn(&ShapeComponent) -> Vec<f32>;
type NumsSet = fn(&mut ShapeComponent, &[f32]);

/// `(suffix, how many numbers, get, set)`: `Get<suffix>(id) -> …` / `Set<suffix>(id, …)`.
const NUMS: [(&str, usize, NumsGet, NumsSet); 9] = [
    (
        "Radius",
        4,
        |s| s.radius.to_array().to_vec(),
        |s, v| ops::set_radius(s, v4(v)),
    ),
    (
        "InnerRadius",
        1,
        |s| vec![s.inner_radius],
        |s, v| ops::set_inner_radius(s, v[0]),
    ),
    (
        "Arc",
        2,
        |s| vec![s.arc_start, s.arc_end],
        |s, v| ops::set_arc(s, v[0], v[1]),
    ),
    (
        "Thickness",
        1,
        |s| vec![s.thickness],
        |s, v| ops::set_thickness(s, v[0]),
    ),
    (
        "Dash",
        2,
        |s| vec![s.dash, s.gap],
        |s, v| ops::set_dash(s, v[0], v[1]),
    ),
    (
        "Color",
        4,
        |s| s.color.to_array().to_vec(),
        |s, v| ops::set_color(s, v4(v)),
    ),
    (
        "Border",
        5,
        |s| lead(&[s.border_width], s.border_color),
        |s, v| ops::set_border(s, v[0], v4(&v[1..])),
    ),
    (
        "Shadow",
        7,
        |s| {
            lead(
                &[s.shadow.offset.x, s.shadow.offset.y, s.shadow.blur],
                s.shadow.color,
            )
        },
        |s, v| ops::set_shadow(s, Vec2::new(v[0], v[1]), v[2], v4(&v[3..])),
    ),
    (
        "Glow",
        6,
        |s| lead(&[s.glow.size, s.glow.intensity], s.glow.color),
        |s, v| ops::set_glow(s, v[0], v[1], v4(&v[2..])),
    ),
];

/// The first four of `v` as a `Vec4`.
fn v4(v: &[f32]) -> Vec4 {
    Vec4::new(v[0], v[1], v[2], v[3])
}

/// `scalars` then `color`'s channels.
fn lead(scalars: &[f32], color: Vec4) -> Vec<f32> {
    scalars.iter().copied().chain(color.to_array()).collect()
}

/// Register the `Shape` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: Scoped<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, count, get, set) in NUMS {
        register_nums(scope, &table, scene, (suffix, count), get, set)?;
    }
    register_names(scope, &table, scene)?;
    register_blend(scope, &table, scene, ("Shape", BLEND))?;
    register_gradient(scope, &table, scene, ("Shape", GRADIENT))?;
    lua.globals().set("Shape", table).map_err(|e| e.to_string())
}

/// Run `f` on `id`'s Shape when it has one.
fn with_shape(scene: &mut Scene, id: u32, f: impl FnOnce(&mut ShapeComponent)) {
    if let Some(mut s) = scene.world.shape_mut(id) {
        f(&mut s);
    }
}

const BLEND: BlendAccess = BlendAccess {
    get: |s, id| s.world.shape(id).map(|c| c.blend),
    set: |s, id, b| with_shape(s, id, |c| ops::set_blend(c, b)),
};

const GRADIENT: GradientAccess = GradientAccess {
    get: |s, id| s.world.shape(id).map(|c| c.gradient.clone()),
    set: |s, id, g| with_shape(s, id, |c| ops::set_gradient(c, g)),
};

/// `Get<suffix>(id) -> …` (zeros without a Shape) / `Set<suffix>(id, …)`, which
/// needs all `count` numbers.
fn register_nums<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: Scoped<'scope>,
    (suffix, count): (&'static str, usize),
    get: NumsGet,
    set: NumsSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: EntityArg| {
            let s = scene.borrow();
            let v = s
                .world
                .shape(id.read())
                .map_or(vec![0.0; count], |c| get(&c));
            Ok(Variadic::from_iter(v))
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, args): (EntityArg, Variadic<f32>)| {
            let id = id.live("Shape", &verb)?;
            if args.len() < count {
                let msg = format!("Shape.Set{suffix} expects {count} numbers");
                return Err(mlua::Error::RuntimeError(msg));
            }
            with_shape(&mut scene.borrow_mut(), id, |c| set(c, &args));
            Ok(())
        }),
    )
}

type NameGet = fn(&ShapeComponent) -> &'static str;
type NameSet = fn(&mut ShapeComponent, &str);

/// `Get/SetKind` and `Get/SetCorner`, by name (case-insensitive; unknown ignored).
const NAMES: [(&str, NameGet, NameSet); 2] = [
    (
        "Kind",
        |c| c.kind.name(),
        |c, n| {
            if let Some(k) = ShapeKind::parse(n) {
                ops::set_kind(c, k);
            }
        },
    ),
    (
        "Corner",
        |c| c.corner.name(),
        |c, n| {
            if let Some(k) = ShapeCorner::parse(n) {
                ops::set_corner(c, k);
            }
        },
    ),
];

/// The [`NAMES`] accessors (`"None"` without a Shape) and `Get/SetRaycastTarget`.
fn register_names<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    scene: Scoped<'scope>,
) -> Reg {
    for (suffix, get, set) in NAMES {
        let read = move |_: &Lua, id: EntityArg| {
            let s = scene.borrow();
            Ok(s.world
                .shape(id.read())
                .map_or("None", |c| get(&c))
                .to_string())
        };
        put(table, &format!("Get{suffix}"), scope.create_function(read))?;
        let verb = format!("Set{suffix}");
        let write = move |_: &Lua, (id, name): (EntityArg, String)| {
            let id = id.live("Shape", &verb)?;
            with_shape(&mut scene.borrow_mut(), id, |c| set(c, &name));
            Ok(())
        };
        put(table, &format!("Set{suffix}"), scope.create_function(write))?;
    }
    put(
        table,
        "GetRaycastTarget",
        scope.create_function(move |_, id: EntityArg| {
            Ok(scene
                .borrow()
                .world
                .shape(id.read())
                .is_some_and(|c| c.raycast_target))
        }),
    )?;
    put(
        table,
        "SetRaycastTarget",
        scope.create_function(move |_, (id, on): (EntityArg, bool)| {
            let id = id.live("Shape", "SetRaycastTarget")?;
            with_shape(&mut scene.borrow_mut(), id, |c| {
                ops::set_raycast_target(c, on)
            });
            Ok(())
        }),
    )
}
