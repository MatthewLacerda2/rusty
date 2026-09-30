//! `Particles.*` bindings vs `authoring::particles`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::ParticleEmitterComponent;
use rusty::scene::authoring::particles as particle_ops;
use rusty::scene::Scene;

/// Attach a default emitter to a fresh entity; returns its id.
fn entity_with_emitter(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent::default()));
    id
}

#[test]
fn particles_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // The Particles field-write bindings are `SetActive` and `SetRate` (with its `≥ 0`
    // clamp); both share the ops the card calls.
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_emitter(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_emitter(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::particle::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Particles.SetActive({via_lua}, false)
            Particles.SetRate({via_lua}, -5.0)
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.particles_mut(via_op).unwrap();
        particle_ops::set_active(&mut e, false);
        particle_ops::set_rate(&mut e, -5.0);
    }

    let sc = scene.borrow();
    let a = sc.world.particles(via_lua).unwrap().clone();
    let b = sc.world.particles(via_op).unwrap().clone();
    assert_eq!(a.active, b.active);
    assert_eq!(a.rate, b.rate);
    assert_eq!(a.rate, 0.0, "single-sourced clamp");
    Ok(())
}
