use super::{is_test_gate, test_only_files};

fn gate(attr: &str) -> bool {
    is_test_gate(&attr.lines().collect::<Vec<_>>())
}

#[test]
fn test_and_all_with_test_are_gates() {
    for attr in [
        "#[cfg(test)]",
        "    #[cfg(test)] use a::B;",
        "#[cfg(all(test, feature = \"dev\"))]",
        "#[cfg(all(feature = \"dev\", all(unix, test)))]",
        "#[cfg(all(\n    feature = \"dev\",\n    test,\n))]",
    ] {
        assert!(gate(attr), "{attr}");
    }
}

#[test]
fn any_not_and_lookalikes_are_not_gates() {
    for attr in [
        "#[cfg(any(test, feature = \"x\"))]",
        "#[cfg(any(target_os = \"macos\", test))]",
        "#[cfg(not(test))]",
        "#[cfg(all(feature = \"dev\", not(test)))]",
        "#[cfg(feature = \"testing\")]",
        "#[cfg_attr(test, derive(Debug))]",
        "fn test() {}",
    ] {
        assert!(!gate(attr), "{attr}");
    }
}

#[test]
fn a_wrapped_gate_on_a_mod_declaration_makes_its_file_test_only() {
    let root = std::env::temp_dir().join(format!("lint_test_gate_{}", std::process::id()));
    let src = root.join("src");
    for dir in ["helpers", "fixture"] {
        std::fs::create_dir_all(src.join(dir)).unwrap();
    }
    let file = |path: &str, text: &str| std::fs::write(src.join(path), text).unwrap();
    file(
        "lib.rs",
        "#[cfg(all(\n    feature = \"dev\",\n    test\n))]\nmod fixture;\n\
         #[cfg(any(test, unix))]\nmod shared;\n#[cfg(test)]\n#[path = \"helpers/support.rs\"]\nmod support;\n",
    );
    file("fixture.rs", "mod nested;\n");
    file("fixture/nested.rs", "");
    file("shared.rs", "");
    file("helpers/support.rs", "");
    let found = test_only_files(&[src.join("lib.rs")]);
    std::fs::remove_dir_all(&root).ok();
    let names: Vec<_> = found
        .iter()
        .map(|p| p.strip_prefix(&src).unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        ["fixture/nested.rs", "fixture.rs", "helpers/support.rs"].map(std::path::PathBuf::from)
    );
}
