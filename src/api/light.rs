//! src/api/light.rs — `Light` namespace.
//!
//! Get/Set over an entity's optional `LightComponent`: colour, intensity, range
//! and light type. Every setter maps onto a real field the renderer reads
//! (`apply_scene_lights`), so a script tweak takes effect on the next frame's
//! lighting uniform. There is no per-light "active" flag in the engine — a light
//! is gated by its owning entity's `active` (a `Scene` concern), so this surface
//! deliberately exposes the light's own data and nothing more.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::components::LightType;
use crate::scene::Scene;

/// Register the `Light` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_color(lua, &table, scene)?;
    register_intensity(lua, &table, scene)?;
    register_range(lua, &table, scene)?;
    register_type(lua, &table, scene)?;

    lua.globals().set("Light", table).map_err(|e| e.to_string())
}

/// `GetColor` / `SetColor` over the light's linear RGB colour.
fn register_color(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "GetColor",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let c = scene
                .get_entity(id)
                .and_then(|e| e.light.as_ref().map(|l| l.color));
            let c = c.unwrap_or(Vec3::ONE);
            Ok((c.x, c.y, c.z))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetColor",
        lua.create_function(move |_, (id, r, g, b): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(light) = &mut e.light {
                    light.color = Vec3::new(r, g, b);
                }
            }
            Ok(())
        }),
    )
}

/// `GetIntensity` / `SetIntensity` (clamped ≥ 0).
fn register_intensity(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "GetIntensity",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            Ok(scene
                .get_entity(id)
                .and_then(|e| e.light.as_ref().map(|l| l.intensity))
                .unwrap_or(0.0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetIntensity",
        lua.create_function(move |_, (id, value): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(light) = &mut e.light {
                    light.intensity = value.max(0.0);
                }
            }
            Ok(())
        }),
    )
}

/// `GetRange` / `SetRange` (clamped ≥ 0).
fn register_range(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "GetRange",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            Ok(scene
                .get_entity(id)
                .and_then(|e| e.light.as_ref().map(|l| l.range))
                .unwrap_or(0.0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetRange",
        lua.create_function(move |_, (id, value): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(light) = &mut e.light {
                    light.range = value.max(0.0);
                }
            }
            Ok(())
        }),
    )
}

/// `GetType` / `SetType` over the light's kind, by name. Unknown names on `SetType`
/// are ignored (the current type is kept).
fn register_type(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "GetType",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let name = scene
                .get_entity(id)
                .and_then(|e| e.light.as_ref().map(|l| type_name(&l.light_type)))
                .unwrap_or("None");
            Ok(name.to_string())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetType",
        lua.create_function(move |_, (id, name): (u32, String)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(light) = &mut e.light {
                    if let Some(t) = parse_type(&name) {
                        light.light_type = t;
                    }
                }
            }
            Ok(())
        }),
    )
}

/// Canonical name for a [`LightType`] (the value `GetType` returns / `SetType` takes).
fn type_name(t: &LightType) -> &'static str {
    match t {
        LightType::Ambient => "Ambient",
        LightType::Directional => "Directional",
        LightType::Point => "Point",
        LightType::Spotlight => "Spotlight",
    }
}

/// Parse a light-type name (case-insensitive); `None` for an unknown name.
fn parse_type(name: &str) -> Option<LightType> {
    match name.to_lowercase().as_str() {
        "ambient" => Some(LightType::Ambient),
        "directional" => Some(LightType::Directional),
        "point" => Some(LightType::Point),
        "spotlight" => Some(LightType::Spotlight),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::LightComponent;

    /// Build a scene with one light-bearing entity; returns `(scene, id)`.
    fn scene_with_light() -> (Rc<RefCell<Scene>>, u32) {
        let mut scene = Scene::default();
        let id = scene.add_entity("Lamp".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.light = Some(LightComponent {
                light_type: LightType::Point,
                color: Vec3::ONE,
                intensity: 1.5,
                range: 10.0,
                inner_cone: 30.0,
                outer_cone: 45.0,
            });
        }
        (Rc::new(RefCell::new(scene)), id)
    }

    #[test]
    fn set_get_color_intensity_range_type() {
        let (scene, id) = scene_with_light();
        let lua = Lua::new();
        register(&lua, &scene).unwrap();

        lua.load(format!("Light.SetColor({id}, 0.2, 0.4, 0.6)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetIntensity({id}, 3.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetRange({id}, 25.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetType({id}, 'Directional')"))
            .exec()
            .unwrap();

        let e = scene.borrow();
        let light = e.get_entity(id).unwrap().light.as_ref().unwrap().clone();
        assert_eq!(light.color, Vec3::new(0.2, 0.4, 0.6));
        assert_eq!(light.intensity, 3.0);
        assert_eq!(light.range, 25.0);
        assert_eq!(light.light_type, LightType::Directional);
    }

    #[test]
    fn clamps_negatives_and_ignores_unknown_type() {
        let (scene, id) = scene_with_light();
        let lua = Lua::new();
        register(&lua, &scene).unwrap();

        lua.load(format!("Light.SetIntensity({id}, -5.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetRange({id}, -2.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetType({id}, 'Bogus')"))
            .exec()
            .unwrap();

        let e = scene.borrow();
        let light = e.get_entity(id).unwrap().light.as_ref().unwrap().clone();
        assert_eq!(light.intensity, 0.0);
        assert_eq!(light.range, 0.0);
        // Unknown name kept the original Point type.
        assert_eq!(light.light_type, LightType::Point);
    }
}
