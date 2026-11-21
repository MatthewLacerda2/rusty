use rusty_edit::authoring::add_script;
use rusty_scene::Scene;

pub const PLAYER_BOT_SCRIPT: &str = "assets/scripts/bot_player.lua";

pub fn attach_player_bot(scene: &mut Scene, script_path: &str) -> bool {
    let Some(player_id) = scene.find_entity_by_name("Player") else {
        return false;
    };
    if let Some(mut scripts) = scene.world.scripts_mut(player_id) {
        scripts.clear();
    }
    add_script(&mut scene.world, player_id, script_path, Default::default())
}
