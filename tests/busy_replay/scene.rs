//! Busy scene for the replay tests: dynamic boxes landing on each other, NavMeshAgents crossing, Lua driving timers, tweens, UI and the seeded `Random`, so an iteration order leaking into behaviour shows.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::dev::harness::Harness;
use rusty::scene::{NavMeshAgentComponent, Scene, ScriptComponent};

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

const BOX: &str = r#"
local B = {}
function B.OnCollisionEnter(id, other) print(string.format("[busy] hit %d %d", id, other)) end
return B
"#;

/// Relative to the harness workspace root, as a scene stores script paths, so two scratch directories snapshot the same paths.
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
            shape,
            ..rusty::edit::authoring::default_collider()
        }),
    );
    scene.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            angular_velocity: Vec3::new(0.3, 0.0, 0.2),
            ..rusty::edit::authoring::default_rigidbody()
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

pub fn run(dir: &str, frames: u32) -> String {
    let out = crate::temp::dir().join(dir);
    let h = Harness::new(&out, DIRECTOR_PATH);
    let workspace = Harness::workspace_of(&out);
    write(&workspace, DIRECTOR_PATH, DIRECTOR);
    write(&workspace, BOX_PATH, BOX);
    {
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
