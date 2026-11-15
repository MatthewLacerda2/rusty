//! A field of the wrong type is an error naming the call, the entry and the
//! field, raised before anything is written; `ForEach` checks its `fn`.

use super::{dump, lua, scene};

#[test]
fn a_field_that_is_not_a_number_is_an_error_that_changes_nothing() {
    for (edit, names) in [
        ("ps[2].age = false", "entry 2's age is a boolean"),
        ("ps[1].x = 'up'", "entry 1's x is a string"),
        ("ps[2].size = '2'", "entry 2's size is a string"),
        ("ps[1].r = {}", "entry 1's r is a table"),
        ("ps[2].custom3 = true", "entry 2's custom3 is a boolean"),
    ] {
        let (scene, id) = scene(2);
        let before = dump(&scene, id);
        let err = lua::<()>(
            &scene,
            &format!(
                "local ps = Particles.GetParticles({id})
                 {edit}
                 Particles.SetParticles({id}, ps)"
            ),
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("Particles.SetParticles: ") && err.contains(names),
            "{edit}: {err}"
        );
        assert!(
            !err.lines().next().unwrap().contains("particles_live"),
            "the error points at the caller: {err}"
        );
        assert_eq!(dump(&scene, id), before, "{edit}");
    }
}

#[test]
fn for_each_names_a_bad_field_and_a_missing_fn() {
    let (scene, id) = scene(2);
    let before = dump(&scene, id);
    let err = lua::<()>(
        &scene,
        &format!("Particles.ForEach({id}, function(p) p.lifetime = 'long' end)"),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("Particles.ForEach: entry 1's lifetime is a string"),
        "{err}"
    );
    assert_eq!(dump(&scene, id), before);
    // A table answering `<` passes the fast check; the conversion still names it.
    let err = lua::<()>(
        &scene,
        &format!(
            "local lt = setmetatable({{}}, {{ __lt = function() return true end }})
             Particles.ForEach({id}, function(p) if p.vy then p.vy = lt end end)"
        ),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("Particles.ForEach: entry 1's vy is a table"),
        "{err}"
    );
    assert_eq!(dump(&scene, id), before);
    let bare = scene.borrow_mut().add_entity("Bare".to_string());
    for target in [id, bare] {
        let err = lua::<()>(&scene, &format!("Particles.ForEach({target}, nil)")).unwrap_err();
        assert!(
            err.to_string()
                .contains("Particles.ForEach: fn is a nil, not a function"),
            "{err}"
        );
    }
}
