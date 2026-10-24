use super::{slashed_constants, RULE};
use crate::rules::{allowed, Finding};
use std::collections::BTreeSet;

fn check(path: &str, src: &str) -> Vec<Finding> {
    let consts = "const CACHE: &str = \"assets/generated\";\nconst ALIAS: &str = a::CACHE;\n\
                  const ROOTED: &str = \"/engine\";\n";
    let slashed = slashed_constants([("src/k.rs", consts)].into_iter());
    super::check_file(path, src, &slashed)
}

fn flagged(path: &str, src: &str) -> bool {
    check(path, src).len() == 1
}

const JOIN: &str = "    let f = dir.join(\"models/b.fbx\");\n";

#[test]
fn a_joined_slash_literal_is_flagged_in_tests_and_production() {
    for path in ["src/a/tests.rs", "src/a/b.rs", "tests/c.rs"] {
        let found = check(path, &format!("fn f() {{\n{JOIN}}}\n"));
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!((found[0].rule, found[0].line), (RULE, 2));
    }
    assert!(flagged("src/a.rs", "dir.join(\"a\").join(\"b/c\");\n"));
}

#[test]
fn every_form_of_a_slash_path_is_flagged() {
    for src in [
        "p.push(\"a/b\");\n",
        "dir.join(r\"a/b\");\n",
        "dir.join(r#\"a/b\"#);\n",
        "dir.join(format!(\"{X}/y\"));\n",
        "dir.join(&format!(\"{X}/y\"));\n",
        "dir.join(CACHE);\n",
        "dir.join(crate::k::CACHE);\n",
        "dir.join(ALIAS);\n",
        "let p = Path::new(\"a/b\");\n",
        "let p = std::path::Path::new(CACHE);\n",
        "let p = PathBuf::from(format!(\"{X}/y\"));\n",
        "p.push(format!(\"{X}/y.glb\"));\n",
    ] {
        assert!(flagged("src/a.rs", src), "{src}");
    }
    let wrapped = "let f = dir.join(\n    \"a/b\",\n);\n";
    assert_eq!(check("tests/c.rs", wrapped)[0].line, 1, "the call's line");
}

#[test]
fn one_part_per_join_and_separators_pass() {
    for src in [
        "dir.join(\"models\").join(\"b.fbx\");\n",
        "dir.join(name).join(\"x\");\n",
        "let s = [\"a\", \"b/c\"].join(\",\");\n",
        "let rel = parts.join(\"/\");\n",
        "let rel = parts.join(\" / \");\n",
        "let rel = parts.join(\"//\");\n",
        "dir.join(OTHER);\n",
        "dir.join(CACHE.len());\n",
        "v.push(\"a/b\".into());\n",
        "v.push(format!(\"no file '{b}/c'\"));\n",
        "let p = Path::new(\"/tmp/rusty.sock\");\n",
        "let p = PathBuf::from(\"x\");\n",
        "dir.join(ROOTED);\n",
        "// dir.join(\"a/b\")\n",
    ] {
        assert!(check("tests/c.rs", src).is_empty(), "{src}");
    }
    assert!(
        check("tools/x.rs", JOIN).is_empty(),
        "outside src/ and tests/"
    );
}

#[test]
fn the_message_names_the_helper_each_crate_can_reach() {
    assert!(check("src/a.rs", JOIN)[0]
        .message
        .contains("crate::core::paths::native"));
    assert!(check("tests/a.rs", JOIN)[0]
        .message
        .contains("rusty::core::paths::native"));
    let made = check("src/a.rs", "let p = Path::new(\"a/b\");\n");
    assert!(made[0].message.starts_with("a path made from"));
}

#[test]
fn only_slash_path_constants_are_collected() {
    let src = "const A: &str = \"a/b\";\nconst U: &str = \"rusty://x\";\n\
               const S: &str = \"/\";\nconst W: &str = \"word\";\n";
    let got = slashed_constants(
        [
            ("src/k.rs", src),
            ("tools/t.rs", "const T: &str = \"x/y\";\n"),
        ]
        .into_iter(),
    );
    assert_eq!(got, BTreeSet::from(["A".to_string()]));
}

#[test]
fn an_allow_with_a_reason_excuses_a_slashed_path() {
    let src = format!("// lint: allow(slash-join) an asset path is `/`-separated\n{JOIN}");
    let found = check("tests/a.rs", &src);
    assert!(allowed(&src, found[0].line, RULE));
}
