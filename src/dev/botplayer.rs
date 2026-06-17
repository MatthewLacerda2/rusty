//! src/dev/botplayer.rs — Bot-player pattern (notes + shared helpers)
//!
//! A bot-player is NOT special engine code. It is a normal scene script, tagged
//! dev-only ("won't ship"), attached to the Player, that drives WRITABLE Input from
//! its Update() — i.e. it presses the same keys a human would. Run it headless via
//! the harness at max speed and read the summary (won/lost, time, health, errors).
//!
//! The actual bots live in `project/scripts/*.lua`, like any other script. This file
//! only documents the pattern and hosts the small shared helpers the harness needs to
//! wire a bot onto the Player at run time.
//!
//! Allowed deps: api (writable Input).

use crate::scene::{Scene, ScriptComponent};

/// Canonical path to the example bot-player script (attached to the Player).
pub const PLAYER_BOT_SCRIPT: &str = "project/scripts/bot_player.lua";

/// Attach a bot-player script to the entity named `Player` in `scene`.
///
/// Called *before* the first headless tick, so the script is picked up when the
/// world enters play mode (`enter_play` reads every entity's `scripts`). The bot
/// brain *drives* the Player, so it REPLACES the human controller script(s)
/// rather than running alongside them. Returns `true` if a Player was found and
/// tagged, `false` otherwise.
pub fn attach_player_bot(scene: &mut Scene, script_path: &str) -> bool {
    let Some(player_id) = scene.find_entity_by_name("Player") else {
        return false;
    };
    if let Some(mut player) = scene.get_entity_mut(player_id) {
        player.scripts = vec![ScriptComponent {
            path: script_path.to_string(),
            is_loaded: false,
            ..Default::default()
        }];
        return true;
    }
    false
}
