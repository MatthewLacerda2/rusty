use glam::{Quat, Vec3};
use rusty::components::particle::{Flipbook, ParticleRenderMode};

use super::{dump, emitter, lua, scene_with};

/// Every hidden field differs between particles, so a shuffled survivor shows.
pub(super) fn hidden(n: u32) -> (std::rc::Rc<std::cell::RefCell<rusty::scene::Scene>>, u32) {
    let mut cfg = emitter(n);
    cfg.render.mode = ParticleRenderMode::Mesh;
    cfg.render.flipbook = Flipbook {
        columns: 4,
        rows: 4,
        random_start: true,
        ..Default::default()
    };
    let (scene, id) = scene_with(cfg);
    let mut s = scene.borrow_mut();
    for (i, p) in s
        .world
        .particles_mut(id)
        .unwrap()
        .runtime
        .particles
        .iter_mut()
        .enumerate()
    {
        p.turn = Quat::from_rotation_y(0.3 * (i + 1) as f32);
    }
    drop(s);
    (scene, id)
}

fn without(
    scene: &std::rc::Rc<std::cell::RefCell<rusty::scene::Scene>>,
    id: u32,
    gone: usize,
) -> String {
    let mut live = scene
        .borrow()
        .world
        .particles(id)
        .unwrap()
        .runtime
        .particles
        .clone();
    live.remove(gone);
    format!("{live:?}")
}

#[test]
fn a_removed_middle_entry_leaves_each_survivor_its_hidden_fields() {
    for code in [
        "local ps = Particles.GetParticles(ID); table.remove(ps, 2); Particles.SetParticles(ID, ps)",
        "local keep = {}
         for i, p in ipairs(Particles.GetParticles(ID)) do if i ~= 2 then keep[#keep + 1] = p end end
         Particles.SetParticles(ID, keep)",
    ] {
        let (scene, id) = hidden(5);
        {
            let s = scene.borrow();
            let live = &s.world.particles(id).unwrap().runtime.particles;
            assert_ne!(live[1].axis, live[2].axis, "the fixture's axes differ");
            assert!(live.iter().any(|p| p.start_frame != live[0].start_frame));
        }
        let expected = without(&scene, id, 1);
        lua::<()>(&scene, &code.replace("ID", &id.to_string())).unwrap();
        assert_eq!(dump(&scene, id), expected, "{code}");
    }
}

#[test]
fn a_table_from_an_older_read_takes_the_hidden_defaults() {
    let (scene, id) = hidden(2);
    lua::<()>(
        &scene,
        &format!(
            "local old = Particles.GetParticles({id})
             Particles.GetParticles({id})
             Particles.SetParticles({id}, old)"
        ),
    )
    .unwrap();
    let s = scene.borrow();
    for p in &s.world.particles(id).unwrap().runtime.particles {
        assert_eq!(
            (p.axis, p.start_frame, p.turn),
            (Vec3::Y, 0, Quat::IDENTITY)
        );
    }
}

#[test]
fn a_write_through_the_arrays_makes_an_earlier_read_the_scripts_own_data() {
    // Every particle of the burst has age 0, so only the emitter's revision
    // tells the read in `ps` from the particles after the arrays' kill.
    let (scene, id) = hidden(3);
    lua::<()>(
        &scene,
        &format!(
            "local ps = Particles.GetParticles({id})
             local a = Particles.GetArrays({id}, {{ 'age', 'lifetime' }})
             a.age[1] = a.lifetime[1]
             Particles.SetArrays({id}, a)
             table.remove(ps, 1)
             Particles.SetParticles({id}, ps)"
        ),
    )
    .unwrap();
    let s = scene.borrow();
    let live = &s.world.particles(id).unwrap().runtime.particles;
    assert_eq!(live.len(), 2);
    for p in live {
        assert_eq!(
            (p.axis, p.start_frame, p.turn),
            (Vec3::Y, 0, Quat::IDENTITY)
        );
    }
}
