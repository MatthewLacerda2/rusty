use super::{check, RULE};

const TRIO: &str = "\
impl Mode {
    pub fn name(self) -> &'static str {
        match self { Self::A => \"A\" }
    }

    pub fn parse(name: &str) -> Option<Self> {
        None
    }
}
";

const FREE: &str = "\
pub fn mode_name(m: Mode) -> &'static str {
    \"A\"
}

pub fn parse_mode(name: &str) -> Option<Mode> {
    None
}
";

fn lines(src: &str) -> Vec<usize> {
    check("src/a.rs", src, &[]).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_hand_written_name_and_parse() {
    assert_eq!(lines(TRIO), [6]);
    assert_eq!(check("src/a.rs", TRIO, &[])[0].rule, RULE);
    let named = TRIO
        .replace("Option<Self>", "Option<Mode>")
        .replace("fn name(self)", "fn as_str(&self)");
    assert_eq!(lines(&named), [6]);
}

#[test]
fn flags_free_parse_and_name_functions() {
    assert_eq!(lines(FREE), [5]);
    let mixed = "impl Mode {\n    fn name(self) -> &'static str { \"\" }\n}\n\
                 fn parse_mode(s: &str) -> Option<Mode> { None }\n";
    assert_eq!(lines(mixed), [4]);
}

#[test]
fn flags_split_indented_and_commented_impl_blocks() {
    let split = "impl Mode {\n    fn name(self) -> &'static str { \"\" }\n}\n\n\
                 impl Mode {\n    fn parse(s: &str) -> Option<Self> { None }\n}\n";
    assert_eq!(lines(split), [6]);
    let inline_mod = "mod m {\n    impl Mode { // the names\n        \
                      fn name(self) -> &'static str { \"\" }\n        \
                      fn parse(s: &str) -> Option<Self> { None }\n    }\n}\n";
    assert_eq!(lines(inline_mod), [4]);
}

#[test]
fn passes_a_lone_parse_or_name_other_types_and_shapes() {
    assert!(
        lines("impl Mode {\n    pub fn parse(s: &str) -> Option<Self> { None }\n}\n").is_empty()
    );
    assert!(lines("impl Mode {\n    pub fn name(self) -> &'static str { \"\" }\n}\n").is_empty());
    assert!(lines(&TRIO.replace("Option<Self>", "Result<Self, String>")).is_empty());
    let two_types = "impl A {\n    fn name(self) -> &'static str { \"\" }\n}\n\
                     impl B {\n    fn parse(s: &str) -> Option<Self> { None }\n}\n";
    assert!(lines(two_types).is_empty());
    let free_two_types = FREE.replace("Option<Mode>", "Option<Kind>");
    assert!(lines(&free_two_types).is_empty());
    let two_args = FREE.replace("(name: &str)", "(name: &str, size: f32)");
    assert!(lines(&two_args).is_empty());
    let braces_in_strings = "impl A {\n    fn f() { let _ = \"}}\"; }\n}\n\
                             impl B {\n    fn name(self) -> &'static str { \"\" }\n}\n\
                             fn parse_a(s: &str) -> Option<A> { None }\n";
    assert!(lines(braces_in_strings).is_empty());
}

#[test]
fn passes_the_macro_body_other_trees_and_tests() {
    assert!(lines(&TRIO.replace("impl Mode {", "        impl $Name {")).is_empty());
    assert!(check("tools/lint/src/a.rs", TRIO, &[]).is_empty());
    let all_test = vec![true; TRIO.lines().count()];
    assert!(check("src/a.rs", TRIO, &all_test).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = TRIO.replace(
        "    pub fn parse",
        "    // lint: allow(named-enum) reads aliases with spaces ignored\n    pub fn parse",
    );
    let found = check("src/a.rs", &src, &[]);
    assert!(crate::rules::allowed(&src, found[0].line, RULE));
}
