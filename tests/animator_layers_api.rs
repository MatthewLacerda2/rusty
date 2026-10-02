//! The `Animator` layer surface (#457) end-to-end through Lua: layers addressed by
//! index or name, `SetLayerWeight` binding the layers from the graph asset when a
//! script's `Start` runs before the evaluator, the base layer's weight refused,
//! and `GetCurrentNode` / `PlayNode` per layer.

use std::cell::RefCell;

use mlua::Lua;
use rusty::asset::animation_graph::{self, AnimationGraph};
use rusty::components::AnimatorComponent;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

const GRAPH: &str = r#"{
  "nodes": [ { "name": "Run", "clip": "Run" } ], "entry": "Run",
  "layers": [ { "name": "Upper", "weight": 0.25, "mask": ["spine"],
    "nodes": [ { "name": "Aim", "clip": "Aim" }, { "name": "Reload", "clip": "Reload" } ],
    "entry": "Aim" } ]
}"#;

/// An entity whose animator references the layered graph, saved to a temp file.
fn layered_scene() -> (RefCell<Scene>, u32) {
    let graph: AnimationGraph = serde_json::from_str(GRAPH).unwrap();
    let path = crate::temp::dir().join("rusty_457_layers_api.animgraph");
    animation_graph::save(&path, &graph).unwrap();
    let mut scene = Scene::new();
    let id = scene.add_entity("Soldier".to_string());
    let anim = AnimatorComponent {
        graph: Some(path.to_string_lossy().replace('\\', "/")),
        ..Default::default()
    };
    scene.world.set_animator(id, Some(anim));
    (RefCell::new(scene), id)
}

/// Run `script` against `scene`'s `Animator` namespace, its value via `tostring`.
fn run(scene: &RefCell<Scene>, script: &str) -> String {
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, &console).unwrap();
        lua.load(format!("return tostring(({script}))")).eval()
    })
    .unwrap()
}

#[test]
fn layer_weights_resolve_by_name_or_index_before_the_first_step() {
    let (scene, id) = layered_scene();
    // Nothing has evaluated the graph yet: reading binds the layers from the asset.
    let get = |layer: &str| run(&scene, &format!("Animator.GetLayerWeight({id}, {layer})"));
    assert_eq!(get("'Upper'"), "0.25", "the authored weight");
    assert_eq!(get("0"), "1.0", "the base layer is always full weight");
    assert_eq!(
        run(
            &scene,
            &format!("Animator.SetLayerWeight({id}, 'Upper', 2)")
        ),
        "true"
    );
    assert_eq!(get("1"), "1.0", "clamped to [0, 1]");
    run(&scene, &format!("Animator.SetLayerWeight({id}, 1, 0.5)"));
    assert_eq!(get("'Upper'"), "0.5");
    let anim = scene.borrow().world.animator(id).unwrap().clone();
    assert_eq!(anim.layers[0].weight, 0.5);
    assert_eq!(anim.layers[0].playback.current_node.as_deref(), Some("Aim"));
}

#[test]
fn the_base_layer_and_unknown_layers_are_refused() {
    let (scene, id) = layered_scene();
    for layer in ["0", "2", "'Legs'", "-1", "1.5"] {
        assert_eq!(
            run(
                &scene,
                &format!("Animator.SetLayerWeight({id}, {layer}, 0)")
            ),
            "false",
            "layer {layer}"
        );
    }
    assert_eq!(
        run(&scene, &format!("Animator.GetLayerWeight({id}, 'Legs')")),
        "nil"
    );
}

#[test]
fn current_node_and_play_node_address_a_layer() {
    let (scene, id) = layered_scene();
    let current = |args: &str| run(&scene, &format!("Animator.GetCurrentNode({id}{args})"));
    assert_eq!(current(", 'Upper'"), "nil", "not bound yet");
    assert_eq!(
        run(
            &scene,
            &format!("Animator.PlayNode({id}, 'Reload', 'Upper')")
        ),
        "true"
    );
    assert_eq!(current(", 'Upper'"), "Reload");
    assert_eq!(current(", 1"), "Reload");
    assert_eq!(current(""), "nil", "the base layer is still unbound");
    assert_eq!(
        run(&scene, &format!("Animator.PlayNode({id}, 'Run', 'Upper')")),
        "false",
        "Run is a base-layer node"
    );
    assert_eq!(
        run(&scene, &format!("Animator.PlayNode({id}, 'Run')")),
        "true"
    );
    assert_eq!(current(", 0"), "Run");
    assert_eq!(
        run(&scene, &format!("Animator.PlayNode({id}, 'Aim', 'Legs')")),
        "false"
    );
}
