//! Every field through every lifecycle, against its class's promise (#1649).
//! Each mark is set on the base scene, the lifecycle runs, and what came back is
//! read as an [`Outcome`]; `check::disagreements` lists every one that breaks
//! its promise. A field with no mark fails, so none is left out.

use super::check::{disagreements, Observed};
use super::inventory::fields;
use super::marks::{all, base, Mark};
use super::{Class, Lifecycle, Outcome};
use crate::scene::{Scene, SceneSnapshot};

fn path(name: &str) -> String {
    let file = crate::test_temp::dir().join(format!("{name}.scene"));
    file.to_string_lossy().into_owned()
}

/// Save → load into a fresh scene. A save Play refuses wrote nothing of it.
fn save(mark: &Mark) -> Outcome {
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    let file = path(mark.name);
    if scene.save_to_file(&file).is_err() {
        return Outcome::Gone;
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

/// One edit step holding a real edit (the base moved) and the mark, undone.
fn undo(mark: &Mark) -> Outcome {
    let (mut scene, base) = base();
    scene.begin_edits();
    scene.world.transform_mut(base).expect("live").position.x = 3.0;
    (mark.set)(&mut scene, base);
    scene.commit_edits(None);
    scene.undo();
    match (mark.is_set)(&scene, base) {
        true => Outcome::Left,
        false => Outcome::Back,
    }
}

/// An empty scene loaded over the marked one.
fn load(mark: &Mark) -> Outcome {
    let empty = path("empty");
    Scene::new()
        .save_to_file(&empty)
        .expect("an empty scene saves");
    let (mut scene, base) = base();
    (mark.set)(&mut scene, base);
    scene.load_from_file(&empty).expect("the empty scene loads");
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

fn class_of(mark: &Mark) -> Class {
    let field = fields().into_iter().find(|f| f.name == mark.field());
    let class = field
        .unwrap_or_else(|| panic!("{} marks no field", mark.name))
        .class;
    mark.part.unwrap_or(class)
}

#[test]
fn every_field_has_a_mark() {
    let marked: Vec<&str> = all().iter().map(|m| m.field()).collect();
    let bare: Vec<&str> = fields()
        .iter()
        .map(|f| f.name)
        .filter(|n| !marked.contains(n))
        .collect();
    assert!(bare.is_empty(), "fields with no mark in `marks`: {bare:?}");
}

#[test]
fn every_lifecycle_keeps_every_class_promise() {
    let mut observed = Vec::new();
    for mark in all() {
        for lifecycle in Lifecycle::ALL {
            let outcome = match lifecycle {
                Lifecycle::Save => save(mark),
                Lifecycle::Stop => stop(mark),
                Lifecycle::Undo => undo(mark),
                Lifecycle::Load => load(mark),
            };
            let class = class_of(mark);
            observed.push(Observed {
                mark: mark.name,
                class,
                lifecycle,
                outcome,
            });
        }
    }
    assert_eq!(disagreements(&observed), Vec::<String>::new());
}
