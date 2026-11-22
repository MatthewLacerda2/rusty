use glam::{Vec2, Vec4};
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::TextComponent;
use rusty_components::{TextAlignment, TextOverflow};
use rusty_core::settings::video::VideoSettings;
use rusty_edit::authoring::ui::text::{self as ops, FontSlot};
use rusty_scene::Scene;
use rusty_ui::ScreenSize;

type F32Get = fn(&TextComponent) -> f32;
type F32Set = fn(&mut TextComponent, f32);
type BoolGet = fn(&TextComponent) -> bool;
type BoolSet = fn(&mut TextComponent, bool);
type StrGet = fn(&TextComponent) -> Option<String>;
type StrSet = fn(&mut TextComponent, Option<String>);
pub(super) type EffectGet = fn(&TextComponent) -> [f32; 6];
pub(super) type EffectSet = fn(&mut TextComponent, [f32; 6]);
pub(super) type Effect = (&'static str, usize, EffectGet, EffectSet);

const F32S: [(&str, F32Get, F32Set); 3] = [
    ("FontSize", |t| t.font_size, ops::set_font_size),
    ("LineSpacing", |t| t.line_spacing, ops::set_line_spacing),
    (
        "LetterSpacing",
        |t| t.letter_spacing,
        ops::set_letter_spacing,
    ),
];

const BOOLS: [(&str, BoolGet, BoolSet); 3] = [
    ("Wrap", |t| t.wrap, ops::set_wrap),
    ("RichText", |t| t.rich_text, ops::set_rich_text),
    (
        "RaycastTarget",
        |t| t.raycast_target,
        ops::set_raycast_target,
    ),
];

const STRS: [(&str, StrGet, StrSet); 6] = [
    (
        "Text",
        |t| Some(t.text.clone()),
        |t, s| ops::set_text(t, s.unwrap_or_default()),
    ),
    (
        "Font",
        |t| t.font.clone(),
        |t, p| ops::set_font(t, FontSlot::Regular, p),
    ),
    (
        "BoldFont",
        |t| t.font_bold.clone(),
        |t, p| ops::set_font(t, FontSlot::Bold, p),
    ),
    (
        "ItalicFont",
        |t| t.font_italic.clone(),
        |t, p| ops::set_font(t, FontSlot::Italic, p),
    ),
    (
        "Alignment",
        |t| Some(t.alignment.name().to_string()),
        |t, n| {
            if let Some(a) = n.as_deref().and_then(TextAlignment::parse) {
                ops::set_alignment(t, a);
            }
        },
    ),
    (
        "Overflow",
        |t| Some(t.overflow.name().to_string()),
        |t, n| {
            if let Some(o) = n.as_deref().and_then(TextOverflow::parse) {
                ops::set_overflow(t, o);
            }
        },
    ),
];

const EFFECTS: [Effect; 4] = [
    (
        "Color",
        0,
        |t| rgba(0.0, 0.0, t.color),
        |t, v| ops::set_color(t, color(v)),
    ),
    (
        "Outline",
        1,
        |t| rgba(t.outline_width, 0.0, t.outline_color),
        |t, v| ops::set_outline(t, v[0], color(v)),
    ),
    (
        "Glow",
        1,
        |t| rgba(t.glow_size, 0.0, t.glow_color),
        |t, v| ops::set_glow(t, v[0], color(v)),
    ),
    (
        "Shadow",
        2,
        |t| {
            [
                t.shadow_offset.x,
                t.shadow_offset.y,
                t.shadow_color.x,
                t.shadow_color.y,
                t.shadow_color.z,
                t.shadow_color.w,
            ]
        },
        |t, v| ops::set_shadow(t, Vec2::new(v[0], v[1]), Vec4::new(v[2], v[3], v[4], v[5])),
    ),
];

fn rgba(a: f32, b: f32, c: Vec4) -> [f32; 6] {
    [a, b, c.x, c.y, c.z, c.w]
}

fn color(v: [f32; 6]) -> Vec4 {
    Vec4::new(v[2], v[3], v[4], v[5])
}

pub fn register(
    lua: &Lua,
    scene: ApiCell<Scene>,
    screen: ApiCell<ScreenSize>,
    video: ApiCell<VideoSettings>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in F32S {
        getter(lua, &t, scene, &format!("Get{suffix}"), move |c| Ok(get(c)))?;
        setter(lua, &t, scene, &format!("Set{suffix}"), move |c, v: f32| {
            set(c, v)
        })?;
    }
    for (suffix, get, set) in BOOLS {
        getter(lua, &t, scene, &format!("Get{suffix}"), move |c| Ok(get(c)))?;
        setter(
            lua,
            &t,
            scene,
            &format!("Set{suffix}"),
            move |c, v: bool| set(c, v),
        )?;
    }
    for (suffix, get, set) in STRS {
        getter(lua, &t, scene, &format!("Get{suffix}"), move |c| Ok(get(c)))?;
        setter(
            lua,
            &t,
            scene,
            &format!("Set{suffix}"),
            move |c, v: Option<String>| set(c, v),
        )?;
    }
    super::text_effects::register(lua, &t, scene, &EFFECTS)?;
    super::text_effects::register_auto_size(lua, &t, scene)?;
    super::text_layout::register(lua, &t, scene, screen, video)?;
    let blend = super::ui_look::BlendAccess {
        get: |s, id| s.world.text(id).map(|t| t.blend),
        set: |s, id, b| {
            if let Some(mut t) = s.world.text_mut(id) {
                ops::set_blend(&mut t, b);
            }
        },
    };
    super::ui_look::register_blend(lua, &t, scene, ("Text", blend))?;
    lua.globals().set("Text", t).map_err(|e| e.to_string())
}

fn getter<R>(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    get: impl Fn(&TextComponent) -> mlua::Result<R> + 'static,
) -> Reg
where
    R: mlua::IntoLua + Default,
{
    put(
        table,
        name,
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            scene
                .world
                .text(id.read())
                .map_or(Ok(R::default()), |t| get(&t))
        }),
    )
}

fn setter<V>(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    name: &str,
    set: impl Fn(&mut TextComponent, V) + 'static,
) -> Reg
where
    V: mlua::FromLua,
{
    let verb = name.to_string();
    put(
        table,
        name,
        lua.create_function(move |lua, (id, value): (EntityArg, V)| {
            let id = id.live("Text", &verb)?;
            if let Some(mut t) = scene.get(lua)?.borrow_mut().world.text_mut(id) {
                set(&mut t, value);
            }
            Ok(())
        }),
    )
}
