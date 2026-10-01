//! src/api/ribbon_style.rs — the style verbs `Trail` and `Line` share (#441).
//!
//! Both components carry one `RibbonStyle`, so their namespaces register the
//! same width / colour / texture / blend verbs from here, each routed through the
//! shared `scene::authoring::ribbon` ops the inspector cards call. Without the
//! component, getters return a neutral default and setters are no-ops.

use std::cell::RefCell;

use mlua::Table;

use super::{put, Reg};
use crate::components::{ParticleBlend, RibbonStyle, TextureMode};
use crate::core::curve::{Curve, Gradient};
use crate::scene::authoring::ribbon as ops;
use crate::scene::Scene;

type Rgba = (f32, f32, f32, f32);
/// `(id, r, g, b, a?)`.
type IdColor = (u32, f32, f32, f32, Option<f32>);
/// `(id, start rgba, end rgba)` flattened.
type IdColors = (u32, f32, f32, f32, f32, f32, f32, f32, f32);

/// Which component's style a verb reaches.
#[derive(Clone, Copy)]
pub(super) enum Ribbon {
    Trail,
    Line,
}

impl Ribbon {
    /// Read `id`'s style through `f`, or `None` without the component.
    fn read<T>(
        self,
        scene: &RefCell<Scene>,
        id: u32,
        f: impl FnOnce(&RibbonStyle) -> T,
    ) -> Option<T> {
        let s = scene.borrow();
        match self {
            Self::Trail => s.world.trail(id).map(|t| f(&t.style)),
            Self::Line => s.world.line(id).map(|l| f(&l.style)),
        }
    }

    /// Apply `f` to `id`'s style, when it has the component.
    fn write(self, scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut RibbonStyle)) {
        let mut s = scene.borrow_mut();
        match self {
            Self::Trail => s.world.trail_mut(id).map(|mut t| f(&mut t.style)),
            Self::Line => s.world.line_mut(id).map(|mut l| f(&mut l.style)),
        };
    }
}

/// Register the shared style verbs onto `t` for `which` component.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
    which: Ribbon,
) -> Reg {
    register_width(scope, t, scene, which)?;
    register_color(scope, t, scene, which)?;
    register_look(scope, t, scene, which)
}

/// `GetWidth(id, t?)`, `SetWidth(id, start, end?)`, `SetWidthCurve(id, {{t, w}, …})`.
fn register_width<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
    which: Ribbon,
) -> Reg {
    let f = scope.create_function(move |_, (id, at): (u32, Option<f32>)| {
        Ok(which
            .read(scene, id, |s| s.width.evaluate(at.unwrap_or(0.0)))
            .unwrap_or(0.0))
    });
    put(t, "GetWidth", f)?;
    let f = scope.create_function(move |_, (id, start, end): (u32, f32, Option<f32>)| {
        let curve = Curve::linear(start, end.unwrap_or(start));
        which.write(scene, id, |s| ops::set_width(s, curve));
        Ok(())
    });
    put(t, "SetWidth", f)?;
    let f = scope.create_function(move |_, (id, keys): (u32, Vec<[f32; 2]>)| {
        let pairs: Vec<(f32, f32)> = keys.iter().map(|&[k, v]| (k, v)).collect();
        which.write(scene, id, |s| ops::set_width(s, Curve::from_keys(&pairs)));
        Ok(())
    });
    put(t, "SetWidthCurve", f)
}

/// `GetColor(id, t?)` → r, g, b, a; `SetColor(id, r, g, b, a?)`;
/// `SetColors(id, start rgba, end rgba)`.
fn register_color<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
    which: Ribbon,
) -> Reg {
    let f = scope.create_function(move |_, (id, at): (u32, Option<f32>)| {
        let c = which.read(scene, id, |s| s.color.evaluate(at.unwrap_or(0.0)));
        let [r, g, b, a] = c.unwrap_or([0.0; 4]);
        Ok::<Rgba, _>((r, g, b, a))
    });
    put(t, "GetColor", f)?;
    let f = scope.create_function(move |_, (id, r, g, b, a): IdColor| {
        let gradient = Gradient::solid([r, g, b, a.unwrap_or(1.0)]);
        which.write(scene, id, |s| ops::set_color(s, gradient));
        Ok(())
    });
    put(t, "SetColor", f)?;
    let f = scope.create_function(move |_, c: IdColors| {
        let gradient = Gradient::linear([c.1, c.2, c.3, c.4], [c.5, c.6, c.7, c.8]);
        which.write(scene, c.0, |s| ops::set_color(s, gradient));
        Ok(())
    });
    put(t, "SetColors", f)
}

/// `Get/SetTexture` (a path, `nil` clears), `Get/SetTextureMode` (`"Stretch"` /
/// `"Tile"`), `Get/SetBlend` (`"Alpha"` / `"Additive"`). Unknown names are ignored.
fn register_look<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: &'scope RefCell<Scene>,
    which: Ribbon,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| {
        Ok(which.read(scene, id, |s| s.texture.clone()).flatten())
    });
    put(t, "GetTexture", f)?;
    let f = scope.create_function(move |_, (id, path): (u32, Option<String>)| {
        which.write(scene, id, |s| ops::set_texture(s, path.unwrap_or_default()));
        Ok(())
    });
    put(t, "SetTexture", f)?;
    let f = scope
        .create_function(move |_, id: u32| Ok(which.read(scene, id, |s| s.texture_mode.name())));
    put(t, "GetTextureMode", f)?;
    let f = scope.create_function(move |_, (id, name): (u32, String)| {
        if let Some(mode) = TextureMode::parse(&name) {
            which.write(scene, id, |s| ops::set_texture_mode(s, mode));
        }
        Ok(())
    });
    put(t, "SetTextureMode", f)?;
    let f =
        scope.create_function(move |_, id: u32| Ok(which.read(scene, id, |s| blend_name(s.blend))));
    put(t, "GetBlend", f)?;
    let f = scope.create_function(move |_, (id, name): (u32, String)| {
        if let Some(blend) = parse_blend(&name) {
            which.write(scene, id, |s| ops::set_blend(s, blend));
        }
        Ok(())
    });
    put(t, "SetBlend", f)
}

fn blend_name(blend: ParticleBlend) -> &'static str {
    match blend {
        ParticleBlend::Alpha => "Alpha",
        ParticleBlend::Additive => "Additive",
    }
}

fn parse_blend(name: &str) -> Option<ParticleBlend> {
    match name.to_lowercase().as_str() {
        "alpha" => Some(ParticleBlend::Alpha),
        "additive" => Some(ParticleBlend::Additive),
        _ => None,
    }
}
