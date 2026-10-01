//! `Trail`: settings and style through the API, recording in the play loop.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ParticleBlend, TrailComponent};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::authoring::{add_component, ComponentKind};
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

use super::{eval, round_trip, with_api};

#[test]
fn settings_and_style_write_through_and_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Rocket".to_string());
    assert!(add_component(
        &mut scene,
        id,
        ComponentKind::parse("TrailRenderer").unwrap()
    ));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Trail.SetEmitting({id}, false)
             Trail.SetTime({id}, -2)
             Trail.SetMinVertexDistance({id}, 0.25)
             Trail.SetWidth({id}, 0.5, 0)
             Trail.SetColors({id}, 4, 2, 0, 1,  0, 0, 1, 0)
             Trail.SetBlend({id}, 'additive')
             Trail.SetBlend({id}, 'glow')
             Trail.SetTextureMode({id}, 'Tile')
             Trail.SetTexture({id}, 'smoke.png')"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "return Trail.IsEmitting({id}), Trail.GetTime({id}), Trail.GetWidth({id}, 0.5),
                 Trail.GetBlend({id}), Trail.GetTextureMode({id}), Trail.GetTexture({id}),
                 Trail.IsEmitting(99), Trail.GetBlend(99)"
            ),
        );
        let want = r#"Boolean(false), Number(0), Number(0.25), String("Additive"), String("Tile"), String("smoke.png"), Boolean(false), Nil"#;
        assert_eq!(got, want);
        let colour = eval(lua, &format!("return Trail.GetColor({id}, 0.5)"));
        assert_eq!(colour, "Number(2), Number(1), Number(0.5), Number(0.5)");
    });
    let loaded = round_trip(&scene.borrow(), "trail");
    let t = loaded.world.trail(id).map(|t| t.clone()).unwrap();
    assert_eq!(t.style.blend, ParticleBlend::Additive);
    assert_eq!(Some(t), scene.borrow().world.trail(id).map(|t| t.clone()));
}

/// A play world whose one trail rides an entity swept along +X at 3 m/s.
fn record(frames: u32) -> Vec<Vec3> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Tracer".to_string());
    let trail = TrailComponent {
        time: 0.25,
        min_vertex_distance: 0.1,
        ..Default::default()
    };
    scene.world.set_trail(id, Some(trail));
    let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.set_playing(true);
    for frame in 0..frames {
        let x = frame as f32 * 3.0 / 60.0;
        let scene = game.scene().borrow_mut();
        let mut s = scene;
        s.world.transform_mut(id).unwrap().position = Vec3::new(x, 1.0, 0.0);
        drop(s);
        game.tick(1.0 / 60.0);
    }
    let scene = game.scene().borrow();
    let trail = scene.world.trail(id).unwrap();
    trail.positions().collect()
}

#[test]
fn the_play_loop_records_a_bounded_deterministic_trail() {
    let a = record(120);
    assert_eq!(a, record(120), "two runs record the identical trail");
    // 0.25 s at 3 m/s is 0.75 m of path, points ≥ 0.1 m apart plus the head.
    assert!((4..=10).contains(&a.len()), "recorded {} points", a.len());
    let span = a.last().unwrap().x - a.first().unwrap().x;
    assert!((0.6..=0.8).contains(&span), "trail spans {span} m");
    assert!(a.windows(2).all(|w| w[1].x > w[0].x), "oldest first");
}

#[test]
fn clear_and_the_point_readers() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Blade".to_string());
    let mut trail = TrailComponent::default();
    trail.advance(Vec3::ZERO, 0.01);
    trail.advance(Vec3::X, 0.01);
    scene.world.set_trail(id, Some(trail));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        let got = eval(
            lua,
            &format!(
                "local p = Trail.GetPositions({id}); return Trail.GetPositionCount({id}), p[2][1]"
            ),
        );
        assert_eq!(got, "Integer(2), Number(1)");
        lua.load(format!("Trail.Clear({id})")).exec().unwrap();
        let got = eval(
            lua,
            &format!("return Trail.GetPositionCount({id}), #Trail.GetPositions({id})"),
        );
        assert_eq!(got, "Integer(0), Integer(0)");
    });
    assert!(round_trip(&scene.borrow(), "trail_rt")
        .world
        .trail(id)
        .unwrap()
        .runtime
        .points
        .is_empty());
}
