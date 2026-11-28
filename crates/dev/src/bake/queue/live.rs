//! The queue run in one call (`Lighting.GenerateScenes`, `export --bake`): each scene
//! baked beside the live one, which is read again from its file at the end when the
//! queue baked it.

use std::path::Path;

use super::run::{Outcome, Runner};
use super::scene::bake_scene;
use crate::bake::generate_lighting::refuse_while_baking;
use crate::bake::progress::BakeLog;
use rusty_edit::LIGHTING_IN_PLAY;
use rusty_scene::Scene;

pub const UNSAVED: &str = "save the open scenes first: the bake queue reads every scene from its \
file and opens the loaded scenes again when it ends";

/// Refused in Play (asked for included), while a bake runs or is asked for, and on unsaved
/// edits to any loaded scene.
pub fn refuse(scene: &Scene, open: Option<&str>, play_heading: bool) -> Result<(), String> {
    if scene.is_playing() || play_heading {
        return Err(LIGHTING_IN_PLAY.to_string());
    }
    refuse_while_baking(scene)?;
    let unsaved = match scene.loaded.active_of_several() {
        Some(_) => rusty_edit::scenes::any_unsaved(scene),
        None => open.is_some_and(|path| rusty_edit::has_unsaved_edits(scene, path)),
    };
    match unsaved {
        true => Err(UNSAVED.to_string()),
        false => Ok(()),
    }
}

pub fn live_files(scene: &Scene, open: Option<&str>) -> Vec<String> {
    match scene.loaded.active_of_several() {
        Some(_) => scene.loaded.paths(None).into_iter().flatten().collect(),
        None => open.map(str::to_string).into_iter().collect(),
    }
}

/// Every scene left in the queue, baked in turn; whether it baked one of `open`.
pub fn run_beside(
    runner: &mut Runner,
    open: &[String],
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<bool, String> {
    let root = runner.root().to_path_buf();
    let total = runner.queue().scenes.len();
    let mut reopen = false;
    while let Some((index, path)) = runner.start_next()? {
        say(format!(
            "[Lighting] bake queue: scene {} of {total}: {path}",
            index + 1
        ));
        let outcome = bake_scene(&root, &path, say, log);
        if let Outcome::Failed(why) = &outcome {
            say(format!("[Lighting] bake queue: {path} failed: {why}"));
        }
        reopen |= open.iter().any(|open| same_file(&root, &path, open));
        runner.finish(index, outcome)?;
    }
    Ok(reopen)
}

/// The queue's `path` names the file `open` does, spelled relative or absolute.
pub fn same_file(root: &Path, path: &str, open: &str) -> bool {
    let queued = super::scene::at(root, path);
    queued == open
        || matches!(
            (std::fs::canonicalize(&queued), std::fs::canonicalize(open)),
            (Ok(a), Ok(b)) if a == b
        )
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
