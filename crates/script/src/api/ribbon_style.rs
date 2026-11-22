use std::cell::RefCell;

use mlua::Table;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_asset::MapRef;
use rusty_components::{ParticleBlend, RibbonStyle, TextureMode};
use rusty_core::curve::{Curve, Gradient};
use rusty_edit::authoring::ribbon as ops;
use rusty_scene::Scene;

type Rgba = (f32, f32, f32, f32);
type IdColor = (EntityArg, f32, f32, f32, Option<f32>);
type IdColors = (EntityArg, f32, f32, f32, f32, f32, f32, f32, f32);

#[derive(Clone, Copy)]
pub(super) enum Ribbon {
    Trail,
    Line,
}

impl Ribbon {
    fn name(self) -> &'static str {
        match self {
            Self::Trail => "Trail",
            Self::Line => "Line",
        }
    }

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

    fn write(self, scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut RibbonStyle)) {
        let mut s = scene.borrow_mut();
        match self {
            Self::Trail => s.world.trail_mut(id).map(|mut t| f(&mut t.style)),
            Self::Line => s.world.line_mut(id).map(|mut l| f(&mut l.style)),
        };
    }
}

pub(super) fn register(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>, which: Ribbon) -> Reg {
    register_width(lua, t, scene, which)?;
    register_color(lua, t, scene, which)?;
    register_look(lua, t, scene, which)
}

fn register_width(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>, which: Ribbon) -> Reg {
    let f = lua.create_function(move |lua, (id, at): (EntityArg, Option<f32>)| {
        Ok(which
            .read(&*scene.get(lua)?, id.read(), |s| {
                s.width.evaluate(at.unwrap_or(0.0))
            })
            .unwrap_or(0.0))
    });
    put(t, "GetWidth", f)?;
    let f = lua.create_function(
        move |lua, (id, start, end): (EntityArg, f32, Option<f32>)| {
            let id = id.live(which.name(), "SetWidth")?;
            let curve = Curve::linear(start, end.unwrap_or(start));
            which.write(&*scene.get(lua)?, id, |s| ops::set_width(s, curve));
            Ok(())
        },
    );
    put(t, "SetWidth", f)?;
    let f = lua.create_function(move |lua, (id, keys): (EntityArg, Vec<[f32; 2]>)| {
        let id = id.live(which.name(), "SetWidthCurve")?;
        let pairs: Vec<(f32, f32)> = keys.iter().map(|&[k, v]| (k, v)).collect();
        which.write(&*scene.get(lua)?, id, |s| {
            ops::set_width(s, Curve::from_keys(&pairs))
        });
        Ok(())
    });
    put(t, "SetWidthCurve", f)
}

fn register_color(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>, which: Ribbon) -> Reg {
    let f = lua.create_function(move |lua, (id, at): (EntityArg, Option<f32>)| {
        let c = which.read(&*scene.get(lua)?, id.read(), |s| {
            s.color.evaluate(at.unwrap_or(0.0))
        });
        let [r, g, b, a] = c.unwrap_or([0.0; 4]);
        Ok::<Rgba, _>((r, g, b, a))
    });
    put(t, "GetColor", f)?;
    let f = lua.create_function(move |lua, (id, r, g, b, a): IdColor| {
        let id = id.live(which.name(), "SetColor")?;
        let gradient = Gradient::solid([r, g, b, a.unwrap_or(1.0)]);
        which.write(&*scene.get(lua)?, id, |s| ops::set_color(s, gradient));
        Ok(())
    });
    put(t, "SetColor", f)?;
    let f = lua.create_function(move |lua, c: IdColors| {
        let id = c.0.live(which.name(), "SetColors")?;
        let gradient = Gradient::linear([c.1, c.2, c.3, c.4], [c.5, c.6, c.7, c.8]);
        which.write(&*scene.get(lua)?, id, |s| ops::set_color(s, gradient));
        Ok(())
    });
    put(t, "SetColors", f)
}

fn register_look(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>, which: Ribbon) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(which
            .read(&*scene.get(lua)?, id.read(), |s| {
                s.texture.as_ref().map(MapRef::to_string)
            })
            .flatten())
    });
    put(t, "GetTexture", f)?;
    let f = lua.create_function(move |lua, (id, path): (EntityArg, Option<String>)| {
        let id = id.live(which.name(), "SetTexture")?;
        which.write(&*scene.get(lua)?, id, |s| {
            ops::set_texture(s, path.unwrap_or_default())
        });
        Ok(())
    });
    put(t, "SetTexture", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(which.read(&*scene.get(lua)?, id.read(), |s| s.texture_mode.name()))
    });
    put(t, "GetTextureMode", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live(which.name(), "SetTextureMode")?;
        if let Some(mode) = TextureMode::parse(&name) {
            which.write(&*scene.get(lua)?, id, |s| ops::set_texture_mode(s, mode));
        }
        Ok(())
    });
    put(t, "SetTextureMode", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(which.read(&*scene.get(lua)?, id.read(), |s| s.blend.name()))
    });
    put(t, "GetBlend", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live(which.name(), "SetBlend")?;
        if let Some(blend) = ParticleBlend::parse(&name) {
            which.write(&*scene.get(lua)?, id, |s| ops::set_blend(s, blend));
        }
        Ok(())
    });
    put(t, "SetBlend", f)
}
