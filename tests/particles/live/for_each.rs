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
               if p.custom1 == 0 and p.age == 0 then p.custom1 = p.size end
               p.vy = 4
               if p.size < 0 then return false end
             end)
             local n = Particles.ForEach({id}, function(p) return p.custom1 < 0.3 end)
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
    // Kills the smaller particles, so survivors move up the list.
    const RULE: &str = "local function rule(p)
      p.vx = p.vx * 0.5 + p.y
      p.custom2 = p.custom2 + 1; p.rotation = p.rotation + 10
      if p.size < 0.3 then return false end
    end";
    let (a, id) = super::identity::hidden(6);
    let (b, _) = super::identity::hidden(6);
    lua::<()>(&a, &format!("{RULE} Particles.ForEach({id}, rule)")).unwrap();
    lua::<()>(
        &b,
        &format!(
            "{RULE} local keep = {{}}
             for _, p in ipairs(Particles.GetParticles({id})) do
               if rule(p) ~= false then keep[#keep + 1] = p end
             end
             Particles.SetParticles({id}, keep)"
        ),
    )
    .unwrap();
    let live = a.borrow().world.particles(id).unwrap().live_count();
    assert!(
        (1..6).contains(&live),
        "the rule kills some, not all: {live}"
    );
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

#[test]
fn a_table_kept_past_its_call_is_an_error_not_another_particle() {
    let (scene, id) = scene(3);
    for (rule, name) in [
        ("function(p) kept = p end", "returned"),
        ("function(p) kept = p; error('stop') end", "errored"),
    ] {
        let before = dump(&scene, id);
        let first = lua::<()>(&scene, &format!("Particles.ForEach({id}, {rule})"));
        assert_eq!(first.is_err(), name == "errored", "{name}");
        assert_eq!(dump(&scene, id), before, "{name}: nothing changed");
        for touch in ["return kept.x", "kept.vy = 1"] {
            let err = lua::<()>(
                &scene,
                &format!("pcall(Particles.ForEach, {id}, {rule}) {touch}"),
            )
            .unwrap_err()
            .to_string();
            assert!(
                err.contains("used after its ForEach returned"),
                "{name}, {touch}: {err}"
            );
        }
    }
}

#[test]
fn a_view_given_a_protected_metatable_still_writes_its_edits() {
    let (scene, id) = scene(2);
    let live: u32 = lua(
        &scene,
        &format!(
            "return Particles.ForEach({id}, function(p)
               if not getmetatable(p) then setmetatable(p, {{ __metatable = 'locked' }}) end
               p.vy = 7
             end)"
        ),
    )
    .unwrap();
    assert_eq!(live, 2);
    let s = scene.borrow();
    for p in &s.world.particles(id).unwrap().runtime.particles {
        assert_eq!(p.velocity.y, 7.0);
    }
}
