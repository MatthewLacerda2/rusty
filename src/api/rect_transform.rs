//! src/api/rect_transform.rs — `RectTransform` namespace (#417).
//!
//! Get/Set over an entity's `RectTransformComponent`: anchors, pivot, anchored
//! position and size delta, each an `x, y` pair in the owning canvas's reference
//! units (y-up). Every setter routes through the shared
//! `scene::authoring::rect_transform` ops the inspector card uses, so validation
//! (anchors clamped to `[0, 1]` and kept ordered) lives once. The computed rect is
//! read with `UI.GetRect`; the Transform keeps rotation and scale.

use std::cell::RefCell;

use glam::Vec2;
use mlua::Lua;

use super::{put, Reg};
use crate::components::RectTransformComponent;
use crate::scene::authoring::rect_transform as rect_ops;
use crate::scene::Scene;

type Getter = fn(&RectTransformComponent) -> Vec2;
type Setter = fn(&mut RectTransformComponent, Vec2);

/// `(suffix, getter, setter)` for every `Get<X>` / `Set<X>` pair.
const PAIRS: [(&str, Getter, Setter); 5] = [
    ("AnchorMin", |r| r.anchor_min, rect_ops::set_anchor_min),
    ("AnchorMax", |r| r.anchor_max, rect_ops::set_anchor_max),
    ("Pivot", |r| r.pivot, rect_ops::set_pivot),
    (
        "AnchoredPosition",
        |r| r.anchored_position,
        rect_ops::set_anchored_position,
    ),
    ("SizeDelta", |r| r.size_delta, rect_ops::set_size_delta),
];

/// Register the `RectTransform` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in PAIRS {
        register_pair(scope, &table, scene, suffix, get, set)?;
    }
    lua.globals()
        .set("RectTransform", table)
        .map_err(|e| e.to_string())
}

/// `Get<suffix>(id) -> x, y` (`0, 0` without a RectTransform) and
/// `Set<suffix>(id, x, y)` (a no-op without one).
fn register_pair<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    suffix: &str,
    get: Getter,
    set: Setter,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let v = scene.world.rect_transform(id).map(|r| get(&r));
            let v = v.unwrap_or(Vec2::ZERO);
            Ok((v.x, v.y))
        }),
    )?;
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, x, y): (u32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut r) = scene.world.rect_transform_mut(id) {
                set(&mut r, Vec2::new(x, y));
            }
            Ok(())
        }),
    )
}
