use super::{check, MAX_DESCRIPTION, MAX_NAME};

fn skill(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n")
}

fn problems(folder: &str, source: &str) -> Vec<String> {
    check(folder, source).into_iter().map(|(_, m)| m).collect()
}

#[test]
fn a_well_formed_frontmatter_passes() {
    assert!(problems("ci-merge", &skill("ci-merge", "Use when merging.")).is_empty());
    let quoted = "---\nname: \"a-1\"\ndescription: 'Use it.'\n---\n";
    assert!(problems("a-1", quoted).is_empty());
    let folded = "---\nname: a\ndescription: >\n  Use it\n  when asked.\n---\n";
    assert!(problems("a", folded).is_empty());
}

#[test]
fn a_missing_or_unclosed_frontmatter_fails() {
    assert_eq!(problems("a", "# A skill\n").len(), 1);
    assert_eq!(problems("a", "---\nname: a\ndescription: d\n").len(), 1);
    assert_eq!(problems("a", "").len(), 1);
}

#[test]
fn the_name_is_present_short_lowercase_and_the_folder() {
    let no_name = "---\ndescription: d\n---\n";
    assert!(problems("a", no_name)[0].contains("no `name:`"));
    let long = "a".repeat(MAX_NAME + 1);
    assert!(problems(&long, &skill(&long, "d"))[0].contains("over 64"));
    let at_cap = "a".repeat(MAX_NAME);
    assert!(problems(&at_cap, &skill(&at_cap, "d")).is_empty());
    assert!(problems("Ci_Merge", &skill("Ci_Merge", "d"))[0].contains("only `a-z`"));
    assert!(problems("ci-merge", &skill("merge", "d"))[0].contains("folder's name, `ci-merge`"));
    let line = check("ci-merge", &skill("merge", "d"))[0].0;
    assert_eq!(line, 2, "reported on the `name:` line");
}

#[test]
fn the_description_is_present_non_empty_and_short() {
    let none = "---\nname: a\n---\n";
    assert!(problems("a", none)[0].contains("no `description:`"));
    assert!(problems("a", &skill("a", ""))[0].contains("is empty"));
    assert!(problems("a", &skill("a", "\"\""))[0].contains("is empty"));
    let over = "é".repeat(MAX_DESCRIPTION + 1);
    assert!(problems("a", &skill("a", &over))[0].contains("1025 characters"));
    let at_cap = "é".repeat(MAX_DESCRIPTION);
    assert!(
        problems("a", &skill("a", &at_cap)).is_empty(),
        "characters, not bytes"
    );
}

#[test]
fn a_value_continued_on_indented_lines_counts_whole() {
    let continued = |rest: &str| format!("---\nname: a\ndescription: Use it\n  {rest}\n---\n");
    let over = "x".repeat(MAX_DESCRIPTION);
    assert!(
        problems("a", &continued(&over))[0].contains("characters, over"),
        "the continuation joins the cap"
    );
    let fits = "x".repeat(MAX_DESCRIPTION - "Use it ".len());
    assert!(problems("a", &continued(&fits)).is_empty());
    let quoted = "---\nname: a\ndescription: \"Use it\n  when asked.\"\n---\n";
    assert!(
        problems("a", quoted).is_empty(),
        "a quoted value spans lines too"
    );
}
