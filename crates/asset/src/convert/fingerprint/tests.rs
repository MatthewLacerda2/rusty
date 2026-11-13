//! The converter fingerprint gate (#1286): the converters inside the engine
//! checked against the table, and the table's rules on made-up rows.

use super::table::{bless, check, format, parse, Now, Row};

/// The converters that need no program: every one but Blender's, whose check
/// runs where Blender does (`blend::tests::fingerprint`). Linux only: another
/// platform's maths library may round a converted float differently, and a bump
/// is caught as well by one platform as by two.
#[cfg(target_os = "linux")]
#[test]
fn converter_fingerprints_hold_for_the_engines_own_converters() {
    super::gate(|c| c.tool.is_none());
}

fn now(version: u32, hash: &str) -> Now {
    Now {
        converter: "ufbx".into(),
        version,
        tool: "-".into(),
        hashes: vec![("a.fbx".into(), hash.into())],
    }
}

fn blessed(version: u32, hash: &str) -> Vec<Row> {
    bless(&[], &now(version, hash)).expect("a first bless")
}

#[test]
fn an_unchanged_output_at_an_unchanged_version_passes() {
    assert_eq!(check(&blessed(9, "aa"), &now(9, "aa")), Ok(()));
}

#[test]
fn a_changed_output_at_an_unchanged_version_fails_asking_for_the_bump() {
    let e = check(&blessed(9, "aa"), &now(9, "bb")).unwrap_err();
    assert!(e.contains("ufbx converter's output changed (a.fbx)"), "{e}");
    assert!(
        e.contains("bump its `version` in `convert/converter.rs`"),
        "{e}"
    );
}

#[test]
fn a_bumped_version_without_a_re_bless_fails_with_the_bless_hint() {
    for hash in ["aa", "bb"] {
        let e = check(&blessed(9, "aa"), &now(10, hash)).unwrap_err();
        assert!(
            e.contains("at version 10 but its fingerprint was blessed at 9"),
            "{e}"
        );
        assert!(e.contains("make bless-converters"), "{e}");
    }
}

#[test]
fn a_re_bless_is_refused_in_place_of_the_bump_and_taken_after_it() {
    let e = bless(&blessed(9, "aa"), &now(9, "bb")).unwrap_err();
    assert!(e.contains("bump its `version`"), "{e}");
    let rows = bless(&blessed(9, "aa"), &now(10, "bb")).expect("a bumped version re-blesses");
    assert_eq!(check(&rows, &now(10, "bb")), Ok(()));
}

#[test]
fn another_program_or_new_fixtures_ask_for_a_re_bless_not_a_bump() {
    let mut other_tool = now(9, "bb");
    other_tool.tool = "5.3.0".into();
    let e = check(&blessed(9, "aa"), &other_tool).unwrap_err();
    assert!(
        e.contains("blessed with its program at -, this one is 5.3.0"),
        "{e}"
    );
    assert!(bless(&blessed(9, "aa"), &other_tool).is_ok());
    let mut more = now(9, "aa");
    more.hashes.push(("b.fbx".into(), "cc".into()));
    let e = check(&blessed(9, "aa"), &more).unwrap_err();
    assert!(e.contains("fixtures changed"), "{e}");
    assert!(bless(&blessed(9, "aa"), &more).is_ok());
}

#[test]
fn a_bless_keeps_the_other_converters_rows() {
    let mut blend = now(1, "dd");
    blend.converter = "blender".into();
    blend.tool = "5.2.3 LTS".into();
    let rows = bless(&blessed(9, "aa"), &blend).unwrap();
    let read = parse(&format(&rows)).expect("the written table reads");
    assert_eq!(read.len(), 2);
    assert_eq!(check(&read, &now(9, "aa")), Ok(()));
    assert_eq!(
        check(&read, &blend),
        Ok(()),
        "a tool with a space round-trips"
    );
}

#[test]
fn a_malformed_row_is_an_error_naming_it() {
    assert!(parse("ufbx nine a.fbx aa -").unwrap_err().contains("nine"));
    assert!(parse("# comment\n\n").unwrap().is_empty());
}
