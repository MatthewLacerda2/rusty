//! The load and editor flags: `--load-runs` a whole number above 0, a child's `--load-out`
//! and `--loads` paths, the editor round's `--editor-out`, `--editor` and `--no-editor`,
//! none of them left in the out dir.

use super::parse;

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

#[test]
fn the_load_flags_are_read_and_taken_out() {
    let a = parse(argv(&[
        "--load-runs",
        "5",
        "--loads",
        "l",
        "--load-out",
        "f",
        "out",
    ]))
    .unwrap();
    assert_eq!(a.load_runs, Some(5));
    assert_eq!(a.loads.as_deref(), Some(std::path::Path::new("l")));
    assert_eq!(a.load_out.as_deref(), Some(std::path::Path::new("f")));
    assert_eq!(a.rest, ["out"]);
    let none = parse(argv(&["out"])).unwrap();
    assert_eq!(
        (none.load_runs, none.loads, none.load_out),
        (None, None, None)
    );
}

#[test]
fn load_runs_below_one_is_refused() {
    let e = parse(argv(&["--load-runs", "0", "out"])).unwrap_err();
    assert!(
        e.contains("--load-runs wants a whole number above 0"),
        "{e}"
    );
}

#[test]
fn the_editor_flags_are_read_and_taken_out() {
    let a = parse(argv(&[
        "--no-editor",
        "--editor-out",
        "e",
        "--editor",
        "r",
        "out",
    ]))
    .unwrap();
    assert!(a.no_editor);
    assert_eq!(a.editor_out.as_deref(), Some(std::path::Path::new("e")));
    assert_eq!(a.editor.as_deref(), Some(std::path::Path::new("r")));
    assert_eq!(a.rest, ["out"]);
    let none = parse(argv(&["out"])).unwrap();
    assert!(!none.no_editor && none.editor_out.is_none() && none.editor.is_none());
}
