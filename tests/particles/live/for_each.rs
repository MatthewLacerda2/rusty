//! `Particles.ForEach`: one reused table per live particle, its edits written
//! back, `false` killing the particle, and the same rule giving the same state as
//! the `GetParticles` / `SetParticles` pair.

use super::{dump, lua, scene};

#[test]
fn edits_are_written_back_and_false_kills() {
    let (scene, id) = scene(4);
    let (live, same): (u32, bool) = lua(
        &scene,
        &format!(
            "local seen
             local live = Particles.ForEach({id}, function(p)
               seen = seen == nil and p or (seen == p and seen or false)
               if p.custom[1] == 0 and p.age == 0 then p.custom[1] = p.size end
               p.velocity.y = 4
               if p.size < 0 then return false end
             end)
             local n = Particles.ForEach({id}, function(p) return p.custom[1] < 0.3 end)
             return n, seen ~= false"
        ),
    )
    .unwrap();
    assert!(same, "one table is reused for every particle");
    let s = scene.borrow();
    let e = s.world.particles(id).unwrap();
    assert_eq!(live as usize, e.live_count());
    assert!(e.live_count() < 4, "some start sizes are at least 0.3");
    for p in &e.runtime.particles {
        assert_eq!((p.velocity.y, p.custom[0]), (4.0, p.size));
        assert!(p.size < 0.3);
    }
}

#[test]
fn for_each_and_the_pair_give_the_same_state() {
    const RULE: &str = "p.velocity.x = p.velocity.x * 0.5 + p.position.y
      p.custom[2] = p.custom[2] + 1; p.rotation = p.rotation + 10";
    let (a, id) = scene(5);
    let (b, _) = scene(5);
    lua::<()>(
        &a,
        &format!("Particles.ForEach({id}, function(p) {RULE} end)"),
    )
    .unwrap();
    lua::<()>(
        &b,
        &format!(
            "local ps = Particles.GetParticles({id})
             for _, p in ipairs(ps) do {RULE} end
             Particles.SetParticles({id}, ps)"
        ),
    )
    .unwrap();
    assert_eq!(dump(&a, id), dump(&b, id));
}

#[test]
fn a_nil_id_is_an_error_and_no_emitter_visits_nothing() {
    let (scene, _) = scene(1);
    let bare = scene.borrow_mut().add_entity("Bare".to_string());
    let err = lua::<()>(&scene, "Particles.ForEach(nil, function() end)").unwrap_err();
    assert!(err.to_string().contains("ForEach: no entity"), "{err}");
    let err = lua::<()>(&scene, "Particles.SetParticles(nil, {})").unwrap_err();
    assert!(err.to_string().contains("SetParticles: no entity"), "{err}");
    let n: u32 = lua(
        &scene,
        &format!("return Particles.ForEach({bare}, function() error('called') end)"),
    )
    .unwrap();
    assert_eq!(n, 0);
}
