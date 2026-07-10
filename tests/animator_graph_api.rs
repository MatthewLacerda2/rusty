//! The `Animator` graph control surface (#316) end-to-end through Lua: `SetGraph`
//! assigns/clears the asset path, `SetGraphEnabled` toggles auto-evaluation,
//! `PlayNode` jumps to a named state bypassing conditions, `PlayAnimation`
//! honestly reports whether the mesh carries the clip, and `GetCurrentNode` reads
//! the active state back.

use std::cell::RefCell;
use std::collections::BTreeMap;

use mlua::Lua;
use rusty::asset::anim_data::AnimationClip;
use rusty::asset::animation_graph::{self, AnimationGraph, GraphNode};
use rusty::components::{AnimatorComponent, MeshComponent};
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

fn scene_with_animator() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Rig".to_string());
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    (RefCell::new(scene), id)
}

fn animator_of(scene: &RefCell<Scene>, id: u32) -> AnimatorComponent {
    scene.borrow().world.animator(id).unwrap().clone()
}

/// Register the `Animator` namespace against `scene`, run `script`, return its
/// value rendered through Lua's `tostring` (the parentheses adjust a zero-value
/// call — a plain setter — to `nil`).
fn run(scene: &RefCell<Scene>, script: &str) -> String {
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, &console).unwrap();
        lua.load(format!("return tostring(({script}))")).eval()
    })
    .unwrap()
}

/// A one-node graph saved to a temp `.animgraph`; the path uses forward slashes
/// so it can be embedded in a Lua literal on Windows too.
fn saved_graph() -> String {
    let graph = AnimationGraph {
        parameters: BTreeMap::new(),
        nodes: vec![GraphNode {
            name: "Run".to_string(),
            clip: "RunClip".to_string(),
            is_loop: true,
            speed: Some(2.0),
        }],
        edges: Vec::new(),
        entry: "Run".to_string(),
    };
    let path = std::env::temp_dir().join("rusty_316_api.animgraph");
    animation_graph::save(&path, &graph).unwrap();
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn set_graph_assigns_clears_and_toggles_evaluation() {
    let (scene, id) = scene_with_animator();
    run(&scene, &format!("Animator.SetGraph({id}, 'a.animgraph')"));
    run(&scene, &format!("Animator.SetGraphEnabled({id}, false)"));
    let anim = animator_of(&scene, id);
    assert_eq!(anim.graph.as_deref(), Some("a.animgraph"));
    assert!(!anim.graph_enabled);
    run(&scene, &format!("Animator.SetGraph({id}, '')"));
    let anim = animator_of(&scene, id);
    assert_eq!(anim.graph, None, "an empty path clears the reference");
}

#[test]
fn play_node_jumps_to_the_named_state_and_reports_honestly() {
    let (scene, id) = scene_with_animator();
    // Before any graph or binding, the current node reads back as nil.
    assert_eq!(
        run(&scene, &format!("Animator.GetCurrentNode({id})")),
        "nil"
    );
    let path = saved_graph();
    run(&scene, &format!("Animator.SetGraph({id}, '{path}')"));
    assert_eq!(
        run(&scene, &format!("Animator.PlayNode({id}, 'Run')")),
        "true"
    );
    let anim = animator_of(&scene, id);
    assert_eq!(anim.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.current_clip, "RunClip");
    assert!(anim.loop_clip, "the node's is_loop is adopted");
    assert_eq!(anim.node_speed, 2.0, "the node's speed is adopted");
    assert_eq!(
        run(&scene, &format!("Animator.GetCurrentNode({id})")),
        "Run"
    );
    assert_eq!(
        run(&scene, &format!("Animator.PlayNode({id}, 'Ghost')")),
        "false",
        "an unknown node is refused"
    );
}

#[test]
fn play_animation_returns_whether_the_mesh_carries_the_clip() {
    let (scene, id) = scene_with_animator();
    // No mesh at all: honest false, and the animator is left untouched.
    assert_eq!(
        run(&scene, &format!("Animator.PlayAnimation({id}, 'Walk')")),
        "false"
    );
    assert!(!animator_of(&scene, id).is_playing);
    scene.borrow_mut().world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: vec![AnimationClip {
                name: "Walk".to_string(),
                tracks: Vec::new(),
                duration: 1.0,
            }],
            pose_palette: Vec::new(),
            is_dirty: Default::default(),
        }),
    );
    assert_eq!(
        run(&scene, &format!("Animator.PlayAnimation({id}, 'Walk')")),
        "true"
    );
    let anim = animator_of(&scene, id);
    assert!(anim.is_playing);
    assert_eq!(anim.current_clip, "Walk");
    assert_eq!(
        run(&scene, &format!("Animator.PlayAnimation({id}, 'Ghost')")),
        "false",
        "a clip the mesh lacks is refused without side effects"
    );
    assert_eq!(animator_of(&scene, id).current_clip, "Walk");
}
