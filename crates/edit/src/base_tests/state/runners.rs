//! A runner proves its lifecycle ran: a save that fails, or an Undo that undid
//! nothing, panics instead of passing for an outcome, unless the mark is one
//! [`IN_PLAY`] names, which the lifecycle refuses by design.

use std::collections::BTreeSet;

use crate::io::SAVE_IN_PLAY;
use crate::SceneSnapshot;
use rusty_scene::state::marks::{base, Mark};
use rusty_scene::state::{Lifecycle, Outcome};
use rusty_scene::{read_scene_file, Scene};

/// The marks that enter Play. A save refuses them (`SAVE_IN_PLAY`), and Undo has
/// nothing to judge them by: in Play `begin_edits` and `undo` are no-ops and
/// entering Play forgets the history, so no step holds them.
pub const IN_PLAY: &[&str] = &["playing"];

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
    let file = rusty_test_support::temp::dir().join(format!("{name}.scene"));
    file.to_string_lossy().into_owned()
}

fn empty() -> String {
    let empty = path("empty");
    let saved = crate::save_to_file(&Scene::new(), &empty);
    saved.expect("an empty scene saves");
    empty
}

fn save(mark: &Mark) -> Outcome {
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    let file = path(mark.name);
    let refused = IN_PLAY.contains(&mark.name);
    match crate::save_to_file(&scene, &file) {
        Err(e) if refused && e == SAVE_IN_PLAY => return Outcome::Gone,
        Err(e) => panic!("{}: the save failed: {e}", mark.name),
        Ok(()) => assert!(!refused, "{}: a save in Play went through", mark.name),
    }
    let mut loaded = Scene::new();
    crate::load_from_file(&mut loaded, &file).expect("the saved scene loads");
    seen((mark.is_set)(&loaded, base), Outcome::Back)
}

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

fn undo(mark: &Mark) -> Option<Outcome> {
    let (mut scene, base) = base();
    scene.begin_edits();
    // A real edit beside the mark, so a step Undo refused can't pass for `Left`.
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
    Some(match (mark.is_set)(&scene, base) {
        true => Outcome::Left,
        false => Outcome::Back,
    })
}

fn load(mark: &Mark) -> Outcome {
    let empty = empty();
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    crate::load_from_file(&mut scene, &empty).expect("the empty scene loads");
    seen((mark.is_set)(&scene, base), Outcome::Left)
}

fn play_load(mark: &Mark) -> Outcome {
    let empty = empty();
    let (mut scene, base) = base();
    scene.set_playing(true);
    (mark.set)(&mut scene, base);
    let data = read_scene_file(&empty).expect("the empty scene reads");
    let swapped = crate::load::swap_in(&mut scene, data, &empty, &BTreeSet::new());
    swapped.expect("the empty scene swaps in");
    seen((mark.is_set)(&scene, base), Outcome::Left)
}

fn seen(there: bool, kept: Outcome) -> Outcome {
    if there {
        kept
    } else {
        Outcome::Gone
    }
}
