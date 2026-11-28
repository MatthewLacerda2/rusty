//! The windowed loop draws a body at any speed blended, and a stated teleport
//! (`Transform.Teleport`, run by a script inside the tick) where it landed.

use glam::Vec3;

use super::frame_tests::playing_world;
use super::*;
use rusty_core::paths;
use rusty_edit::authoring::{create_entity, default_collider, default_rigidbody, Primitive};
use rusty_scene::ScriptComponent;

/// A script that hops its entity 50 m with `Transform.Teleport` on its tenth `Update`.
fn hopper() -> ScriptComponent {
    let dir = rusty_test_support::temp::dir().join("rusty_2508_teleport");
    std::fs::create_dir_all(&dir).unwrap();
    let lua = dir.join("hop.lua");
    let code = "local n = 0\nreturn { Update = function(id)\n\
        n = n + 1\n\
        if n == 10 then local x, y, z = Transform.GetPosition(id)\n\
        Transform.Teleport(id, x + 50, y, z) end end }";
    std::fs::write(&lua, code).unwrap();
    ScriptComponent {
        path: paths::to_slash(&lua),
        ..Default::default()
    }
}

/// A missile at 400 m/s under physics with a fin parented to it, about to enter Play.
fn missile() -> (GameWorld, [u32; 2]) {
    let game = playing_world();
    let ids = {
        let mut scene = game.scene().borrow_mut();
        let body = create_entity(&mut scene, "Missile", Some(Primitive::Box));
        let fin = create_entity(&mut scene, "Fin", Some(Primitive::Box));
        scene.set_parent(fin, Some(body)).unwrap();
        scene.world.transform_mut(body).unwrap().position = Vec3::new(0.0, 50.0, 0.0);
        let rb = rusty_components::RigidBodyComponent {
            velocity: Vec3::new(400.0, 0.0, 0.0),
            use_gravity: false,
            drag: 0.0,
            ..default_rigidbody()
        };
        scene.world.set_rigidbody(body, Some(rb));
        scene.world.set_collider(body, Some(default_collider()));
        *scene.world.scripts_mut(body).unwrap() = vec![hopper()];
        [body, fin]
    };
    (game, ids)
}

#[test]
fn a_400_m_s_body_blends_and_its_scripted_teleport_is_a_cut() {
    let (mut game, [body, fin]) = missile();
    let (mut ticks, mut poses) = Default::default();
    let x = |g: &GameWorld, id| g.scene().borrow().world.world_position(id).x;
    let (mut blended, mut cut) = (0, false);
    for _ in 0..60 {
        let (frames, before) = (game.play_frame(), x(&game, body));
        advance_sim(&mut game, Step::of(&mut ticks, &mut poses), 1.0 / 144.0);
        if game.play_frame() != frames + 1 || ticks.alpha() <= 0.0 || frames < 3 {
            continue;
        }
        let scene = game.scene().borrow();
        let after = scene.world.world_position(body).x;
        let drawn = |id| poses.world(&scene, id).w_axis.x;
        if after - before > 20.0 {
            // The tick that ran the hop: no in-between frame, the fin with it.
            assert_eq!(drawn(body), after, "drawn where it landed");
            assert_eq!(drawn(fin), scene.world.world_position(fin).x);
            cut = true;
            continue;
        }
        assert!(
            after - before > 6.0,
            "physics moved it {} m",
            after - before
        );
        assert!(
            drawn(body) > before && drawn(body) < after,
            "{before} < {} < {after}",
            drawn(body)
        );
        let offset = scene.world.world_position(fin).x - after;
        assert!(
            (drawn(fin) - drawn(body) - offset).abs() < 1e-2,
            "the fin rides along"
        );
        blended += 1;
    }
    assert!(cut && blended > 5, "cut {cut}, {blended} blended frames");
    assert!(
        x(&game, body) > 400.0 * 0.3,
        "kept its speed through the warp"
    );
}
