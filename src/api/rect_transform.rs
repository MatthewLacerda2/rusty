//! src/api/rect_transform.rs — `RectTransform` namespace (#417).
//!
//! Get/Set over an entity's `RectTransformComponent`: anchors, pivot, anchored
//! position and size delta, each an `x, y` pair in the owning canvas's reference
//! units (y-up). Every setter routes through the shared
//! `scene::authoring::ui::layout::rect_transform` ops the inspector card uses, so validation
//! (anchors clamped to `[0, 1]` and kept ordered) lives once. The computed rect is
//! read with `UI.GetRect`; the Transform keeps rotation and scale. The world
//! anchor verbs (#429) make an element a marker pinned to a world point.
//! `SetAnchorPreset` (#423) is the inspector's anchor-preset grid: it re-anchors
//! against the parent's rect on the sim's screen, keeping the element in place.

use glam::{Vec2, Vec3};
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::RectTransformComponent;
use crate::core::settings::video::VideoSettings;
use crate::scene::authoring::ui::layout::rect_transform::{
    self as rect_ops, AnchorPreset, AxisPreset,
};
use crate::scene::Scene;
use crate::ui::layout::rect_in;
use crate::ui::ScreenSize;

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
pub fn register(
    lua: &Lua,
    scene: ApiCell<Scene>,
    screen: (ApiCell<ScreenSize>, ApiCell<VideoSettings>),
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in PAIRS {
        register_pair(lua, &table, scene, suffix, get, set)?;
    }
    register_world_anchor(lua, &table, scene)?;
    register_preset(lua, &table, scene, screen)?;
    lua.globals()
        .set("RectTransform", table)
        .map_err(|e| e.to_string())
}

/// `Get<suffix>(id) -> x, y` (`0, 0` without a RectTransform) and
/// `Set<suffix>(id, x, y)` (a no-op without one).
fn register_pair(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    suffix: &str,
    get: Getter,
    set: Setter,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let v = scene.world.rect_transform(id.read()).map(|r| get(&r));
            let v = v.unwrap_or(Vec2::ZERO);
            Ok((v.x, v.y))
        }),
    )?;
    let verb = format!("Set{suffix}");
    put(
        table,
        &format!("Set{suffix}"),
        lua.create_function(move |lua, (id, x, y): (EntityArg, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("RectTransform", &verb)?;
            let mut scene = scene.borrow_mut();
            if let Some(mut r) = scene.world.rect_transform_mut(id) {
                set(&mut r, Vec2::new(x, y));
            }
            Ok(())
        }),
    )
}

/// `SetAnchorPreset(id, x, y, setPivot?, setPosition?)`: `x` is `left` / `center`
/// / `right` / `stretch`, `y` is `bottom` / `middle` / `top` / `stretch`. A no-op
/// without a RectTransform; an unknown name is an error.
fn register_preset(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    (screen, video): (ApiCell<ScreenSize>, ApiCell<VideoSettings>),
) -> Reg {
    type Args = (EntityArg, String, String, Option<bool>, Option<bool>);
    let f = lua.create_function(move |lua, (id, x, y, pivot, position): Args| {
        let scene = &*scene.get(lua)?;
        let id = id.live("RectTransform", "SetAnchorPreset")?;
        let axis = |name: &str, axis| {
            AxisPreset::from_name(name, axis).ok_or_else(|| {
                let want = AxisPreset::ALL.map(|p| p.name(axis)).join(", ");
                mlua::Error::runtime(format!("unknown anchor preset '{name}' (want {want})"))
            })
        };
        let preset = AnchorPreset {
            x: axis(&x, 0)?,
            y: axis(&y, 1)?,
            set_pivot: pivot.unwrap_or(false),
            set_position: position.unwrap_or(false),
        };
        let view = screen
            .get(lua)?
            .borrow()
            .view(&video.get(lua)?.borrow(), None);
        let mut scene = scene.borrow_mut();
        let parent = scene.world.parent_id(id);
        let parent = parent.and_then(|p| rect_in(&scene.world, p, &view));
        let size = parent.map_or(Vec2::ZERO, |p| p.rect.1);
        if let Some(mut r) = scene.world.rect_transform_mut(id) {
            rect_ops::apply_anchor_preset(&mut r, preset, size);
        }
        Ok(())
    });
    put(table, "SetAnchorPreset", f)
}

