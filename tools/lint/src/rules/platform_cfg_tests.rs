use super::{check, RULE};

fn lines(src: &str) -> Vec<usize> {
    check("src/a.rs", src).iter().map(|f| f.line).collect()
}

#[test]
fn flags_every_spelling_of_an_unshipped_platform() {
    let src = "#[cfg(windows)]\n\
               fn a() {}\n\
               #[cfg(not(unix))]\n\
               let w = cfg!(target_os = \"windows\");\n\
               #[cfg_attr(target_family = \"unix\", allow(dead_code))]\n\
               #[cfg(any(target_os = \"macos\", windows))]\n";
    assert_eq!(lines(src), [1, 3, 4, 5, 6]);
    assert_eq!(check("tests/a.rs", "#[cfg(unix)]\n")[0].rule, RULE);
    assert_eq!(check("tools/lint/src/a.rs", "#[cfg(unix)]\n").len(), 1);
}

#[test]
fn flags_a_wrapped_predicate_on_the_line_its_cfg_starts() {
    let src = "fn a() {}\n\
               #[cfg_attr(\n\
                   unix,\n\
                   allow(dead_code)\n\
               )]\n\
               #[cfg(any(\n\
                   target_os = \"macos\", // a note\n\
                   target_family = \"windows\",\n\
               ))]\n";
    assert_eq!(lines(src), [2, 6]);
}

#[test]
fn passes_other_keys_string_values_and_cfg_attrs_attributes() {
    let src = "#[cfg(feature = \"windows\")]\n\
               #[cfg_attr(test, doc = \"over a unix socket\")]\n\
               #[cfg_attr(\n\
                   feature = \"unix\",\n\
                   path = \"windows.rs\"\n\
               )]\n\
               #[cfg(target_family = \"wasm\")]\n";
    assert!(lines(src).is_empty(), "{:?}", lines(src));
}

#[test]
fn passes_real_splits_comments_strings_and_other_words() {
    let src = "#[cfg(target_os = \"macos\")]\n\
               #[cfg(not(target_os = \"linux\"))]\n\
               #[cfg(test)]\n\
               // #[cfg(windows)] is gone\n\
               let s = \"#[cfg(unix)]\";\n\
               let w = steps.windows(2);\n\
               #[cfg(feature = \"unix-socket\")]\n\
               let c = my_cfg(windows);\n";
    assert!(lines(src).is_empty(), "{:?}", lines(src));
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(platform-cfg) a reason\n#[cfg(unix)]\n";
    let found = check("src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
