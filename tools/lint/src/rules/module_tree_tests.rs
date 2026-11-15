use super::{reached, Reach};

/// What `src/lib.rs` reaches when every candidate file exists.
fn reach(text: &str) -> Vec<Reach> {
    reached("src/lib.rs", true, text, |_| true)
}

fn module(path: &str, mod_rs: bool) -> Reach {
    Reach::Module {
        path: path.into(),
        mod_rs,
    }
}

fn first(text: &str) -> Reach {
    reach(text).into_iter().next().expect("reaches something")
}

#[test]
fn a_block_inside_an_inline_module_keeps_the_module_open() {
    let text = "mod a {\n    fn f() {}\n    mod b;\n}\nmod c;\n";
    assert_eq!(
        reach(text),
        [
            module("src/a/b.rs", false),
            module("src/a/b/mod.rs", true),
            module("src/c.rs", false),
            module("src/c/mod.rs", true),
        ]
    );
}

#[test]
fn a_path_ends_with_the_item_it_sits_on() {
    let text = "#[path = \"x.rs\"]\nconst A: u8 = 1;\nmod z;\n";
    assert_eq!(first(text), module("src/z.rs", false));
}

#[test]
fn an_inner_attribute_s_include_is_read() {
    let text = "#![doc = include_str!(\"../README.md\")]\n";
    assert_eq!(reach(text), [Reach::Included("README.md".into())]);
}

#[test]
fn a_path_inside_a_cfg_attr_predicate_is_none() {
    let text = "#[cfg_attr(any(a, path = \"no.rs\"), allow(dead_code))]\nmod m;\n";
    assert_eq!(first(text), module("src/m.rs", false));
}

#[test]
fn only_include_and_include_str_name_a_file() {
    let text = "const A: &str = concat!(\"a.rs\");\nconst B: &str = env!(\"b.rs\");\n";
    assert_eq!(reach(text), []);
}

#[test]
fn a_literal_unescapes_unless_it_is_raw() {
    let text = "const A: &str = include_str!(\"x\\\\y.txt\");\n\
                const B: &str = include_str!(r\"p\\q.txt\");\n";
    assert_eq!(
        reach(text),
        [
            Reach::Included("src/x\\y.txt".into()),
            Reach::Included("src/p\\q.txt".into()),
        ]
    );
}

#[test]
fn a_malformed_file_ends_the_scan() {
    // an attribute never closed, a lone `#`, a `mod` cut short: each ends cleanly
    for text in [
        "#[path = \"x.rs\"\nmod a;\n",
        "mod a;\n#",
        "#!",
        "mod",
        "mod a",
    ] {
        let _ = reach(text);
    }
    assert_eq!(first("mod a;\n#[path"), module("src/a.rs", false));
}

#[test]
fn an_attribute_ends_at_its_own_bracket() {
    // a predicate's own parentheses close before the `path`
    let nested = "#[cfg_attr(any(a), path = \"x.rs\")]\nmod m;\n";
    assert_eq!(first(nested), module("src/x.rs", true));
    // and the next `#[path]` is that `mod`'s, not this one's
    let two = "#[path = \"x.rs\"]\nmod m;\n#[path = \"y.rs\"]\nmod n;\n";
    let paths: Vec<Reach> = reach(two);
    assert_eq!(paths, [module("src/x.rs", true), module("src/y.rs", true)]);
}
