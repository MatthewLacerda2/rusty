//! src/api/layout_group.rs — `LayoutGroup` namespace (#421).
//!
//! Get/Set over an entity's `LayoutGroupComponent` (Unity's Horizontal / Vertical
//! / Grid layout groups): the kind, padding, spacing, child alignment, the
//! row/column control and force-expand pairs, and the grid's cell size,
//! constraint, count, start corner and start axis. Every setter routes through
//! the shared `scene::authoring::layout_group` ops the inspector card uses.
//! Getters return a neutral default without a group; setters are then no-ops.
//! Enum values travel as their names (case-insensitive; unknown names ignored).
//! The placed rects are read with `UI.GetRect`.

use std::cell::RefCell;

use glam::{Vec2, Vec4};
use mlua::{FromLuaMulti, IntoLuaMulti, Lua, Table};

use super::{put, Reg};
use crate::components::LayoutGroupComponent;
use crate::scene::authoring::layout_group::{self as ops, CONSTRAINTS, CORNERS, KINDS};
use crate::scene::authoring::text::{alignment_name, parse_alignment};
use crate::scene::Scene;

type Cell<'s> = &'s RefCell<Scene>;
type G = LayoutGroupComponent;
type NameGet = fn(&G) -> &'static str;
type NameSet = fn(&mut G, &str);

/// `Get/SetKind`, `Get/SetChildAlignment`, `Get/SetConstraint`, `Get/SetStartCorner`.
const NAMES: [(&str, NameGet, NameSet); 4] = [
    (
        "Kind",
        |g| ops::name_of(&KINDS, g.kind),
        |g, n| {
            if let Some(v) = ops::parse(&KINDS, n) {
                ops::set_kind(g, v);
            }
        },
    ),
    (
        "ChildAlignment",
        |g| alignment_name(g.child_alignment),
        |g, n| {
            if let Some(v) = parse_alignment(n) {
                ops::set_child_alignment(g, v);
            }
        },
    ),
    (
        "Constraint",
        |g| ops::name_of(&CONSTRAINTS, g.constraint),
        |g, n| {
            if let Some(v) = ops::parse(&CONSTRAINTS, n) {
                ops::set_constraint(g, v);
            }
        },
    ),
    (
        "StartCorner",
        |g| ops::name_of(&CORNERS, g.start_corner),
        |g, n| {
            if let Some(v) = ops::parse(&CORNERS, n) {
                ops::set_start_corner(g, v);
            }
        },
    ),
];

/// Register the `LayoutGroup` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: Cell<'scope>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in NAMES {
        pair(
            scope,
            &t,
            scene,
            suffix,
            move |g| get(g),
            move |g, n: String| set(g, &n),
        )?;
    }
    register_numbers(scope, &t, scene)?;
    register_flags(scope, &t, scene)?;
    lua.globals()
        .set("LayoutGroup", t)
        .map_err(|e| e.to_string())
}

/// `Get/SetPadding`, `Get/SetSpacing`, `Get/SetCellSize`, `Get/SetConstraintCount`.
fn register_numbers<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: Cell<'scope>,
) -> Reg {
    type Lbrt = (f32, f32, f32, f32);
    let padding = |g: &G| (g.padding.x, g.padding.y, g.padding.z, g.padding.w);
    let set_padding = |g: &mut G, (l, b, r, t): Lbrt| ops::set_padding(g, Vec4::new(l, b, r, t));
    pair(scope, t, scene, "Padding", padding, set_padding)?;
    let spacing = |g: &G| (g.spacing.x, g.spacing.y);
    let set_spacing = |g: &mut G, (x, y): (f32, f32)| ops::set_spacing(g, Vec2::new(x, y));
    pair(scope, t, scene, "Spacing", spacing, set_spacing)?;
    let cell = |g: &G| (g.cell_size.x, g.cell_size.y);
    let set_cell = |g: &mut G, (x, y): (f32, f32)| ops::set_cell_size(g, Vec2::new(x, y));
    pair(scope, t, scene, "CellSize", cell, set_cell)?;
    let count = |g: &G| g.constraint_count;
    let set_count = |g: &mut G, n: u32| ops::set_constraint_count(g, n);
    pair(scope, t, scene, "ConstraintCount", count, set_count)
}

/// `Get/SetControlChildSize`, `Get/SetChildForceExpand` (width, height) and
/// `Get/SetStartVertical`.
fn register_flags<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: Cell<'scope>,
) -> Reg {
    let control = |g: &G| (g.control_child_width, g.control_child_height);
    let set_control = |g: &mut G, (w, h): (bool, bool)| ops::set_control_child_size(g, w, h);
    pair(scope, t, scene, "ControlChildSize", control, set_control)?;
    let expand = |g: &G| (g.child_force_expand_width, g.child_force_expand_height);
    let set_expand = |g: &mut G, (w, h): (bool, bool)| ops::set_child_force_expand(g, w, h);
    pair(scope, t, scene, "ChildForceExpand", expand, set_expand)?;
    let vertical = |g: &G| g.start_vertical;
    let set_vertical = |g: &mut G, v: bool| ops::set_start_vertical(g, v);
    pair(scope, t, scene, "StartVertical", vertical, set_vertical)
}

/// Register `Get<suffix>(id)` → `get(group)` (Lua's defaults without a group) and
/// `Set<suffix>(id, …)` → `set(group, …)` (a no-op without one).
fn pair<'lua, 'scope, R, A>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: Cell<'scope>,
    suffix: &str,
    get: impl Fn(&G) -> R + 'scope,
    set: impl Fn(&mut G, A) + 'scope,
) -> Reg
where
    R: for<'a> IntoLuaMulti<'a> + Default,
    A: for<'a> FromLuaMulti<'a>,
{
    let f = scope.create_function(move |_, id: u32| {
        Ok(scene
            .borrow()
            .world
            .layout_group(id)
            .map(|g| get(&g))
            .unwrap_or_default())
    });
    put(t, &format!("Get{suffix}"), f)?;
    let f = scope.create_function(move |lua, (id, rest): (u32, mlua::MultiValue)| {
        let value = A::from_lua_multi(rest, lua)?;
        if let Some(mut g) = scene.borrow_mut().world.layout_group_mut(id) {
            set(&mut g, value);
        }
        Ok(())
    });
    put(t, &format!("Set{suffix}"), f)
}
