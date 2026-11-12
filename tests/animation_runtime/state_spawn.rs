//! Graph-state callbacks reach scripts that wake after their animator binds
//! (#1901): a prefab instantiated from `Update` hears its entry state enter
//! once, and an entity deactivated and activated again hears its states
//! enter afresh, with no exit while it was off (Unity's default,
//! `keepAnimatorStateOnDisable` off).

use std::cell::RefCell;
use std::rc::Rc;

use rusty::components::AnimatorComponent;
use rusty::scene::{prefab, Scene};
use rusty::scripting::ConsoleLogs;

const GRAPH: &str = r#"{
  "nodes": [ { "name": "Idle", "clip": "Slide", "is_loop": true, "tag": "calm" } ],
  "entry": "Idle"
}"#;

const BOT: &str = r#"return {
  OnEnable = function(id) print("[enable]") end,
  OnStateEnter = function(id, info) print("[enter] " .. info.state) end,
  OnStateExit = function(id, info) print("[exit] " .. info.state) end,
}"#;

const SPAWNER: &str = r#"
local tick, bot = 0, nil
return { Update = function(id)
  tick = tick + 1
  if tick == 2 then bot = Scene.Instantiate(PREFAB); print("[spawn]") end
  if tick == 5 then Scene.SetActive(bot, false); print("[off]") end
  if tick == 8 then Scene.SetActive(bot, true); print("[on]") end
end }"#;

fn slashed(p: std::path::PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

/// A scene holding `Bot`, an Animator on the one-state graph (written as
/// `file`) and the listener script; returns it with the bot's id.
fn bot_scene(file: &str) -> (Scene, u32) {
    let graph = slashed(crate::temp::dir().join(file));
    std::fs::write(&graph, GRAPH).unwrap();
    let mut scene = Scene::new();
    let bot = scene.add_entity("Bot".to_string());
    let anim = AnimatorComponent {
        graph: Some(graph),
        ..Default::default()
    };
    scene.world.set_animator(bot, Some(anim));
    crate::play::attach_script(&mut scene, bot, &format!("{file}.lua"), BOT);
    (scene, bot)
}

/// Write the bot prefab and return its path.
fn bot_prefab() -> String {
    let (template, bot) = bot_scene("rusty_1901_spawn.animgraph");
    let path = slashed(crate::temp::dir().join("rusty_1901_bot.prefab"));
    prefab::save_prefab(&template, bot, &path).unwrap();
    path
}

/// The `[..]` lines `scene` prints over `ticks` steps of Play.
fn log_of(scene: Scene, ticks: usize) -> Vec<String> {
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(Rc::new(RefCell::new(scene)), 5.0, Rc::clone(&console));
    game.set_playing(true);
    crate::play::run(&mut game, ticks);
    let log = console.borrow();
    let lines = log.messages.iter().map(|(m, _)| m.clone());
    lines.filter(|m| m.starts_with('[')).collect()
}

#[test]
fn a_spawned_bot_hears_its_entry_and_a_reactivated_one_hears_it_again() {
    let prefab = bot_prefab();
    let mut scene = Scene::new();
    let spawner = scene.add_entity("Spawner".to_string());
    let source = SPAWNER.replace("PREFAB", &format!("'{prefab}'"));
    crate::play::attach_script(&mut scene, spawner, "rusty_1901_spawner.lua", &source);
    let lines = log_of(scene, 12);
    assert_eq!(
        lines,
        [
            "[spawn]",
            "[enable]",
            "[enter] Idle",
            "[off]",
            "[on]",
            "[enable]",
            "[enter] Idle",
        ],
        "{lines:?}"
    );
}

#[test]
fn a_saved_node_reads_its_tag_before_the_first_step() {
    let (mut scene, bot) = bot_scene("rusty_1901_saved.animgraph");
    let mut anim = scene.world.animator_mut(bot).unwrap();
    anim.base.current_node = Some("Idle".to_string()); // as a loaded scene has it
    drop(anim);
    let probe = scene.add_entity("Probe".to_string());
    let source = format!(
        "return {{ Start = function(id) print('[tag] ' .. tostring(Animator.GetCurrentTag({bot}))) end }}"
    );
    crate::play::attach_script(&mut scene, probe, "rusty_1901_probe.lua", &source);
    let log = log_of(scene, 1);
    assert!(log.contains(&"[tag] calm".to_string()), "{log:?}");
}