/// A one-flag marker option setter (a no-op on a non-marker).
type AnchorFlag = fn(&mut RectTransformComponent, bool);

/// The world anchor: `SetWorldAnchor(id, target|nil, ox, oy, oz)`,
/// `ClearWorldAnchor(id)`, `GetWorldAnchor(id)` (a table, or `nil` for a
/// non-marker), and the options `SetWorldAnchorClamp(id, clamp, padding)`,
/// `SetWorldAnchorRotate(id, on)`, `SetWorldAnchorHideWhenBehind(id, on)`.
fn register_world_anchor(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let edit = move |lua: &mlua::Lua, id: u32, f: &dyn Fn(&mut RectTransformComponent)| {
        if let Some(mut r) = scene.get(lua)?.borrow_mut().world.rect_transform_mut(id) {
            f(&mut r);
        }
        mlua::Result::Ok(())
    };
    type Args = (EntityArg, EntityArg, Option<f32>, Option<f32>, Option<f32>);
    let f = lua.create_function(move |lua, (id, target, x, y, z): Args| {
        let (id, target) = (id.live("RectTransform", "SetWorldAnchor")?, target.get());
        let offset = Vec3::new(x.unwrap_or(0.0), y.unwrap_or(0.0), z.unwrap_or(0.0));
        edit(lua, id, &|r| rect_ops::set_world_anchor(r, target, offset))?;
        Ok(())
    });
    put(table, "SetWorldAnchor", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let id = id.live("RectTransform", "ClearWorldAnchor")?;
        edit(lua, id, &rect_ops::clear_world_anchor)?;
        Ok(())
    });
    put(table, "ClearWorldAnchor", f)?;
    let f = lua.create_function(
        move |lua, (id, clamp, pad): (EntityArg, bool, Option<f32>)| {
            let id = id.live("RectTransform", "SetWorldAnchorClamp")?;
            edit(lua, id, &|r| {
                rect_ops::set_anchor_clamp(r, clamp, pad.unwrap_or(0.0))
            })?;
            Ok(())
        },
    );
    put(table, "SetWorldAnchorClamp", f)?;
    let flags: [(&str, AnchorFlag); 2] = [
        ("SetWorldAnchorRotate", rect_ops::set_anchor_rotate),
        (
            "SetWorldAnchorHideWhenBehind",
            rect_ops::set_anchor_hide_when_behind,
        ),
    ];
    for (name, set) in flags {
        let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let id = id.live("RectTransform", name)?;
            edit(lua, id, &|r| set(r, on))?;
            Ok(())
        });
        put(table, name, f)?;
    }
    register_get_world_anchor(lua, table, scene)
}

/// `GetWorldAnchor(id)`: the marker's anchor as a table, or `nil` for a non-marker.
fn register_get_world_anchor(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let scene = scene.borrow();
        let anchor = scene
            .world
            .rect_transform(id.read())
            .and_then(|r| r.world_anchor.clone());
        let Some(a) = anchor else {
            return Ok(None);
        };
        let t = lua.create_table()?;
        t.set("target", a.target)?;
        t.set("target_lost", a.target_lost)?;
        let offset = lua.create_table()?;
        offset.set("x", a.offset.x)?;
        offset.set("y", a.offset.y)?;
        offset.set("z", a.offset.z)?;
        t.set("offset", offset)?;
        t.set("clamp_to_screen_edge", a.clamp_to_screen_edge)?;
        t.set("edge_padding", a.edge_padding)?;
        t.set("rotate_toward_target", a.rotate_toward_target)?;
        t.set("hide_when_behind", a.hide_when_behind)?;
        Ok(Some(t))
    });
    put(table, "GetWorldAnchor", f)
}
