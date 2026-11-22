use super::{fragment, report};
use crate::repo::scaffold::Repo;

fn signals(files: &[(&str, &str)], doc: &str) -> Vec<String> {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let repo = Repo::new(&format!("comment-signals-{run}"), false);
    repo.file("docs/api/X.md", doc);
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(p, s)| (p.to_string(), s.to_string()))
        .collect();
    let out = report(&repo.0, &files);
    assert!(out.starts_with("signal (not a failure)"));
    out.lines().skip(1).map(str::to_string).collect()
}

const SAID: &str = "// The probe grid is rebuilt whenever a static collider moves.\nfn f() {}\n";

#[test]
fn one_point_at_two_sites_of_a_crate_is_signalled_once() {
    let twice = [("crates/a/src/x.rs", SAID), ("crates/a/src/y.rs", SAID)];
    let found = signals(&twice, "");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0]
        .starts_with("crates/a/src/x.rs:1: [comment-duplicate] also said at crates/a/src/y.rs:1"));
    let apart = [("crates/a/src/x.rs", SAID), ("crates/b/src/y.rs", SAID)];
    assert!(signals(&apart, "").is_empty(), "another crate's");
    let short = "// Off by default.\n";
    let short = [("crates/a/src/x.rs", short), ("crates/a/src/y.rs", short)];
    assert!(signals(&short, "").is_empty(), "too short to be one point");
}

#[test]
fn a_sentence_a_doc_already_says_is_signalled() {
    let doc = "# X\n\nThe probe grid is rebuilt whenever a static collider moves. More.\n";
    let found = signals(&[("crates/a/src/x.rs", SAID)], doc);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("[comment-doc] restates docs/api/X.md:3"));
    let other =
        "# X\n\nA probe volume holds one irradiance sample per grid cell, in linear light.\n";
    assert!(signals(&[("crates/a/src/x.rs", SAID)], other).is_empty());
}

#[test]
fn a_fragment_opener_is_signalled_and_a_sentence_is_not() {
    for block in [
        "So the next frame.",
        "On a resize, the cache.",
        "Not for the player.",
    ] {
        assert!(fragment(block), "{block}");
    }
    for block in [
        "So the next frame reads it.",
        "On a resize the cache is dropped.",
        "The cache, on a resize.",
        "In order: the shadows, then the lights.",
    ] {
        assert!(!fragment(block), "{block}");
    }
    let found = signals(
        &[("crates/a/src/x.rs", "// Before the bake.\nfn f() {}\n")],
        "",
    );
    assert_eq!(found, ["crates/a/src/x.rs:1: [comment-fragment] opens on a fragment; start with what the code does"]);
}
