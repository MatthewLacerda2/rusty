//! Graph v2 through the real tick (#457): a 1D locomotion tree on the base layer
//! and an upper-body layer masked to the spine pose two bone GameObjects — the
//! legs follow the tree, the spine the layer — deterministically.

use glam::{Mat4, Vec3};
use rusty::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use rusty::asset::animation_graph::{self, AnimationGraph};
use rusty::asset::mesh_data::{JointTransform, SkinData};
use rusty::components::{AnimatorComponent, MeshComponent};
use rusty::scene::{DirtyFlag, Scene};

use super::rig::play;

const GRAPH: &str = r#"{
  "parameters": { "speed": { "Float": 0.0 } },
  "nodes": [ { "name": "Loco", "is_loop": true, "blend_tree": { "Simple1D": {
    "parameter": "speed", "children": [ { "clip": "Walk", "threshold": 2.0 },
                                        { "clip": "Run", "threshold": 6.0 } ] } } } ],
  "entry": "Loco",
  "layers": [ { "name": "Upper", "mask": ["spine"],
    "nodes": [ { "name": "Aim", "clip": "Aim", "is_loop": true } ], "entry": "Aim" } ]
}"#;

/// Both joints slide along +X from `from` to `to` over `duration` seconds.
fn slide(name: &str, from: f32, to: f32, duration: f32) -> AnimationClip {
    let track = Track {
        times: vec![0.0, duration],
        values: vec![Vec3::X * from, Vec3::X * to],
        interpolation: Interpolation::Linear,
    };
    let joint = JointTrack {
        translation: track,
        ..JointTrack::default()
    };
    let mut clip = AnimationClip {
        name: name.to_string(),
        tracks: vec![joint.clone(), joint],
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

/// A two-bone (`spine`, `leg`) character running the layered graph at speed 4.
fn soldier() -> (Scene, u32) {
    let graph: AnimationGraph = serde_json::from_str(GRAPH).unwrap();
    let path = crate::temp::dir().join("rusty_457_runtime.animgraph");
    animation_graph::save(&path, &graph).unwrap();
    let mut scene = Scene::new();
    let hero = scene.add_entity("Soldier".to_string());
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY; 2],
        bind_global: vec![Mat4::IDENTITY; 2],
        local_bind: vec![JointTransform::default(); 2],
        parents: vec![None, None],
        joint_nodes: vec![0, 1],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["spine".to_string(), "leg".to_string()],
    };
    let clips = vec![
        slide("Walk", 0.0, 2.0, 1.0),
        slide("Run", 0.0, 6.0, 2.0),
        slide("Aim", -1.0, -1.0, 1.0),
    ];
    scene.world.set_mesh(
        hero,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: Some("soldier.glb::Body".to_string()),
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_palette: vec![Mat4::IDENTITY; 2],
            skin: Some(skin),
            clips,
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    let mut anim = AnimatorComponent {
        graph: Some(path.to_string_lossy().replace('\\', "/")),
        ..Default::default()
    };
    anim.set_float("speed", 4.0);
    scene.world.set_animator(hero, Some(anim));
    scene.sync_skeleton(hero);
    (scene, hero)
}

/// The (spine, leg) bones' x after `ticks` fixed steps.
fn bones_after(ticks: usize) -> (f32, f32) {
    let (scene, hero) = soldier();
    let (spine, leg) = (
        scene.find_bone(hero, "spine").unwrap(),
        scene.find_bone(hero, "leg").unwrap(),
    );
    let scene = play(scene, ticks);
    let s = scene.borrow();
    let x = |bone| s.world.transform(bone).unwrap().position.x;
    (x(spine), x(leg))
}

#[test]
fn the_legs_follow_the_tree_and_the_spine_follows_the_masked_layer() {
    // 45 steps = 0.75 s; half Walk (1 s), half Run (2 s) cycles every 1.5 s, so
    // the phase is 0.5: Walk at x = 1, Run at x = 3, blended to 2.
    let (spine, leg) = bones_after(45);
    assert!((leg - 2.0).abs() < 1e-3, "leg at {leg}");
    assert!((spine + 1.0).abs() < 1e-6, "spine at {spine}");
}

#[test]
fn layered_posing_is_deterministic_across_runs() {
    let (a, b) = (bones_after(37), bones_after(37));
    assert_eq!(
        (a.0.to_bits(), a.1.to_bits()),
        (b.0.to_bits(), b.1.to_bits())
    );
}
