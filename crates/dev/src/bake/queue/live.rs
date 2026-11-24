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

pub const UNSAVED: &str = "save the open scene first: the bake queue reads every scene from its \
file and opens the scene again when it ends";

/// Why a queue cannot start over the live scene now: Play (asked for included), a bake
/// running or asked for, or unsaved edits to the open scene.
pub fn refuse(scene: &Scene, open: Option<&str>, play_heading: bool) -> Result<(), String> {
    if scene.is_playing() || play_heading {
        return Err(LIGHTING_IN_PLAY.to_string());
    }
    refuse_while_baking(scene)?;
    match open.is_some_and(|path| rusty_edit::has_unsaved_edits(scene, path)) {
        true => Err(UNSAVED.to_string()),
        false => Ok(()),
    }
}

/// Every scene left in the queue, baked one after another; whether `open` (the live
/// scene's file) was one of them, so the caller reads it again.
pub fn run_beside(
    runner: &mut Runner,
    open: Option<&str>,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<bool, String> {
    let root = runner.root().to_path_buf();
    let total = runner.queue().scenes.len();
    let mut reopen = false;
    while let Some((index, path)) = runner.next()? {
        say(format!(
            "[Lighting] bake queue: scene {} of {total}: {path}",
            index + 1
        ));
        let outcome = bake_scene(&root, &path, say, log);
        if let Outcome::Failed(why) = &outcome {
            say(format!("[Lighting] bake queue: {path} failed: {why}"));
        }
        reopen |= open.is_some_and(|open| same_file(&root, &path, open));
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
