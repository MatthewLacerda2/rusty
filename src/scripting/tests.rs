use std::cell::RefCell;
use std::rc::Rc;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;
use crate::components::HealthComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;
use glam::Vec3;

fn manager() -> (ScriptManager, Rc<RefCell<Scene>>, Rc<RefCell<Camera>>) {
    let mut raw = Scene::new();
    let id = raw.add_entity("Target".to_string());
    if let Some(mut e) = raw.get_entity_mut(id) {
        e.health = Some(HealthComponent {
            current_health: 100.0,
            max_health: 100.0,
            is_dead: false,
        });
    }
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
    // No live physics world in these unit tests, so Physics.Raycast/Shoot miss.
    let physics = Rc::new(RefCell::new(None));
    m.init_runtime(&physics).expect("runtime inits");
    (m, scene, camera)
}

#[test]
fn health_get_damage_heal_roundtrip() {
    let (m, scene, _cam) = manager();
    m.exec("Health.Damage(1, 30)").unwrap();
    m.exec("Health.Heal(1, 5)").unwrap();
    let hp = scene
        .borrow()
        .get_entity(1)
        .and_then(|e| e.health.as_ref().map(|h| h.current_health));
    assert_eq!(hp, Some(75.0));
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
fn raycast_misses_into_empty_space() {
    let (m, _scene, _cam) = manager();
    // Target has no collider, so nothing to hit.
    m.exec("local hit = Physics.Raycast(0,0,0, 1,0,0); assert(hit == false)")
        .unwrap();
}
