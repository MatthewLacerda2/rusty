//! What keeps one process's answers apart from another's: a memo of its own,
//! the path as found in the key, and a count that must be written.

use super::super::super::known;
use super::super::super::locate::Memo;
use super::super::super::Search;
use super::{counting, fresh, kept_in, runs};

/// The process's own memo is never emptied by a fresh one: a test asking it
/// between another's calls leaves that test's count alone, in either order.
#[test]
fn a_fresh_memo_leaves_the_process_memo_alone() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    let (search, _) = kept_in(&dir, &blender);
    let other = counting(&dir.join("other"), "5.0.1");
    let (other, _) = kept_in(&dir.join("o"), &other);
    for turn in 0..3 {
        if turn != 1 {
            search.find().expect("the process memo");
        }
        fresh(&other).unwrap();
    }
    assert_eq!(runs(&dir.join("bin")), 1, "the process asked once");
}

#[test]
fn two_paths_to_one_binary_keep_two_answers() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    let link = |name: &str| {
        let at = dir.join(name).join("blender");
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&blender, &at).unwrap();
        kept_in(&dir, &at).0
    };
    let (a, b) = (link("a"), link("b"));
    fresh(&a).unwrap();
    fresh(&b).unwrap();
    assert_eq!(runs(&dir.join("bin")), 2, "b did not read a's answer");
    let memo = Memo::default();
    a.find_in(&memo).unwrap();
    b.find_in(&memo).unwrap();
    assert_eq!(runs(&dir.join("bin")), 4, "nor a's memo");
}

#[test]
fn a_count_that_cannot_be_written_asks_blender() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    // The temp name beside a file this long passes the name limit, so no write
    // there succeeds while the file reads (permissions would not hold as root).
    let file = dir.join("k".repeat(250));
    let (search, kept) = kept_in(&dir, &blender);
    fresh(&search).unwrap();
    std::fs::copy(&kept, &file).unwrap();
    let search = Search {
        known: Some(file.clone()),
        ..search
    };
    fresh(&search).unwrap();
    assert_eq!(runs(&dir.join("bin")), 2, "an uncounted read is no answer");
    assert_eq!(known::read(&file).unwrap().reads, 0);
}

#[test]
fn a_changed_executable_is_asked_again() {
    let dir = rusty_test_support::temp::dir();
    let bin = dir.join("bin");
    let blender = counting(&bin, "5.0.1");
    let (search, _) = kept_in(&dir, &blender);
    fresh(&search).unwrap();
    counting(&bin, "5.1.10");
    assert_eq!(fresh(&search).as_deref(), Ok("5.1.10"), "a new length");
    let epoch = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    let file = std::fs::File::options().write(true).open(&blender).unwrap();
    file.set_modified(epoch).unwrap();
    drop(file); // closed before it runs: never "text file busy"
    fresh(&search).unwrap();
    assert_eq!(runs(&bin), 3, "a new modification time asks again");
}
