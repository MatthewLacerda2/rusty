//! `Component.Set` on the "read it, change it, `Set` it back" path (#1369's
//! review): paired fields, holed lists, unchanged keys and number coercion.

use std::cell::RefCell;

use mlua::Lua;

use super::register;
use crate::scene::authoring::{add_component, ComponentKind};
use crate::scene::Scene;

/// Run `chunk` over entity `id` carrying every Add Component kind, after `prep`.
fn run<R: mlua::FromLuaMulti>(prep: impl FnOnce(&mut Scene, u32), chunk: &str) -> (Scene, R) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Dressed".to_string());
    for add in ComponentKind::ALL {
        add_component(&mut scene, id, *add);
    }
    prep(&mut scene, id);
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.globals().set("id", id).unwrap();
    let out = lua.scope(|s| {
        register(&lua, s, &scene).unwrap();
        lua.load(chunk).eval::<R>()
    });
    (scene.into_inner(), out.unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn a_paired_op_sees_both_new_values() {
    // Both past the other half's current value: the light's outer cone is 45, the
    // text's auto-size max 72.
    let (_, out) = run::<(f32, f32, f32, f32)>(
        |_, _| {},
        r#"
        Component.Set(id, "Light", { light_type = "Spotlight", inner_cone = 70, outer_cone = 80 })
        Component.Set(id, "Text", { auto_size_min = 80, auto_size_max = 100 })
        local l, t = Component.Get(id, "Light"), Component.Get(id, "Text")
        return l.inner_cone, l.outer_cone, t.auto_size_min, t.auto_size_max
        "#,
    );
    assert_eq!(out, (70.0, 80.0, 80.0, 100.0));
}

#[test]
fn a_holed_list_round_trips() {
    let (scene, n) = run::<u32>(
        |_, _| {},
        r#"
        Component.Set(id, "Selectable", { sprites = { nil, "hover.png", nil, "off.png" } })
        local s = Component.Get(id, "Selectable")
        s.fade_duration = 0.5
        Component.Set(id, "Selectable", s)
        s = Component.Get(id, "Selectable")
        s.sprites[3] = "down.png"
        Component.Set(id, "Selectable", s)
        local n = 0
        for _ in pairs(Component.Get(id, "Selectable").sprites) do n = n + 1 end
        return n
        "#,
    );
    let sprites = scene.world.selectable(1).unwrap().sprites.clone();
    let names: Vec<Option<String>> = sprites
        .iter()
        .map(|s| s.as_ref().map(|p| p.as_str().to_owned()))
        .collect();
    let want = [
        None,
        Some("hover.png"),
        Some("down.png"),
        Some("off.png"),
        None,
    ];
    assert_eq!(names, want.map(|s| s.map(str::to_owned)));
    assert_eq!(n, 3, "the holes stay holes");
}

#[test]
fn an_unchanged_key_runs_no_op() {
    let (scene, out) = run::<(String, f32, f32)>(
        |scene, id| {
            let mut a = scene.world.animator_mut(id).unwrap();
            a.graph = Some("rig.graph".to_string());
            a.base.current_node = Some("Run".to_string());
        },
        r#"
        local z = Component.Get(id, "AudioReverbZone")
        z.params.decay_time = 3
        Component.Set(id, "AudioReverbZone", z)
        local a = Component.Get(id, "Animator")
        a.speed = 2
        Component.Set(id, "Animator", a)
        Component.Set(id, "Camera", { fov = 60 })
        local c = Component.Get(id, "Camera")
        c.fov = 70
        Component.Set(id, "Camera", c)
        z = Component.Get(id, "AudioReverbZone")
        return z.preset, z.params.decay_time, Component.Get(id, "Camera").fov
        "#,
    );
    assert_eq!(out, ("Custom".to_string(), 3.0, 70.0));
    let a = scene.world.animator(1).unwrap();
    assert_eq!(a.speed, 2.0);
    assert_eq!(
        a.base.current_node.as_deref(),
        Some("Run"),
        "graph not reset"
    );
}

#[test]
fn numbers_coerce_as_the_named_setters_do() {
    let (scene, ()) = run::<()>(
        |_, _| {},
        r#"
        Component.Set(id, "Camera", { culling_mask = -1, render_order = 2.0 })
        Component.Set(id, "NavMeshAgent", { area_mask = -1 ~ 4 })
        Component.Set(id, "LODGroup", { levels = { { screen_height = 0.5, renderers = {} } } })
        "#,
    );
    let cam = scene.world.camera(1).unwrap();
    assert_eq!((cam.culling_mask, cam.render_order), (u32::MAX, 2));
    assert_eq!(scene.world.nav_agent(1).unwrap().area_mask, !4);
    let levels = &scene.world.lod_group(1).unwrap().levels;
    assert_eq!(levels.len(), 1);
    assert!(levels[0].renderers.is_empty());
}

#[test]
fn fields_names_an_optional_type() {
    let (_, out) = run::<(String, String, String)>(
        |_, _| {},
        r#"
        local function ty(kind, name)
            for _, row in ipairs(Component.Fields(kind)) do
                if row.name == name then return row.type end
            end
        end
        return ty("Camera", "target_texture"), ty("Joint", "connected_body"),
            ty("Camera", "fov")
        "#,
    );
    let want = ("optional table", "optional number", "number");
    assert_eq!((out.0.as_str(), out.1.as_str(), out.2.as_str()), want);
}
