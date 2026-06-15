use super::console::ConsoleLogs;
use super::manager::{ScriptCtx, ScriptManager};
use crate::components::HealthComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;
use glam::Vec3;

/// Owned engine state a script test borrows into a [`ScriptCtx`] per call.
struct Env {
    scene: Scene,
    input: InputState,
    nav: NavigationGraph,
    console: ConsoleLogs,
    camera: Camera,
    time: Time,
    physics: Option<PhysicsWorld>,
}

impl Env {
    fn ctx(&mut self) -> ScriptCtx<'_> {
        ScriptCtx {
            scene: &mut self.scene,
            input: &mut self.input,
            nav: &mut self.nav,
            console: &mut self.console,
            camera: &mut self.camera,
            time: &mut self.time,
            physics: &mut self.physics,
        }
    }
}

fn manager() -> (ScriptManager, Env) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Target".to_string());
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.health = Some(HealthComponent {
            current_health: 100.0,
            max_health: 100.0,
            is_dead: false,
        });
    }
    let mut time = Time::new();
    time.advance(0.25);
    let env = Env {
        scene,
        input: InputState::new(),
        nav: NavigationGraph::new(-10.0, 10.0, -10.0, 10.0, 1.0),
        console: ConsoleLogs::new(),
        camera: Camera::new(Vec3::ZERO, 0.0, 0.0),
        time,
        // No live physics world in these unit tests, so Physics.Raycast/Shoot miss.
        physics: None,
    };
    let mut m = ScriptManager::new();
    m.init_runtime().expect("runtime inits");
    (m, env)
}

#[test]
fn health_get_damage_heal_roundtrip() {
    let (m, mut env) = manager();
    m.exec(&mut env.ctx(), "Health.Damage(1, 30)").unwrap();
    m.exec(&mut env.ctx(), "Health.Heal(1, 5)").unwrap();
    let hp = env
        .scene
        .get_entity(1)
        .and_then(|e| e.health.as_ref().map(|h| h.current_health));
    assert_eq!(hp, Some(75.0));
}

#[test]
fn time_namespace_reads_clock() {
    let (m, mut env) = manager();
    m.exec(&mut env.ctx(), "assert(Time.deltaTime() == 0.25)")
        .unwrap();
    m.exec(&mut env.ctx(), "assert(Time.fixedDeltaTime() > 0)")
        .unwrap();
    m.exec(&mut env.ctx(), "assert(Time.frameCount() == 1)")
        .unwrap();
}

#[test]
fn camera_set_moves_shared_camera() {
    let (m, mut env) = manager();
    m.exec(
        &mut env.ctx(),
        "Camera.SetPosition(1, 2, 3); Camera.SetFov(60)",
    )
    .unwrap();
    assert_eq!(env.camera.position, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(env.camera.fov, 60.0);
}

#[test]
fn input_press_release_is_writable() {
    let (m, mut env) = manager();
    m.exec(
        &mut env.ctx(),
        "Input.Press('W'); assert(Input.IsKeyDown('W'))",
    )
    .unwrap();
    m.exec(
        &mut env.ctx(),
        "Input.Release('W'); assert(not Input.IsKeyDown('W'))",
    )
    .unwrap();
}

#[test]
fn raycast_misses_into_empty_space() {
    let (m, mut env) = manager();
    // Target has no collider, so nothing to hit.
    m.exec(
        &mut env.ctx(),
        "local hit = Physics.Raycast(0,0,0, 1,0,0); assert(hit == false)",
    )
    .unwrap();
}
