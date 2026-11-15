//! The pair's edges: a NaN, a list past `max_particles`, a dead entry, no
//! emitter, and an entry that is not a table.

use glam::Vec3;

use super::{dump, lua, scene};

#[test]
fn a_nan_or_an_infinity_leaves_the_field_as_it_was() {
    let (scene, id) = scene(2);
    let before = dump(&scene, id);
    lua::<()>(
        &scene,
        &format!(
            "local ps = Particles.GetParticles({id})
             ps[1].position.x = 0/0; ps[1].age = 1/0; ps[1].rotation = -1/0
             ps[2].color = {{ r = 0/0 }}; ps[2].custom = {{ 0/0, 1/0 }}
             Particles.SetParticles({id}, ps)"
        ),
    )
    .unwrap();
    assert_eq!(dump(&scene, id), before);
}

#[test]
fn a_longer_list_adds_particles_up_to_max_particles() {
    let (scene, id) = scene(2);
    let live: u32 = lua(
        &scene,
        &format!(
            "local ps = Particles.GetParticles({id})
             for i = 1, 10 do ps[#ps + 1] = {{ position = {{ x = i, y = 0, z = 0 }} }} end
             return Particles.SetParticles({id}, ps)"
        ),
    )
    .unwrap();
    assert_eq!(live, 8, "capped at max_particles");
    let s = scene.borrow();
    let e = s.world.particles(id).unwrap();
    let added = e.runtime.particles[2];
    assert_eq!(added.position, Vec3::X);
    assert_eq!((added.age, added.lifetime, added.size), (0.0, 50.0, 0.1));
    assert_eq!((added.velocity, added.custom), (Vec3::ZERO, [0.0; 4]));
}

#[test]
fn dead_entries_are_dropped_and_no_emitter_reads_empty() {
    let (scene, id) = scene(3);
    let live: u32 = lua(
        &scene,
        &format!(
            "local ps = Particles.GetParticles({id})
             ps[2].age = ps[2].lifetime
             return Particles.SetParticles({id}, ps)"
        ),
    )
    .unwrap();
    assert_eq!(live, 2);
    let bare = scene.borrow_mut().add_entity("Bare".to_string());
    let (n, set): (usize, u32) = lua(
        &scene,
        &format!(
            "return #Particles.GetParticles({bare}), Particles.SetParticles({bare}, {{ {{}} }})"
        ),
    )
    .unwrap();
    assert_eq!((n, set), (0, 0));
    let err = lua::<()>(&scene, &format!("Particles.SetParticles({id}, {{ 3 }})")).unwrap_err();
    assert!(err.to_string().contains("entry 1"), "{err}");
    assert_eq!(
        scene.borrow().world.particles(id).unwrap().live_count(),
        2,
        "unchanged"
    );
}
