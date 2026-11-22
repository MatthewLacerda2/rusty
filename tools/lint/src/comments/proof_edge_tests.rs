use super::tests::{passes, prove};

#[test]
fn a_lifetime_beside_a_removed_comment_passes() {
    let old = "fn f(x: &'static str) {}\n// gone\nfn g() {}\n";
    assert!(passes("a.rs", old, "fn f(x: &'static str) {}\nfn g() {}\n"));
    let split = "fn f(x: &'static str, // why\n    y: u8) {}\n";
    assert!(passes("a.rs", split, "fn f(x: &'static str, y: u8) {}\n"));
    let doc = "struct S<'a>(&'a u8);\n#[doc = \"x\"]\nfn f() {}\n";
    assert!(passes("a.rs", doc, "struct S<'a>(&'a u8);\nfn f() {}\n"));
}

#[test]
fn an_edit_after_a_lifetime_or_a_joined_token_fails() {
    let old = "fn f<'a>(x: &'a str) { g(\"don't  panic\") }\n";
    assert!(!passes(
        "a.rs",
        old,
        "fn f<'a>(x: &'a str) { g(\"don't panic\") }\n"
    ));
    for (old, new) in [
        ("a & &b", "a && b"),
        ("a - -b", "a --b"),
        ("x < <y", "x << y"),
        ("a = =b", "a == b"),
    ] {
        assert!(
            !passes("a.rs", &format!("{old};\n"), &format!("{new};\n")),
            "{old}"
        );
    }
}

#[test]
fn the_line_named_is_the_first_differing_tokens() {
    let old = "// x\n// y\nfn a() {}\nfn b() {}\nfn c() {}\n";
    let (found, _) = prove(
        &[("a.rs", old)],
        &[("a.rs", Some("fn a() {}\nfn d() {}\nfn c() {}\n"))],
    );
    assert!(found[0].starts_with("a.rs:2: [comment-only]"), "{found:?}");
    let (found, _) = prove(
        &[("a.rs", "fn a() {}\n")],
        &[("a.rs", Some("fn a() {}\n\nfn b() {}\n"))],
    );
    assert!(found[0].starts_with("a.rs:3: [comment-only]"), "{found:?}");
}
