//! Integration coverage for the `Particles` scripting namespace (issue #350):
//! scripted emission goes through the component's seeded spawn path and respects
//! the max-particle cap, `Burst` fires the configured count, tuning verbs write
//! the component, and every verb is a safe no-op without an emitter.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::{ParticleEmitterComponent, Scene};

/// A scene with one emitter entity (engine defaults: burst 32, cap 256) and one
/// bare entity with no emitter.
fn scene_with_emitter() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene.get_entity_mut(id).unwrap().particles = Some(ParticleEmitterComponent::default());
    let bare = scene.add_entity("NoEmitter".to_string());
    (RefCell::new(scene), id, bare)
}

#[test]
fn emit_spawns_counts_and_clears() {
    let lua = Lua::new();
    let (scene, id, _) = scene_with_emitter();

    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();

        let spawned: u32 = lua
            .load(format!("return Particles.Emit({id}, 5)"))
            .eval()
            .unwrap();
        assert_eq!(spawned, 5, "Emit reports the number actually spawned");
        let count: u32 = lua
            .load(format!("return Particles.GetCount({id})"))
            .eval()
            .unwrap();
        assert_eq!(count, 5);

        lua.load(format!("Particles.Clear({id})")).exec().unwrap();
        let count: u32 = lua
            .load(format!("return Particles.GetCount({id})"))
            .eval()
            .unwrap();
        assert_eq!(count, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn emit_is_swallowed_at_the_max_particle_cap() {
    let lua = Lua::new();
    let (scene, id, _) = scene_with_emitter();

    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();
        // Default cap is 256: an oversized request is truncated, a full emitter spawns 0.
        let spawned: u32 = lua
            .load(format!("return Particles.Emit({id}, 999)"))
            .eval()
            .unwrap();
        assert_eq!(spawned, 256, "spawn count truncates at max_particles");
        let more: u32 = lua
            .load(format!("return Particles.Emit({id}, 1)"))
            .eval()
            .unwrap();
        assert_eq!(more, 0, "a full emitter spawns nothing");
        Ok(())
    })
    .unwrap();
}

#[test]
fn burst_fires_the_configured_burst_count() {
    let lua = Lua::new();
    let (scene, id, _) = scene_with_emitter();

    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();
        let spawned: u32 = lua
            .load(format!("return Particles.Burst({id})"))
            .eval()
            .unwrap();
        assert_eq!(spawned, 32, "default burst_count");
        Ok(())
    })
    .unwrap();
}

#[test]
fn tuning_verbs_write_the_component() {
    let lua = Lua::new();
    let (scene, id, _) = scene_with_emitter();

    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();
        lua.load(format!(
            "Particles.SetActive({id}, false); Particles.SetRate({id}, 7.5)"
        ))
        .exec()
        .unwrap();

        let active: bool = lua
            .load(format!("return Particles.IsActive({id})"))
            .eval()
            .unwrap();
        assert!(!active);
        let s = scene.borrow();
        let p = s.get_entity(id).unwrap().particles.clone().unwrap();
        assert!(!p.active);
        assert_eq!(p.rate, 7.5);
        Ok(())
    })
    .unwrap();
}

#[test]
fn verbs_are_safe_noops_without_an_emitter() {
    let lua = Lua::new();
    let (scene, _, bare) = scene_with_emitter();

    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();
        for target in [bare, 999] {
            let spawned: u32 = lua
                .load(format!("return Particles.Emit({target}, 5)"))
                .eval()
                .unwrap();
            assert_eq!(spawned, 0);
            let active: bool = lua
                .load(format!("return Particles.IsActive({target})"))
                .eval()
                .unwrap();
            assert!(!active);
            lua.load(format!(
                "Particles.SetRate({target}, 1); Particles.Clear({target})"
            ))
            .exec()
            .unwrap();
        }
        Ok(())
    })
    .unwrap();
}
