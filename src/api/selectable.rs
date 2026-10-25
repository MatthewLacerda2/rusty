//! src/api/selectable.rs — `Selectable` namespace (#420).
//!
//! Get/Set over an entity's `SelectableComponent`: interactable, the transition and
//! its target graphic, per-state colours and sprites, the fade, and navigation
//! (mode + explicit targets). Every setter routes through the shared
//! `scene::authoring::selectable` ops the inspector card uses. `IsInteractable` and
//! `GetState` read the effective, runtime answers (CanvasGroups and the event
//! system included). Getters return a neutral default without a Selectable; setters
//! are then no-ops. States, directions and enum values travel as their names
//! (case-insensitive; unknown names are ignored by setters and read as `nil`).

use std::cell::RefCell;

use glam::Vec4;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::{NavigationMode, SelectableTransition};
use crate::components::{SelectableComponent, SelectionState};
use crate::scene::authoring::selectable as ops;
use crate::scene::Scene;
use crate::ui::events::is_interactable;
use crate::ui::EventSystem;

type SceneCell<'s> = &'s RefCell<Scene>;

/// Register the `Selectable` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    events: &'scope RefCell<EventSystem>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_state(scope, &t, scene, events)?;
    register_transition(scope, &t, scene)?;
    register_navigation(scope, &t, scene)?;
    lua.globals()
        .set("Selectable", t)
        .map_err(|e| e.to_string())
}

/// `id`'s Selectable, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<SelectableComponent> {
    scene.borrow().world.selectable(id).map(|s| s.clone())
}

/// Apply `f` to `id`'s Selectable, when it has one.
fn set(scene: SceneCell, id: u32, f: &dyn Fn(&mut SelectableComponent)) {
    if let Some(mut s) = scene.borrow_mut().world.selectable_mut(id) {
        f(&mut s);
    }
}

/// `Get/SetInteractable`, `IsInteractable`, `GetState`.
fn register_state<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
    events: &'scope RefCell<EventSystem>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).is_some_and(|s| s.interactable))
    });
    put(t, "GetInteractable", f)?;
    let f = scope.create_function(move |_, (id, v): (EntityArg, bool)| {
        let id = id.live("Selectable", "SetInteractable")?;
        set(scene, id, &|s| ops::set_interactable(s, v));
        Ok(())
    });
    put(t, "SetInteractable", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        let id = id.read();
        let world = &scene.borrow().world;
        Ok(world.has_selectable(id) && is_interactable(world, id))
    });
    put(t, "IsInteractable", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        let id = id.read();
        let world = &scene.borrow().world;
        let has = world.has_selectable(id);
        Ok(has.then(|| events.borrow().state_of(world, id).name()))
    });
    put(t, "GetState", f)
}

/// `Get/SetTransition`, `Get/SetTargetGraphic`, `Get/SetFadeDuration`, then the
/// per-state pairs.
fn register_transition<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|s| s.transition.name()))
    });
    put(t, "GetTransition", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live("Selectable", "SetTransition")?;
        if let Some(v) = SelectableTransition::parse(&name) {
            set(scene, id, &|s| ops::set_transition(s, v));
        }
        Ok(())
    });
    put(t, "SetTransition", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).and_then(|s| s.target_graphic))
    });
    put(t, "GetTargetGraphic", f)?;
    let f = scope.create_function(move |_, (id, target): (EntityArg, EntityArg)| {
        let id = id.live("Selectable", "SetTargetGraphic")?;
        let target = target.get();
        set(scene, id, &|s| ops::set_target_graphic(s, target));
        Ok(())
    });
    put(t, "SetTargetGraphic", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map_or(0.0, |s| s.fade_duration))
    });
    put(t, "GetFadeDuration", f)?;
    let f = scope.create_function(move |_, (id, v): (EntityArg, f32)| {
        let id = id.live("Selectable", "SetFadeDuration")?;
        set(scene, id, &|s| ops::set_fade_duration(s, v));
        Ok(())
    });
    put(t, "SetFadeDuration", f)?;
    register_per_state(scope, t, scene)
}

/// `Get/SetColor` and `Get/SetSprite`, keyed by state name.
fn register_per_state<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let c = SelectionState::parse(&name).and_then(|st| Some(get(scene, id.read())?.color(st)));
        let c = c.unwrap_or(Vec4::ZERO);
        Ok((c.x, c.y, c.z, c.w))
    });
    put(t, "GetColor", f)?;
    type Rgba = (EntityArg, String, f32, f32, f32, Option<f32>);
    let f = scope.create_function(move |_, (id, name, r, g, b, a): Rgba| {
        let id = id.live("Selectable", "SetColor")?;
        if let Some(st) = SelectionState::parse(&name) {
            let c = Vec4::new(r, g, b, a.unwrap_or(1.0));
            set(scene, id, &|s| ops::set_color(s, st, c));
        }
        Ok(())
    });
    put(t, "SetColor", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let st = SelectionState::parse(&name);
        Ok(get(scene, id.read()).and_then(|s| Some(s.sprite(st?)?.to_string())))
    });
    put(t, "GetSprite", f)?;
    let f = scope.create_function(
        move |_, (id, name, path): (EntityArg, String, Option<String>)| {
            let id = id.live("Selectable", "SetSprite")?;
            if let Some(st) = SelectionState::parse(&name) {
                set(scene, id, &|s| ops::set_sprite(s, st, path.clone()));
            }
            Ok(())
        },
    );
    put(t, "SetSprite", f)
}

/// `Get/SetNavigation`, `Get/SetSelectOn`.
fn register_navigation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|s| s.navigation.name()))
    });
    put(t, "GetNavigation", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live("Selectable", "SetNavigation")?;
        if let Some(v) = NavigationMode::parse(&name) {
            set(scene, id, &|s| ops::set_navigation(s, v));
        }
        Ok(())
    });
    put(t, "SetNavigation", f)?;
    let f = scope.create_function(move |_, (id, dir): (EntityArg, String)| {
        let i = ops::direction_index(&dir);
        Ok(get(scene, id.read()).and_then(|s| s.select_on[i?]))
    });
    put(t, "GetSelectOn", f)?;
    let f = scope.create_function(
        move |_, (id, dir, target): (EntityArg, String, EntityArg)| {
            let id = id.live("Selectable", "SetSelectOn")?;
            let target = target.get();
            if let Some(i) = ops::direction_index(&dir) {
                set(scene, id, &|s| ops::set_select_on(s, i, target));
            }
            Ok(())
        },
    );
    put(t, "SetSelectOn", f)
}
