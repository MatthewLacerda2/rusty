//! src/api/text.rs — `Text` namespace (#419).
//!
//! Get/Set over an entity's `TextComponent` — the string, fonts, size, colour,
//! alignment, wrapping, overflow, spacing, auto-size, rich text, raycast target
//! and the SDF effects (outline, shadow, glow) — plus two reads of the CPU layout:
//! `GetPreferredSize` (what the text wants, for layout) and `GetLayout` (what it
//! drew: size, lines, the auto-sized font size, whether it was cut). Every setter
//! routes through the shared `scene::authoring::text` ops the inspector card uses.
//! Getters return a neutral default when the entity has no Text; setters are then
//! no-ops. Enum values travel as their names (case-insensitive; unknown ignored).

use std::cell::RefCell;

use glam::{Vec2, Vec4};
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::TextComponent;
use crate::core::video::VideoSettings;
use crate::scene::authoring::text::{self as ops, FontSlot};
use crate::scene::Scene;
use crate::ui::{layout, text as ui_text, ScreenSize};

type Scoped<'s> = &'s RefCell<Scene>;
type F32Get = fn(&TextComponent) -> f32;
type F32Set = fn(&mut TextComponent, f32);
type BoolGet = fn(&TextComponent) -> bool;
type BoolSet = fn(&mut TextComponent, bool);
type StrGet = fn(&TextComponent) -> Option<String>;
type StrSet = fn(&mut TextComponent, Option<String>);
/// An effect as a flat `[lead0, lead1, r, g, b, a]`; see `text_effects`.
pub(super) type EffectGet = fn(&TextComponent) -> [f32; 6];
pub(super) type EffectSet = fn(&mut TextComponent, [f32; 6]);
/// A `(suffix, leading scalars, get, set)` effect row.
pub(super) type Effect = (&'static str, usize, EffectGet, EffectSet);

/// `Get/SetFontSize`, `Get/SetLineSpacing`, `Get/SetLetterSpacing`.
const F32S: [(&str, F32Get, F32Set); 3] = [
    ("FontSize", |t| t.font_size, ops::set_font_size),
    ("LineSpacing", |t| t.line_spacing, ops::set_line_spacing),
    (
        "LetterSpacing",
        |t| t.letter_spacing,
        ops::set_letter_spacing,
    ),
];

/// `Get/SetWrap`, `Get/SetRichText`, `Get/SetRaycastTarget`.
const BOOLS: [(&str, BoolGet, BoolSet); 3] = [
    ("Wrap", |t| t.wrap, ops::set_wrap),
    ("RichText", |t| t.rich_text, ops::set_rich_text),
    (
        "RaycastTarget",
        |t| t.raycast_target,
        ops::set_raycast_target,
    ),
];

/// `Get/SetText`, `Get/SetFont`, `Get/SetBoldFont`, `Get/SetItalicFont`,
/// `Get/SetAlignment`, `Get/SetOverflow` — strings (a font `nil` clears it).
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
        |t| Some(ops::alignment_name(t.alignment).to_string()),
        |t, n| {
            if let Some(a) = n.as_deref().and_then(ops::parse_alignment) {
                ops::set_alignment(t, a);
            }
        },
    ),
    (
        "Overflow",
        |t| Some(ops::overflow_name(t.overflow).to_string()),
        |t, n| {
            if let Some(o) = n.as_deref().and_then(ops::parse_overflow) {
                ops::set_overflow(t, o);
            }
        },
    ),
];

/// `Get/SetOutline` (`width, r, g, b, a`), `Get/SetGlow` (`size, r, g, b, a`),
/// `Get/SetShadow` (`dx, dy, r, g, b, a`) and `Get/SetColor` (`r, g, b, a`).
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

/// Register the `Text` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: Scoped<'scope>,
    screen: &'scope RefCell<ScreenSize>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    for (suffix, get, set) in F32S {
        getter(scope, &t, scene, &format!("Get{suffix}"), move |c| {
            Ok(get(c))
        })?;
        setter(
            scope,
            &t,
            scene,
            &format!("Set{suffix}"),
            move |c, v: f32| set(c, v),
        )?;
    }
    for (suffix, get, set) in BOOLS {
        getter(scope, &t, scene, &format!("Get{suffix}"), move |c| {
            Ok(get(c))
        })?;
        setter(
            scope,
            &t,
            scene,
            &format!("Set{suffix}"),
            move |c, v: bool| set(c, v),
        )?;
    }
    for (suffix, get, set) in STRS {
        getter(scope, &t, scene, &format!("Get{suffix}"), move |c| {
            Ok(get(c))
        })?;
        setter(
            scope,
            &t,
            scene,
            &format!("Set{suffix}"),
            move |c, v: Option<String>| set(c, v),
        )?;
    }
    super::text_effects::register(scope, &t, scene, &EFFECTS)?;
    super::text_effects::register_auto_size(scope, &t, scene)?;
    register_layout_reads(scope, &t, scene, screen, video)?;
    lua.globals().set("Text", t).map_err(|e| e.to_string())
}

/// `name(id)` → `get(text)`, or Lua's default (`nil` / `false` / `0`) without a Text.
fn getter<'lua, 'scope, R>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: Scoped<'scope>,
    name: &str,
    get: impl Fn(&TextComponent) -> mlua::Result<R> + 'scope,
) -> Reg
where
    R: for<'a> mlua::IntoLua<'a> + Default,
{
    put(
        table,
        name,
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            scene.world.text(id).map_or(Ok(R::default()), |t| get(&t))
        }),
    )
}

/// `name(id, value)` → `set(text, value)` when the entity has a Text.
fn setter<'lua, 'scope, V>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: Scoped<'scope>,
    name: &str,
    set: impl Fn(&mut TextComponent, V) + 'scope,
) -> Reg
where
    V: for<'a> mlua::FromLua<'a>,
{
    put(
        table,
        name,
        scope.create_function(move |_, (id, value): (u32, V)| {
            if let Some(mut t) = scene.borrow_mut().world.text_mut(id) {
                set(&mut t, value);
            }
            Ok(())
        }),
    )
}

/// `GetPreferredSize(id) -> w, h` (wrapped at the element's current rect width) and
/// `GetLayout(id) -> { width, height, lines, font_size, truncated }` (what draws).
fn register_layout_reads<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: Scoped<'scope>,
    screen: &'scope RefCell<ScreenSize>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let rect_size = move |id: u32| {
        let px = screen.borrow().pixels(&video.borrow());
        layout::rect_of(&scene.borrow().world, id, px).map(|r| r.rect.1)
    };
    put(
        table,
        "GetPreferredSize",
        scope.create_function(move |_, id: u32| {
            let width = rect_size(id).map_or(f32::INFINITY, |s| s.x);
            let text = scene.borrow().world.text(id).map(|t| t.clone());
            let size = text.map_or(Vec2::ZERO, |t| ui_text::preferred_size(&t, width));
            Ok((size.x, size.y))
        }),
    )?;
    put(
        table,
        "GetLayout",
        scope.create_function(move |lua, id: u32| {
            let text = scene.borrow().world.text(id).map(|t| t.clone());
            let (Some(text), Some(rect)) = (text, rect_size(id)) else {
                return Ok(None);
            };
            let l = ui_text::layout_text(&text, rect);
            let t = lua.create_table()?;
            t.set("width", l.size.x)?;
            t.set("height", l.size.y)?;
            t.set("lines", l.lines)?;
            t.set("font_size", l.font_size)?;
            t.set("truncated", l.truncated)?;
            Ok(Some(t))
        }),
    )
}
