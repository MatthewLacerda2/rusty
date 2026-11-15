//! `Particles.GetArrays` / `SetArrays`: one array per field, index-aligned with
//! the live particles, written back in place, and refused once stale.

use super::{dump, lua, scene};

const ALL: &str = "{ 'x', 'y', 'z', 'vx', 'vy', 'vz', 'age', 'lifetime', 'size', 'rotation',
  'r', 'g', 'b', 'a', 'custom1', 'custom2', 'custom3', 'custom4' }";

#[test]
fn a_round_trip_of_every_field_leaves_the_emitter_bit_identical() {
    let (scene, id) = super::identity::hidden(4);
    let before = dump(&scene, id);
    let live: u32 = lua(
        &scene,
        &format!("return Particles.SetArrays({id}, Particles.GetArrays({id}, {ALL}))"),
    )
    .unwrap();
    assert_eq!(live, 4);
    assert_eq!(dump(&scene, id), before);
}

#[test]
fn writes_land_in_place_and_an_age_past_lifetime_drops_the_particle() {
    let (scene, id) = super::identity::hidden(3);
    let hidden = |s: &std::rc::Rc<std::cell::RefCell<rusty::scene::Scene>>| {
        let s = s.borrow();
        let ps = &s.world.particles(id).unwrap().runtime.particles;
        ps.iter()
            .map(|p| (p.axis, p.turn, p.start_frame))
            .collect::<Vec<_>>()
    };
    let before = hidden(&scene);
    let live: u32 = lua(
        &scene,
        &format!(
            "local a = Particles.GetArrays({id}, {{ 'vy', 'age', 'lifetime', 'custom2' }})
             for i = 1, #a.vy do a.vy[i] = i; a.custom2[i] = 10 * i end
             a.age[2] = a.lifetime[2]; a.vy[3] = 0/0
             return Particles.SetArrays({id}, a)"
        ),
    )
    .unwrap();
    assert_eq!(live, 2, "the second particle is dead");
    let ps = scene
        .borrow()
        .world
        .particles(id)
        .unwrap()
        .runtime
        .particles
        .clone();
    assert_eq!((ps[0].velocity.y, ps[0].custom[1]), (1.0, 10.0));
    assert_eq!(
        ps[1].custom[1], 30.0,
        "the third moved up, its own values kept"
    );
    assert_ne!(ps[1].velocity.y, 3.0, "a NaN keeps the field");
    let after = hidden(&scene);
    assert_eq!((after[0], after[1]), (before[0], before[2]));
}

#[test]
fn a_bad_or_stale_write_is_an_error_that_changes_nothing() {
    for (code, says) in [
        (
            "Particles.GetArrays(ID, { 'speed' })",
            "speed is not a particle field",
        ),
        ("Particles.GetArrays(ID, 'vy')", "the fields are a string"),
        (
            "Particles.SetArrays(ID, { vy = { 1, 2 } })",
            "not a GetArrays table",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' }); a.vy[3] = nil
          Particles.SetArrays(ID, a)",
            "vy holds 2 entries, not the 3 read",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' }); a.vy[2] = '2'
          Particles.SetArrays(ID, a)",
            "vy[2] is a string, not a number",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' }); a.vy = 4
          Particles.SetArrays(ID, a)",
            "vy is a number, not an array",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' }); Particles.Emit(ID, 1)
          Particles.SetArrays(ID, a)",
            "from an older read",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' })
          local n = 0; Particles.ForEach(ID, function() n = n + 1; return n ~= 2 end)
          Particles.SetArrays(ID, a)",
            "from an older read",
        ),
        // Same count and the same ages after (every particle is born at age 0),
        // but particle 3 is gone and the last is new.
        (
            "local a = Particles.GetArrays(ID, { 'vy' })
          local n = 0; Particles.ForEach(ID, function() n = n + 1; return n ~= 3 end)
          Particles.Emit(ID, 1)
          Particles.SetArrays(ID, a)",
            "from an older read",
        ),
        (
            "local a = Particles.GetArrays(ID, { 'vy' })
          local ps, rev = Particles.GetParticles(ID), {}
          for i = #ps, 1, -1 do rev[#rev + 1] = ps[i] end
          Particles.SetParticles(ID, rev)
          Particles.SetArrays(ID, a)",
            "from an older read",
        ),
    ] {
        let (scene, id) = scene(3);
        let code = code.replace("ID", &id.to_string());
        let before = dump(&scene, id);
        let err = lua::<()>(&scene, &code).unwrap_err().to_string();
        assert!(err.contains(says), "{code}: {err}");
        if !["Emit", "ForEach", "SetParticles"]
            .iter()
            .any(|c| code.contains(c))
        {
            assert_eq!(dump(&scene, id), before, "{code}");
        }
    }
}

#[test]
fn no_emitter_reads_empty_arrays_and_writes_nothing() {
    let (scene, _) = scene(1);
    let bare = scene.borrow_mut().add_entity("Bare".to_string());
    let (n, set): (usize, u32) = lua(
        &scene,
        &format!(
            "local a = Particles.GetArrays({bare}, {{ 'x' }})
             return #a.x, Particles.SetArrays({bare}, a)"
        ),
    )
    .unwrap();
    assert_eq!((n, set), (0, 0));
    let err = lua::<()>(&scene, "Particles.SetArrays(nil, {})").unwrap_err();
    assert!(err.to_string().contains("SetArrays"), "{err}");
}
