//! The lifecycles the inventory is checked against, as runners (#1649): each sets
//! one mark on the base scene, runs its lifecycle, and reads what came back as an
//! [`Outcome`] for `lifecycle_tests` to hold to the mark's class.
//!
//! A runner proves its lifecycle ran: a save that fails, or an Undo that undid
//! nothing, panics instead of passing for an outcome, unless the mark is one
//! [`IN_PLAY`] names, which the lifecycle refuses by design.

use std::collections::BTreeSet;

use super::marks::{base, Mark};
use super::{Lifecycle, Outcome};
use crate::scene::io::SAVE_IN_PLAY;
use crate::scene::{read_scene_file, Scene, SceneSnapshot};

/// The marks that enter Play. A save refuses them (`SAVE_IN_PLAY`), and Undo has
/// nothing to judge them by: in Play `begin_edits` and `undo` are no-ops and
/// entering Play forgets the history, so no step holds them. What Play does with
/// them is judged by the Stop and Play-load runners instead.
pub const IN_PLAY: &[&str] = &["playing"];

/// Run `lifecycle` on `mark`: `None` when the lifecycle does not apply to it
/// (an Undo of a mark in [`IN_PLAY`]).
pub fn run(lifecycle: Lifecycle, mark: &Mark) -> Option<Outcome> {
    Some(match lifecycle {
        Lifecycle::Save => save(mark),
        Lifecycle::Stop => stop(mark),
        Lifecycle::Undo => return undo(mark),
        Lifecycle::Load => load(mark),
        Lifecycle::PlayLoad => play_load(mark),
    })
}

fn path(name: &str) -> String {
    let file = crate::test_temp::dir().join(format!("{name}.scene"));
    file.to_string_lossy().into_owned()
}

/// An empty scene saved to disk, for a load to read.
fn empty() -> String {
    let empty = path("empty");
    let saved = Scene::new().save_to_file(&empty);
    saved.expect("an empty scene saves");
    empty
}

/// Save → load into a fresh scene. Only a mark that enters Play may be refused,
/// and then the save wrote nothing of it.
fn save(mark: &Mark) -> Outcome {
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    let file = path(mark.name);
    let refused = IN_PLAY.contains(&mark.name);
    match scene.save_to_file(&file) {
        Err(e) if refused && e == SAVE_IN_PLAY => return Outcome::Gone,
        Err(e) => panic!("{}: the save failed: {e}", mark.name),
        Ok(()) => assert!(!refused, "{}: a save in Play went through", mark.name),
    }
    let mut loaded = Scene::new();
    loaded.load_from_file(&file).expect("the saved scene loads");
    seen((mark.is_set)(&loaded, base), Outcome::Back)
}

/// Play → Stop, the mark set before Play (`before`) or during it.
fn play_then_stop(mark: &Mark, before: bool) -> bool {
    let (mut scene, base) = base();
    if before {
        (mark.set)(&mut scene, base);
    }
    let snapshot = SceneSnapshot::capture(&scene);
    scene.set_playing(true);
    if !before {
        (mark.set)(&mut scene, base);
    }
    // In `GameWorld::exit_play`'s order: the play state first, then the restore.
    scene.set_playing(false);
    snapshot.restore(&mut scene);
    (mark.is_set)(&scene, base)
}

fn stop(mark: &Mark) -> Outcome {
    match (play_then_stop(mark, false), play_then_stop(mark, true)) {
        (false, true) => Outcome::Back,
        (false, false) => Outcome::Gone,
        (true, _) => Outcome::Left,
    }
}

/// One edit step holding a real edit (the base moved) and the mark, undone. The
/// base's move must come back, so a step Undo refused can't pass for `Left`.
fn undo(mark: &Mark) -> Option<Outcome> {
    let (mut scene, base) = base();
    scene.begin_edits();
    scene.world.transform_mut(base).expect("live").position.x = 3.0;
    (mark.set)(&mut scene, base);
    let in_play = IN_PLAY.contains(&mark.name);
    assert_eq!(
        scene.is_playing(),
        in_play,
        "{}: listed in IN_PLAY",
        mark.name
    );
    if in_play {
        return None;
    }
    scene.commit_edits(None);
    assert!(scene.undo().is_some(), "{}: Undo undid a step", mark.name);
    let x = scene.world.transform(base).map(|t| t.position.x);
    assert_eq!(x, Some(0.0), "{}: the base's move came back", mark.name);
    // A mark still there was left; one gone was taken back to before the step.
    Some(match (mark.is_set)(&scene, base) {
        true => Outcome::Left,
        false => Outcome::Back,
    })
}

/// An empty scene loaded over the marked one in edit mode.
fn load(mark: &Mark) -> Outcome {
    let empty = empty();
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    scene.load_from_file(&empty).expect("the empty scene loads");
    seen((mark.is_set)(&scene, base), Outcome::Left)
}

/// A Play-mode `Scene.Load` of an empty scene (`Scene::swap_in`, the tick's
/// scene-load phase), the mark set in Play and the base entity left behind, so
/// nothing keyed by it may cross with its now stale id.
fn play_load(mark: &Mark) -> Outcome {
    let empty = empty();
    let (mut scene, base) = base();
    scene.set_playing(true);
    (mark.set)(&mut scene, base);
    let data = read_scene_file(&empty).expect("the empty scene reads");
    let swapped = scene.swap_in(data, &empty, &BTreeSet::new());
    swapped.expect("the empty scene swaps in");
    seen((mark.is_set)(&scene, base), Outcome::Left)
}

/// `kept` (`Back` or `Left`) when the mark is still there, `Gone` when not.
fn seen(there: bool, kept: Outcome) -> Outcome {
    if there {
        kept
    } else {
        Outcome::Gone
    }
}
