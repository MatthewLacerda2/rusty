//! Root motion and who else moves the entity (#1745): an agent whose animator
//! can't drive it moves itself (an unknown bone warns once), an off-mesh link
//! decides where the clip's travel goes, scripts read the delta, and an
//! inactive entity reports none.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::tests::{add_box, lunging, play};
use crate::app::game::game_tests::{world_with, DT};
use crate::components::{NavMeshAgentComponent, OffMeshLinkData, OffMeshLinkKind};
use crate::scene::Scene;

/// A lunging guard on a floor, with an agent heading 8 m the other way.
fn agent_scene(link: Option<bool>) -> (Scene, u32) {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::Y * -0.5, Vec3::new(20.0, 1.0, 20.0));
    let guard = lunging(&mut scene);
    let mut agent = NavMeshAgentComponent {
        active: true,
        radius: 0.3,
        speed: 5.0,
        acceleration: 50.0,
        target: Vec3::new(-8.0, 0.0, 0.0),
        update_rotation: false,
        ..NavMeshAgentComponent::default()
    };
    if let Some(auto) = link {
        let end = Vec3::new(0.0, 0.0, -10.0);
        let kind = OffMeshLinkKind::Jump;
        let (start, owner) = (Vec3::ZERO, None);
        agent.off_mesh_link = Some(OffMeshLinkData {
            kind,
            start,
            end,
            owner,
        });
        agent.auto_traverse_off_mesh_link = auto;
        agent.speed = 1.0;
    }
    scene.world.set_nav_agent(guard, Some(agent));
    (scene, guard)
}

#[test]
fn an_agent_whose_animator_cannot_drive_it_moves_itself() {
    let (mut stopped, guard) = agent_scene(None);
    stopped.world.animator_mut(guard).unwrap().is_playing = false;
    let (end, _) = play(stopped, guard)[89];
    assert!(end.x < -3.0, "a stopped animator: the agent walks: {end}");

    let (mut misspelled, guard) = agent_scene(None);
    misspelled.world.animator_mut(guard).unwrap().root_bone = "hipz".into();
    let scene = Rc::new(RefCell::new(misspelled));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    (0..90).for_each(|_| {
        gw.tick(DT);
    });
    let end = scene.borrow().world.world_position(guard);
    assert!(end.x < -3.0, "an unknown bone: the agent walks: {end}");
    let console = gw.console().borrow();
    let warned = console
        .messages
        .iter()
        .filter(|(m, _)| m.contains("'hipz'"));
    assert_eq!(warned.count(), 1, "warned once");
}

#[test]
fn an_auto_traversed_link_carries_the_agent_and_drops_the_clip() {
    let (scene, guard) = agent_scene(Some(true));
    let (end, _) = play(scene, guard)[59];
    assert!(end.x.abs() < 1e-3, "no lunge on the link: {end}");
    assert!((end.z + 1.0).abs() < 0.05, "the link's own 1 m/s: {end}");
}

#[test]
fn a_scripted_link_takes_the_clip_on_the_transform() {
    let (scene, guard) = agent_scene(Some(false));
    let (end, hips) = play(scene, guard)[89];
    assert!((end.x - 2.0).abs() < 1e-3, "the vault's travel: {end}");
    assert!((hips.x - end.x).abs() < 1e-4, "the hips stay with the body");
}

#[test]
fn a_root_moved_agent_records_what_it_really_moved() {
    let (scene, guard) = agent_scene(None);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    (0..30).for_each(|_| {
        gw.tick(DT);
    });
    let moved = scene
        .borrow()
        .world
        .animator(guard)
        .unwrap()
        .root_motion
        .moved;
    assert!((moved.x - 2.0 * DT).abs() < 1e-3, "{moved}");
}

#[test]
fn an_inactive_entity_reports_no_root_motion() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
    scene.borrow_mut().world.set_active(guard, false);
    gw.tick(DT);
    let eval = |line: &str| gw.script_manager().eval(line).unwrap();
    let dx: f32 = eval(&format!("return (Animator.GetRootMotion({guard}))"))
        .parse()
        .unwrap();
    assert_eq!(dx, 0.0, "nothing measured while inactive");
}

#[test]
fn scripts_read_the_delta_and_turn_root_motion_off() {
    let mut scene = Scene::new();
    let guard = lunging(&mut scene);
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
    let eval = |gw: &crate::app::GameWorld, line: &str| gw.script_manager().eval(line).unwrap();
    let dx: f32 = eval(&gw, &format!("return (Animator.GetRootMotion({guard}))"))
        .parse()
        .unwrap();
    assert!((dx - 2.0 * DT).abs() < 1e-4, "{dx}");
    eval(&gw, &format!("Animator.SetApplyRootMotion({guard}, false)"));
    let before = scene.borrow().world.world_position(guard);
    gw.tick(DT);
    assert_eq!(
        scene.borrow().world.world_position(guard),
        before,
        "off: it stays"
    );
    let still: f32 = eval(&gw, &format!("return (Animator.GetRootMotion({guard}))"))
        .parse()
        .unwrap();
    assert!(still > 0.0, "still measured while off");
}
