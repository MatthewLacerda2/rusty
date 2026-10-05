//! The busy scene the replay tests step (#764): a pile of dynamic boxes that land on
//! each other, NavMeshAgents crossing paths, and Lua driving timers, tweens, a UI
//! layout and the seeded `Random` — every sim module that keeps a hash map gets
//! work to do, so an iteration order that leaks into behaviour shows in the result.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::dev::harness::Harness;
use rusty::scene::{NavMeshAgentComponent, Scene, ScriptComponent};

/// The director: UI, timers, tweens and RNG, on the default scene's Enemy_1.
const DIRECTOR: &str = r#"
local D = {}
function D.Start(id)
    local canvas = UI.Create("Canvas")
    local panel = UI.Create("Panel", canvas)
    local buttons = {}
    for i = 1, 4 do buttons[i] = UI.Create("Button", panel) end
    Tween.To(panel, "RectTransform.size_delta", {300, 120}, 0.5, { ease = "bounce_out", loops = -1, yoyo = true })
    Tween.To(id, "Transform.scale", {1.6, 2.4, 1.6}, 0.4, { ease = "elastic_out", loops = -1, yoyo = true })
    Timer.InvokeRepeating(id, function()
        local r = UI.GetRect(buttons[2])
        local w = r and r.width or -1
        print(string.format("[busy] t %.4f %d %.3f", w, math.random(1, 1000), math.random()))
    end, 0.1, 0.25)
end
return D
"#;

/// Every box reports the contacts it starts, so the order events fire is observed.
const BOX: &str = r#"
local B = {}
function B.OnCollisionEnter(id, other) print(string.format("[busy] hit %d %d", id, other)) end
return B
"#;

/// Where the busy scripts sit, relative to the harness's workspace root — the form
/// a scene stores a script path in on every OS (#783), so two runs in two scratch
/// directories snapshot the same paths.
const DIRECTOR_PATH: &str = "assets/scripts/busy_director.lua";
const BOX_PATH: &str = "assets/scripts/busy_box.lua";

fn write(workspace: &std::path::Path, path: &str, body: &str) {
    std::fs::write(workspace.join(path), body).expect("write script");
}

fn add_box(scene: &mut Scene, pos: Vec3, script: &str) {
    let id = scene.add_entity("Crate".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let shape = ColliderShape::Box {
        size: Vec3::splat(0.8),
    };
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape,
            is_trigger: false,
            material: Default::default(),
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    scene.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::new(0.3, 0.0, 0.2),
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        }),
    );
    scene.world.scripts_mut(id).unwrap().push(ScriptComponent {
        path: script.to_string(),
        ..Default::default()
    });
}

fn add_agent(scene: &mut Scene, from: Vec3, to: Vec3) {
    let id = scene.add_entity("Walker".to_string());
    scene.world.transform_mut(id).unwrap().position = from;
    let agent = NavMeshAgentComponent {
        active: true,
        radius: 0.4,
        target: to,
        speed: 3.0,
        acceleration: 8.0,
        stopping_distance: 0.2,
        ..Default::default()
    };
    scene.world.set_nav_agent(id, Some(agent));
}

/// The harness on the default scene plus the busy additions, stepped `frames` ticks.
/// Returns the world snapshot and the `[busy]` console lines, as one string.
pub fn run(dir: &str, frames: u32) -> String {
    let out = crate::temp::dir().join(dir);
    let h = Harness::new(&out, DIRECTOR_PATH);
    let workspace = Harness::workspace_of(&out);
    write(&workspace, DIRECTOR_PATH, DIRECTOR);
    write(&workspace, BOX_PATH, BOX);
    {
        // Play is entered on the first tick, so these are in the scene it snapshots.
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        for i in 0..16 {
            let (x, z) = ((i % 4) as f32 * 0.7 - 1.0, (i / 4) as f32 * 0.7 - 1.0);
            add_box(&mut scene, Vec3::new(x, 2.0 + i as f32 * 0.45, z), BOX_PATH);
        }
        for i in 0..6 {
            let s = i as f32 * 1.5 - 4.0;
            add_agent(
                &mut scene,
                Vec3::new(-10.0, 0.0, s),
                Vec3::new(10.0, 0.0, -s),
            );
        }
    }
    h.step(frames);
    let console = h.console.borrow();
    let lines: Vec<&str> = console
        .messages
        .iter()
        .map(|m| m.0.as_str())
        .filter(|m| m.contains("[busy]") || m.contains("Error"))
        .collect();
    format!("{}\n{}", h.snapshot(), lines.join("\n"))
}
