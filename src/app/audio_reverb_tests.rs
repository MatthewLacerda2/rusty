//! Reverb zones on a ticking `GameWorld` (#469): the `LateUpdate` system blends the
//! zones around the camera, follows a parented zone's world position, skips an
//! inactive zone, and reads back the same trace on two headless runs. Primitive
//! entities only (a zone needs no mesh).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::audio::ReverbState;
use crate::components::{ReverbPreset, ReverbZoneComponent};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::authoring::reverb_zone as ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// A zone of `preset` at `at`, radii 4 / 8.
fn zone(scene: &mut Scene, at: Vec3, preset: ReverbPreset) -> u32 {
    let id = scene.add_entity(format!("{preset:?}"));
    scene.world.transform_mut(id).unwrap().position = at;
    let mut z = ReverbZoneComponent::default();
    ops::set_preset(&mut z, preset);
    ops::set_min_distance(&mut z, 4.0);
    ops::set_max_distance(&mut z, 8.0);
    scene.world.set_reverb_zone(id, Some(z));
    id
}

/// A tunnel zone at x = 0 and a room zone at x = 10; returns the world and the
/// tunnel's id.
fn level() -> (GameWorld, u32) {
    let mut scene = Scene::new();
    let tunnel = zone(&mut scene, Vec3::ZERO, ReverbPreset::Tunnel);
    zone(&mut scene, Vec3::new(10.0, 0.0, 0.0), ReverbPreset::Room);
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0);
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = GameWorld::new(scene, input, Rc::new(RefCell::new(nav)), console);
    world.set_playing(true);
    (world, tunnel)
}

/// Put the camera at `x` and tick once; the reverb the listener hears there.
fn hear_at(world: &mut GameWorld, x: f32) -> ReverbState {
    world.resources.camera.borrow_mut().position = Vec3::new(x, 0.0, 0.0);
    world.tick(DT);
    world.resources.audio.borrow().reverb_state()
}

#[test]
fn walking_from_the_tunnel_into_the_room_blends_the_two() {
    let (mut world, _) = level();
    let tunnel = ReverbPreset::Tunnel.params().unwrap();
    let room = ReverbPreset::Room.params().unwrap();
    assert_eq!(hear_at(&mut world, 0.0).params, tunnel);
    let door = hear_at(&mut world, 5.0);
    // 5 m from the tunnel (weight ¾) and from the room (¾): an even blend.
    assert_eq!(door.zones, 2);
    let mid = (tunnel.decay_time + room.decay_time) / 2.0;
    assert!((door.params.decay_time - mid).abs() < 1e-5, "{door:?}");
    assert_eq!(hear_at(&mut world, 10.0).params, room);
    assert_eq!(hear_at(&mut world, 30.0), ReverbState::DRY);
}

#[test]
fn a_zone_counts_at_its_world_position_and_only_while_active() {
    let (mut world, tunnel) = level();
    {
        let scene = world.scene();
        let mut scene = scene.borrow_mut();
        let pivot = scene.add_entity("Pivot".to_string());
        scene.world.transform_mut(pivot).unwrap().position = Vec3::new(0.0, 0.0, 50.0);
        scene.world.set_parent_id(tunnel, Some(pivot));
    }
    assert_eq!(
        hear_at(&mut world, 0.0),
        ReverbState::DRY,
        "moved with its parent"
    );
    let tunnel_params = ReverbPreset::Tunnel.params().unwrap();
    let state = {
        world.resources.camera.borrow_mut().position = Vec3::new(0.0, 0.0, 50.0);
        world.tick(DT);
        world.resources.audio.borrow().reverb_state()
    };
    assert_eq!(state.params, tunnel_params);
    world.scene().borrow_mut().world.set_active(tunnel, false);
    world.tick(DT);
    assert_eq!(
        world.resources.audio.borrow().reverb_state(),
        ReverbState::DRY
    );
}

#[test]
fn the_reverb_trace_is_identical_across_two_headless_runs() {
    let trace = || {
        let (mut world, _) = level();
        (0..60)
            .map(|i| hear_at(&mut world, i as f32 * 0.25))
            .collect::<Vec<_>>()
    };
    assert_eq!(trace(), trace());
}
