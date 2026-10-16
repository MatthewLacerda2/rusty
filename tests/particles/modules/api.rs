//! The #439 `Particles` setters write the component through the shared ops.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::particle::{EmitFrom, EmitShape, ParticleEmitterComponent};
use rusty::core::curve::Range;
use rusty::scene::Scene;

#[test]
fn module_setters_write_the_emitter() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent::default()));
    let smoke = scene.add_entity("Smoke".to_string());
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    let (ok, bad_shape, bad_trigger): (bool, bool, bool) = lua
        .scope(|s| {
            rusty::api::particle::register(&lua, s, &scene).unwrap();
            lua.load(format!(
                r#"
                local ok = Particles.SetShape({id}, "cone", {{ angle = 40, radius = 0.2, surface = true }})
                Particles.SetDirection({id}, 0, 0, 1)
                Particles.SetLifetime({id}, 0.5, 1.5)
                Particles.SetSpeed({id}, 3)
                Particles.SetSize({id}, -1, 0.4)
                Particles.SetColor({id}, 1, 0.5, 0)
                Particles.SetSubEmitter({id}, "Death", {smoke})
                return ok, Particles.SetShape({id}, "torus"),
                    Particles.SetSubEmitter({id}, "landing", {smoke})
            "#
            ))
            .eval()
        })
        .unwrap();
    assert!(ok && !bad_shape && !bad_trigger);

    let sc = scene.borrow();
    let e = sc.world.particles(id).unwrap();
    assert_eq!(
        e.shape,
        EmitShape::Cone {
            angle: 40.0,
            radius: 0.2
        }
    );
    assert_eq!(e.emit_from, EmitFrom::Surface);
    assert_eq!(e.direction, Vec3::Z);
    assert_eq!(e.lifetime, Range::new(0.5, 1.5));
    assert_eq!(e.speed, Range::constant(3.0));
    assert_eq!(e.size, Range::new(0.0, 0.4), "size clamps to 0");
    assert_eq!(e.color.min, [1.0, 0.5, 0.0, 1.0]);
    assert_eq!(e.sub_emitters.death, Some(smoke));
}

#[test]
fn clearing_a_sub_emitter_takes_nil() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    let mut cfg = ParticleEmitterComponent::default();
    cfg.sub_emitters.birth = Some(7);
    scene.world.set_particles(id, Some(cfg));
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::particle::register(&lua, s, &scene).unwrap();
        lua.load(format!("Particles.SetSubEmitter({id}, 'birth', nil)"))
            .exec()
    })
    .unwrap();
    assert_eq!(
        scene
            .borrow()
            .world
            .particles(id)
            .unwrap()
            .sub_emitters
            .birth,
        None
    );
}
