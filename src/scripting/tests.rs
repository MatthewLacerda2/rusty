use std::cell::RefCell;
use std::rc::Rc;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;
use glam::Vec3;

fn manager() -> (ScriptManager, Rc<RefCell<Scene>>, Rc<RefCell<Camera>>) {
    let mut raw = Scene::new();
    raw.add_entity("Target".to_string());
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    time.borrow_mut().advance(0.25);
    let mut m = ScriptManager::new(
        Rc::clone(&scene),
        input,
        nav,
        console,
        Rc::clone(&camera),
        time,
    );
    // No live physics world in these unit tests, so Physics.Raycast misses.
    let physics = Rc::new(RefCell::new(None));
    m.init_runtime(&physics).expect("runtime inits");
    (m, scene, camera)
}

#[test]
fn time_namespace_reads_clock() {
    let (m, _scene, _cam) = manager();
    m.exec("assert(Time.deltaTime() == 0.25)").unwrap();
    m.exec("assert(Time.fixedDeltaTime() > 0)").unwrap();
    m.exec("assert(Time.frameCount() == 1)").unwrap();
}

#[test]
fn camera_set_moves_shared_camera() {
    let (m, _scene, cam) = manager();
    m.exec("Camera.SetPosition(1, 2, 3); Camera.SetFov(60)")
        .unwrap();
    assert_eq!(cam.borrow().position, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(cam.borrow().fov, 60.0);
}

#[test]
fn input_press_release_is_writable() {
    let (m, _scene, _cam) = manager();
    m.exec("Input.Press('W'); assert(Input.IsKeyDown('W'))")
        .unwrap();
    m.exec("Input.Release('W'); assert(not Input.IsKeyDown('W'))")
        .unwrap();
}

#[test]
fn storage_roundtrips_scalars_and_tables() {
    let (m, _scene, _cam) = manager();
    // Scalar set/get + Has/Delete.
    m.exec("Storage.Set('audio', 'volume', 0.5)").unwrap();
    m.exec("assert(Storage.Get('audio', 'volume') == 0.5)")
        .unwrap();
    m.exec("assert(Storage.Has('audio', 'volume'))").unwrap();
    m.exec("assert(Storage.Delete('audio', 'volume'))").unwrap();
    m.exec("assert(Storage.Get('audio', 'volume') == nil)")
        .unwrap();
    // Structured whole-namespace blob via SetTable/GetTable.
    m.exec("Storage.SetTable('binds', { jump = 'Space', n = 3 })")
        .unwrap();
    m.exec("local b = Storage.GetTable('binds'); assert(b.jump == 'Space' and b.n == 3)")
        .unwrap();
}

#[test]
fn assets_manifest_returns_a_table() {
    let (m, _scene, _cam) = manager();
    // The manifest is always a (possibly empty) array table, and `List` aliases it.
    m.exec("assert(type(Assets.Manifest()) == 'table')")
        .unwrap();
    m.exec("assert(type(Assets.List()) == 'table')").unwrap();
}

#[test]
fn raycast_misses_into_empty_space() {
    let (m, _scene, _cam) = manager();
    // Target has no collider, so nothing to hit.
    m.exec("local hit = Physics.Raycast(0,0,0, 1,0,0); assert(hit == false)")
        .unwrap();
}

#[test]
fn add_force_applies_dt_scaled_velocity() {
    let (m, scene, _cam) = manager();
    // Give entity 1 a dynamic rigidbody so AddForce takes effect.
    {
        let mut s = scene.borrow_mut();
        let mut e = s.get_entity_mut(1).unwrap();
        e.rigidbody = Some(crate::components::RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 2.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: crate::components::CollisionDetection::Discrete,
        });
    }
    // AddForce is a continuous force (Unity ForceMode.Force): Δv = F/m · dt.
    // F=(20,0,0), m=2 ⇒ a=10; over one fixed step (1/60s) ⇒ Δvx = 10/60.
    m.exec("Physics.AddForce(1, 20, 0, 0)").unwrap();
    let vx = scene
        .borrow()
        .get_entity(1)
        .and_then(|e| e.rigidbody.as_ref().map(|r| r.velocity.x))
        .unwrap();
    let expected = 10.0 * crate::time::FIXED_DELTA_TIME;
    assert!(
        (vx - expected).abs() < 1e-6,
        "AddForce should scale by dt: got {vx}, want {expected}"
    );
}
