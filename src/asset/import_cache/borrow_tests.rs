//! A cached import that borrows clips (#1747) follows a converted source: a
//! re-exported `.fbx` is converted again on the borrower's lookup, and the
//! borrower is parsed again. And one cache serves every thread (#2123).

use super::*;
use crate::asset::humanoid::tests::model;

#[test]
fn a_reexported_converted_source_reparses_the_borrower() {
    let dir = crate::test_temp::dir().join("rusty_import_cache_borrow_fbx");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("grunt.fbx");
    let fbx = include_str!("../../../tests/fixtures/models/embedded_textures.fbx");
    std::fs::write(&source, fbx).unwrap();
    let brute = model(&dir, "brute.gltf", "Idle", &[&source]);
    imported(&brute).unwrap();
    let before = parses();
    imported(&brute).unwrap();
    assert_eq!(parses(), before, "served from the cache");
    // A re-export, written by another process (not the engine's writer).
    std::fs::write(&source, format!("{fbx}\n; re-exported\n")).unwrap();
    imported(&brute).unwrap();
    assert_eq!(
        parses(),
        before + 1,
        "the source's change re-made the import"
    );
}

#[test]
fn an_import_made_on_another_thread_is_this_threads_hit() {
    // A load worker imports a level's models (#2123); the ticking thread uses them.
    let path = super::tests::model("worker");
    let from = path.clone();
    let worker = std::thread::spawn(move || super::imported(&from).unwrap()).join();
    let worker = worker.unwrap();
    let before = super::parses();
    let here = super::imported(&path).unwrap();
    assert!(std::sync::Arc::ptr_eq(&worker.asset, &here.asset));
    assert_eq!(super::parses(), before, "no parse on this thread");
}
