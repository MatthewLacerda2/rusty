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

fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src, &[]).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_hand_written_name_and_parse() {
    assert_eq!(lines("src/a.rs", TRIO), [6]);
    let named = TRIO
        .replace("Option<Self>", "Option<Mode>")
        .replace("fn name(self)", "fn as_str(&self)");
    assert_eq!(check("src/a.rs", &named, &[])[0].rule, RULE);
    assert_eq!(lines("src/a.rs", &named), [6]);
}

#[test]
fn passes_a_lone_parse_or_name_other_types_and_tests() {
    let lone_parse = "impl Mode {\n    pub fn parse(s: &str) -> Option<Self> { None }\n}\n";
    assert!(lines("src/a.rs", lone_parse).is_empty());
    let lone_name = "impl Mode {\n    pub fn name(self) -> &'static str { \"\" }\n}\n";
    assert!(lines("src/a.rs", lone_name).is_empty());
    let result = TRIO.replace("Option<Self>", "Result<Self, String>");
    assert!(lines("src/a.rs", &result).is_empty());
    let split = "impl A {\n    fn name(self) -> &'static str { \"\" }\n}\n\
                 impl B {\n    fn parse(s: &str) -> Option<Self> { None }\n}\n";
    assert!(lines("src/a.rs", split).is_empty());
    let macro_body = TRIO.replace("impl Mode {", "        impl $Name {");
    assert!(lines("src/a.rs", &macro_body).is_empty());
    assert!(lines("tools/lint/src/a.rs", TRIO).is_empty());
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
