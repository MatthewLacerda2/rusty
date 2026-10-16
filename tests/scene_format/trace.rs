//! The tracer's driver: runs passes until every enum variant at every place in the
//! format has been walked, and collects the fingerprint lines they record.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::DeserializeOwned;

/// What `deserialize_any` answers, in order, at a place whose type is opaque to
/// serde (an untagged enum, a `serde_json::Value`): the first a pass accepts sticks.
pub const ANY_ANSWERS: usize = 3;

/// One walk of the format: builds one value, every enum at the variant `script`
/// names for its place (the first otherwise), and records what it saw.
pub struct Pass<'s> {
    pub path: String,
    pub script: &'s BTreeMap<String, usize>,
    pub any: &'s BTreeMap<String, usize>,
    /// Field places not to fill: aliases, which serde lists beside the field.
    pub aliases: &'s BTreeSet<String>,
    pub lines: BTreeSet<String>,
    /// Every enum place this pass met, with its variant count.
    pub enums: Vec<(String, usize)>,
    /// The last opaque place answered, blamed when the pass fails.
    pub last_any: Option<String>,
    /// The last field place filled, blamed when serde reports it twice.
    pub last_field: Option<String>,
}

impl Pass<'_> {
    /// Run `f` one path segment deeper.
    pub fn at<R>(&mut self, segment: &str, f: impl FnOnce(&mut Self) -> R) -> R {
        let len = self.path.len();
        self.path.push_str(segment);
        assert!(
            self.path.matches(['.', '[', '{', '<']).count() < 48,
            "the scene format recurses at {}: the tracer needs a depth rule",
            self.path
        );
        let out = f(self);
        self.path.truncate(len);
        out
    }

    /// Record `kind` at the current place.
    pub fn line(&mut self, kind: &str) {
        self.lines.insert(format!("{}: {kind}", self.path));
    }
}

/// Every fingerprint line of `T`'s read side, rooted at `root`, plus the value the
/// first pass built (fully populated: `Some` options, one-element collections).
pub fn trace<T: DeserializeOwned>(root: &str) -> (BTreeSet<String>, T) {
    let mut lines = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut any = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    let mut first = None;
    let mut scripts = vec![BTreeMap::new()];
    while let Some(script) = scripts.pop() {
        let pass = loop {
            let mut pass = Pass {
                path: root.to_string(),
                script: &script,
                any: &any,
                aliases: &aliases,
                lines: BTreeSet::new(),
                enums: Vec::new(),
                last_any: None,
                last_field: None,
            };
            match T::deserialize(&mut pass) {
                Ok(value) => break (pass.lines, pass.enums, value),
                Err(e) if e.0.starts_with("duplicate field") => {
                    let alias = pass
                        .last_field
                        .unwrap_or_else(|| panic!("trace failed: {e}"));
                    aliases.insert(alias);
                }
                Err(e) => {
                    let place = pass.last_any.unwrap_or_else(|| panic!("trace failed: {e}"));
                    let answer = any.entry(place.clone()).or_insert(0);
                    *answer += 1;
                    assert!(*answer < ANY_ANSWERS, "no answer fits {place}: {e}");
                }
            }
        };
        let (pass_lines, enums, value) = pass;
        lines.extend(pass_lines);
        first.get_or_insert(value);
        for (place, count) in enums {
            if seen.insert(place.clone()) {
                for variant in 1..count {
                    let mut next = script.clone();
                    next.insert(place.clone(), variant);
                    scripts.push(next);
                }
            }
        }
    }
    (lines, first.expect("the first pass always runs"))
}

/// The tracer's error: whatever serde reported, as text.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}
