use super::{check, KEYS};

fn def(name: &str, rest: &str) -> String {
    format!("---\nname: {name}\ndescription: Use it.\n{rest}---\n\nBody.\n")
}

fn problems(stem: &str, source: &str) -> Vec<String> {
    check(stem, source).into_iter().map(|(_, m)| m).collect()
}

#[test]
fn a_model_without_a_space_after_its_colon_is_no_model() {
    let found = problems("helper", &def("helper", "model:opus\n"));
    assert!(found.iter().any(|m| m.contains("no `model:`")), "{found:?}");
}

#[test]
fn a_trailing_comment_is_cut_from_an_unquoted_model() {
    let commented = def("helper", "model: opus  # the reviewer's\n");
    assert!(problems("helper", &commented).is_empty());
    let quoted = problems("helper", &def("helper", "model: \"opus # x\"\n"));
    assert!(quoted[0].contains("is no concrete model"), "{quoted:?}");
}

#[test]
fn optional_documented_keys_and_concrete_models_pass() {
    let optional = "model: sonnet\nisolation: worktree\ndisallowedTools: Edit\n\
                    tools: Read, Grep\neffort: high\n";
    assert!(problems("helper", &def("helper", optional)).is_empty());
    for m in ["opus", "haiku", "fable", "claude-opus-5-5"] {
        let one = def("helper", &format!("model: {m}\n"));
        assert!(problems("helper", &one).is_empty(), "{m}");
    }
    let hooks = "model: opus\nhooks:\n  PreToolUse:\n    - matcher: Bash\n";
    assert!(
        problems("helper", &def("helper", hooks)).is_empty(),
        "nested lines"
    );
    assert!(KEYS.contains(&"experimental"));
}

#[test]
fn a_missing_or_inherited_model_fails() {
    assert!(problems("helper", &def("helper", ""))[0].contains("no `model:`"));
    let inherit = problems("helper", &def("helper", "model: inherit\n"));
    assert!(inherit[0].contains("`model: inherit` is no concrete model"));
}

#[test]
fn a_misspelt_key_fails_on_its_line() {
    let found = check("helper", &def("helper", "modle: opus\n"));
    let at: Vec<usize> = found.iter().map(|f| f.0).collect();
    assert!(found[0].1.contains("`modle` is no key"), "{found:?}");
    assert_eq!(at[0], 4);
    assert!(found.iter().any(|f| f.1.contains("no `model:`")));
}

#[test]
fn a_pinned_definition_on_another_model_fails() {
    let found = problems("reviewer", &def("reviewer", "model: sonnet\n"));
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("pinned to `model: opus`"));
    let full_id = problems("reviewer", &def("reviewer", "model: claude-opus-5-5\n"));
    assert!(
        full_id[0].contains("the alias and not a full id"),
        "{full_id:?}"
    );
    assert!(problems("coder", &def("coder", "model: opus\n")).is_empty());
}

#[test]
fn the_frontmatter_opens_the_file_and_names_its_stem() {
    assert!(problems("a", "# A helper\n")[0].contains("opens with `---`"));
    let other = problems("coder", &def("helper", "model: opus\n"));
    assert!(other[0].contains("must equal its file's stem, `coder`"));
    let bare = "---\nname: a\nmodel: opus\n---\n";
    assert!(problems("a", bare)[0].contains("`description:` is missing"));
}

#[test]
fn an_empty_description_is_flagged() {
    let empty = "---\nname: helper\ndescription:\nmodel: opus\n---\n";
    let found = problems("helper", empty);
    assert!(
        found.iter().any(|m| m.contains("`description:`")),
        "{found:?}"
    );
}

#[test]
fn an_unknown_key_is_reported_at_its_own_line() {
    let source = def("helper", "model: opus\nmodle: x\n");
    let at: Vec<usize> = check("helper", &source).iter().map(|(l, _)| *l).collect();
    // `---` is line 1, so the frontmatter's fourth line is the file's fifth.
    assert_eq!(at, [5]);
}
