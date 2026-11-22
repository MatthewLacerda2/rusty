use super::super::File;
use super::{claims, phrase};

fn claimed(path: &str, source: &str) -> Vec<(String, String)> {
    let test = vec![false; source.lines().count()];
    let file = File {
        path,
        source,
        test: &test,
    };
    let found = claims(&file);
    found
        .iter()
        .map(|c| (c.kind.to_string(), c.name.to_string()))
        .collect()
}

#[test]
fn the_phrase_is_the_convention_s_own_words() {
    assert_eq!(phrase("A test hook: x."), Some(2));
    assert_eq!(phrase("Test hooks for the bake."), Some(0));
    assert_eq!(phrase("Runs the latest hook registered."), None);
    assert_eq!(phrase("The contest hooks in."), None);
    assert_eq!(phrase("A test hooked up."), None);
    let latest = "/// Runs the latest hook registered.\npub fn run() {}\n";
    assert!(claimed("crates/a/src/b.rs", latest).is_empty());
}

#[test]
fn a_header_claims_its_module_only_when_it_says_the_module_is_one() {
    let path = "crates/asset/src/mesh_data/shared.rs";
    let module =
        "//! A test hook, compiled in every build: other crates' tests use it.\nfn f() {}\n";
    assert_eq!(claimed(path, module), [("mod".into(), "shared".into())]);
    let item = "//! Meshes shared between instances. [`copies`] is a test hook, compiled in\n//! every build.\npub struct Shared;\npub fn copies() {}\n";
    assert_eq!(claimed(path, item), [("fn".into(), "copies".into())]);
    let world = "//! Test hook: the arena `bench` builds on;\n//! `rusty-dev`'s bench reads its `PLAYER`.\npub const PLAYER: u8 = 0;\n";
    let at = "crates/ai/src/perception/test_world.rs";
    assert_eq!(claimed(at, world), [("mod".into(), "test_world".into())]);
    let neither =
        "//! Meshes shared between instances; other crates hold a test hook.\npub struct Shared;\n";
    assert!(claimed(path, neither).is_empty());
}

#[test]
fn an_item_comment_claims_the_item_below_it() {
    let source = "/// A test hook: `dev`'s tests.\n#[doc(hidden)]\npub fn probe() {}\n";
    assert_eq!(
        claimed("crates/a/src/b.rs", source),
        [("fn".into(), "probe".into())]
    );
    let on_impl = "/// A test hook for the bake.\nimpl Bake {}\n";
    assert!(claimed("crates/a/src/b.rs", on_impl).is_empty());
}
