use super::{check, RULE};
use crate::repo::scaffold::Repo;

/// A repo whose root and one crate, `ecs`, each have a `lib.rs` and a test file.
fn repo(name: &str) -> Repo {
    let repo = Repo::new(&format!("orphan-seeds-{name}"), false);
    repo.file("src/lib.rs", "");
    repo.file("src/physics/watch/tests.rs", "");
    repo.file("tests/main.rs", "");
    repo.file("tests/proptest/world_space.rs", "");
    repo.file("crates/ecs/src/lib.rs", "");
    repo.file("crates/ecs/src/changes/derived_tests.rs", "");
    repo
}

#[test]
fn passes_seeds_beside_their_sources() {
    let dir = repo("pass");
    dir.file("proptest-regressions/physics/watch/tests.txt", "cc 1\n");
    dir.file("proptest-regressions/proptest/world_space.txt", "cc 2\n");
    dir.file(
        "crates/ecs/proptest-regressions/changes/derived_tests.txt",
        "cc 3\n",
    );
    assert!(check(&dir.0).is_empty(), "{:?}", check(&dir.0));
}

#[test]
fn flags_a_root_seed_whose_module_moved_and_names_where_it_goes() {
    let dir = repo("moved");
    dir.file(
        "proptest-regressions/ecs/changes/derived_tests.txt",
        "cc 1\n",
    );
    let found = check(&dir.0);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].path.as_str(), found[0].rule),
        ("proptest-regressions/ecs/changes/derived_tests.txt", RULE)
    );
    let to = "crates/ecs/proptest-regressions/changes/derived_tests.txt";
    assert!(found[0].message.contains(to), "{}", found[0]);
}

#[test]
fn flags_a_seed_whose_test_was_renamed_or_deleted() {
    let dir = repo("gone");
    dir.file("proptest-regressions/physics/watch/old.txt", "cc 1\n");
    dir.file(
        "crates/ecs/proptest-regressions/activity_tests.txt",
        "cc 2\n",
    );
    let paths: Vec<_> = check(&dir.0).into_iter().map(|f| f.path).collect();
    assert_eq!(
        paths,
        [
            "proptest-regressions/physics/watch/old.txt",
            "crates/ecs/proptest-regressions/activity_tests.txt"
        ]
    );
}
