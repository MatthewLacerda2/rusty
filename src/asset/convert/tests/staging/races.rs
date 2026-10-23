//! The swap seen from between its steps (`stage::hook`): a reader, a second
//! conversion, a rename that fails.

use super::*;
use std::path::PathBuf;
use std::rc::Rc;

/// A source and its cache in a fresh folder, converted once from "one".
fn cached() -> (PathBuf, PathBuf, PathBuf) {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&TEXTURED, &source, &out).unwrap();
    (dir, source, out)
}

#[test]
fn a_reader_between_the_swaps_steps_finds_a_whole_set() {
    let (dir, source, out) = cached();
    std::fs::write(&source, "two").unwrap();
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (log, at) = (seen.clone(), out.clone());
    hook::set(move |step| {
        if step != Step::Staged {
            let stamped = at.with_file_name("a.src.glb.source").is_file();
            log.borrow_mut().push((step, read_whole(&at), stamped));
        }
    });
    let result = refresh(&TEXTURED, &source, &out);
    hook::unset();
    result.unwrap();
    assert_eq!(
        *seen.borrow(),
        [
            (Step::TexturesIn, "one".into(), true),
            // The old stamp went before the `.glb`: a crash here re-converts.
            (Step::GlbIn, "two".into(), false),
            (Step::Stamped, "two".into(), true),
        ]
    );
    assert_eq!(textures_dirs(&out).len(), 1, "{:?}", names(&dir));
    // The source, the `.glb`, its textures and its stamp: the lock is elsewhere.
    assert_eq!(names(&dir).len(), 4, "{:?}", names(&dir));
}

#[test]
fn a_second_conversion_waits_for_the_first() {
    let (dir, source, out) = cached();
    std::fs::write(&source, "three").unwrap();
    let b = Rc::new(std::cell::RefCell::new(None));
    let (slot, b_source, b_out) = (b.clone(), source.clone(), out.clone());
    hook::set(move |step| {
        if step == Step::GlbIn {
            // B, another thread as another process would be, converts "two".
            let (source, out) = (b_source.clone(), b_out.clone());
            let (done, finished) = std::sync::mpsc::channel();
            let handle = std::thread::spawn(move || {
                std::fs::write(&source, "two").unwrap();
                let result = refresh(&TEXTURED, &source, &out);
                let _ = done.send(());
                result
            });
            let early = finished.recv_timeout(std::time::Duration::from_millis(200));
            assert!(early.is_err(), "B ran between A's steps");
            *slot.borrow_mut() = Some(handle);
        }
    });
    let a = refresh(&TEXTURED, &source, &out);
    hook::unset();
    a.unwrap();
    // B converted after A, reading what is on disk by then.
    let handle = b.borrow_mut().take().expect("B started");
    handle.join().unwrap().unwrap();
    assert_eq!(read_whole(&out), "two");
    assert_eq!(textures_dirs(&out).len(), 1, "{:?}", names(&dir));
}

#[test]
fn without_the_lock_an_interleaved_conversion_still_leaves_a_whole_set() {
    // B converts "two" through another source path, as on a filesystem with no
    // locks, while A, converting "three", is at `step`. Between A's `.glb` rename
    // and its stamp only the lock keeps B out (the test above).
    for (step, stamped) in [
        (Step::Staged, true),
        (Step::TexturesIn, false),
        (Step::Stamped, true),
    ] {
        let (dir, source, out) = cached();
        let (b_source, b_out) = (dir.join("b-copy.src"), out.clone());
        std::fs::write(&b_source, "two").unwrap();
        std::fs::write(&source, "three").unwrap();
        hook::set(move |at| {
            if at == step {
                lock::unavailable(|| refresh(&TEXTURED, &b_source, &b_out)).unwrap();
            }
        });
        let a = refresh(&TEXTURED, &source, &out);
        hook::unset();
        // A fails when B cleared its staging folder, else B's set is the newer.
        assert_eq!(a.is_ok(), step == Step::Stamped, "{step:?}: {a:?}");
        assert_eq!(read_whole(&out), "two", "{step:?}");
        let stamp = dir.join("a.src.glb.source");
        assert_eq!(stamp.is_file(), stamped, "{step:?}: unstamped re-converts");
        assert_eq!(textures_dirs(&out).len(), 1, "{step:?}: {:?}", names(&dir));
    }
}

/// A converter that stages its textures but no `.glb`, so step 3's rename fails.
const NO_GLB: Converter = Converter {
    convert: no_glb,
    ..TEXTURED
};

fn no_glb(_: &Path, to: &Staging) -> Result<Converted, ImportError> {
    std::fs::create_dir(&to.textures).unwrap();
    std::fs::write(to.textures.join("t.png"), "orphan").unwrap();
    Ok(Converted::default())
}

#[test]
fn a_failed_glb_rename_unstamps_the_old_cache_and_removes_its_orphan() {
    let (dir, source, out) = cached();
    std::fs::write(&source, "two").unwrap();
    assert!(refresh(&NO_GLB, &source, &out).is_err());
    assert_eq!(read_whole(&out), "one");
    assert!(
        !dir.join("a.src.glb.source").exists(),
        "re-converts next time"
    );
    assert_eq!(textures_dirs(&out).len(), 1, "{:?}", names(&dir));
}
