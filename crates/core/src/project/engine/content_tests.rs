use super::*;

#[test]
fn the_folder_this_build_reads_is_hashed_like_any_other() {
    let engine = crate::project::engine_dir();
    assert_eq!(on_disk(), digest(engine).ok().as_deref());
    assert!(on_disk().is_some(), "{} has a digest", engine.display());
}

#[test]
fn a_stamp_matches_only_when_both_digests_are_known_and_equal() {
    let matches = |b, f| EngineStamp::of(b, f).content_matches;
    assert_eq!(matches(Some("a"), Some("a")), Some(true));
    assert_eq!(matches(Some("a"), Some("b")), Some(false));
    assert_eq!(matches(None, Some("b")), None);
    assert_eq!(matches(Some("a"), None), None);
    assert_eq!(EngineStamp::of(None, None).commit, engine_commit());
}

#[test]
fn only_a_mismatch_warns_and_the_warning_names_both_digests() {
    assert_eq!(EngineStamp::of(Some("a"), Some("a")).warning(), None);
    assert_eq!(EngineStamp::of(None, Some("b")).warning(), None);
    let unhashed = EngineStamp::of(Some("0123abcd"), None).warning().unwrap();
    assert!(unhashed.contains("could not be hashed"), "{unhashed}");
    let warning = EngineStamp::of(Some("0123abcd"), Some("feed9876"))
        .warning()
        .unwrap();
    for part in ["0123abcd", "feed9876", "Rebuild"] {
        assert!(warning.contains(part), "{part} in {warning}");
    }
    let engine = crate::project::engine_dir().display().to_string();
    assert!(warning.contains(&engine), "{warning}");
}

#[test]
fn the_stamp_serialises_with_every_field_named() {
    let json = serde_json::to_value(EngineStamp::of(Some("a"), Some("b"))).unwrap();
    for key in [
        "commit",
        "content_built",
        "content_found",
        "content_matches",
    ] {
        assert!(json.get(key).is_some(), "{key} in {json}");
    }
}

#[test]
fn registering_another_build_warns_once_and_keeps_the_first() {
    // Alone: `BUILT` is the process's, set once by whoever registers first.
    rusty_test_support::alone!(|| {
        let other = "0000000000000000";
        let logged = rusty_test_support::logs::during(|| {
            register_built(other);
            register_built(on_disk().unwrap());
        });
        assert_eq!(stamp().content_built.as_deref(), Some(other));
        assert_eq!(stamp().content_matches, Some(false));
        let warnings: Vec<_> = logged.iter().filter(|l| l.contains(other)).collect();
        assert_eq!(warnings.len(), 1, "{logged:?}");
    });
}

#[test]
fn an_unstamped_process_never_reads_the_folder() {
    // Alone: a sibling hashing the folder or registering a build sets `FOUND`.
    rusty_test_support::alone!(|| {
        assert_eq!(stamp(), EngineStamp::of(None, None));
        assert!(FOUND.get().is_none(), "the folder was hashed");
    });
}
