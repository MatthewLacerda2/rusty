//! `Particles.SetTexture` / `GetTexture` (#2052): the sprite image round-trips,
//! `nil` and `""` go back to the default sprite, and both are safe without an
//! emitter.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::scene::{ParticleEmitterComponent, Scene};

#[test]
fn set_texture_round_trips_and_nil_clears() {
    let lua = Lua::new();
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent::default()));
    let bare = scene.add_entity("NoEmitter".to_string());
    let scene = Rc::new(RefCell::new(scene));
    rusty::api::particle::register(&lua, bind(&lua, &scene)).unwrap();
    let get = |lua: &Lua, id: u32| -> Option<String> {
        lua.load(format!("return Particles.GetTexture({id})"))
            .eval()
            .unwrap()
    };

    assert_eq!(get(&lua, id), None, "a new emitter names no texture");
    lua.load(format!(r#"Particles.SetTexture({id}, "fx/smoke.png")"#))
        .exec()
        .unwrap();
    assert_eq!(get(&lua, id).as_deref(), Some("fx/smoke.png"));
    let stored = scene.borrow().world.particles(id).unwrap().texture.clone();
    assert_eq!(stored, Some("fx/smoke.png".into()), "the component's field");

    lua.load(format!("Particles.SetTexture({id}, nil)"))
        .exec()
        .unwrap();
    assert_eq!(get(&lua, id), None, "nil goes back to the default sprite");
    lua.load(format!(r#"Particles.SetTexture({id}, "a.png")"#))
        .exec()
        .unwrap();
    lua.load(format!(r#"Particles.SetTexture({id}, "")"#))
        .exec()
        .unwrap();
    assert_eq!(get(&lua, id), None, "an empty path clears too");

    lua.load(format!(r#"Particles.SetTexture({bare}, "a.png")"#))
        .exec()
        .unwrap();
    assert_eq!(get(&lua, bare), None, "no emitter: a no-op and nil");
}
