use super::{check, RULE};

const EDITOR: &str = "crates/editor/src/hierarchy/tree.rs";

/// The lines `check` flags in `src` at `path`, no line a test's.
fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &[]).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_dropped_call_in_the_editor_and_its_frontend() {
    let src = "fn f() {\n    let _ = save(a);\n    std::fs::create_dir_all(&d).ok();\n}\n";
    for path in [EDITOR, "crates/shell/src/editor/watch/mod.rs"] {
        let found: Vec<_> = check(path, src, &[])
            .iter()
            .map(|f| (f.line, f.rule))
            .collect();
        assert_eq!(found, [(2, RULE), (3, RULE)], "{path}");
    }
    let wrapped = "let _ =\n    x.write(\n        a,\n    );\n";
    assert_eq!(lines(EDITOR, wrapped), [1]);
    let chain = "fn f() {\n    std::fs::write(p, b)\n        .ok();\n}\n";
    assert_eq!(lines(EDITOR, chain), [3]);
}

#[test]
fn passes_a_kept_value_a_named_one_and_other_crates() {
    let kept = "let _ = (ctx, console);\nlet _ = scene;\nlet x = f().ok();\n\
                return made.ok();\nself.x = y\n    .ok();\nlet v = f()?;\n";
    assert!(lines(EDITOR, kept).is_empty());
    let commented = "// let _ = f();\nlet s = \"let _ = f();\";\n";
    assert!(lines(EDITOR, commented).is_empty());
    let dropped = "let _ = save(a);\n";
    for path in ["crates/shell/src/settings.rs", "crates/scene/src/a.rs"] {
        assert!(lines(path, dropped).is_empty(), "{path}");
    }
    assert!(check(EDITOR, dropped, &[true]).is_empty(), "a test's line");
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(dropped-result) cleanup of scratch files\nlet _ = rm(d);\n";
    let found = check(EDITOR, src, &[]);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
