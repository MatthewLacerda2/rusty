//! The inventory's gates as plain functions (#1649), so each has a case it must
//! flag and a case it must pass (`names_tests`), apart from the scenes the
//! lifecycle tests build.

use super::{Class, Lifecycle, Outcome};

/// What one lifecycle did to one mark.
#[derive(Clone, Copy, Debug)]
pub struct Observed {
    /// The mark's name: its field, and the part of it when a field has two.
    pub mark: &'static str,
    /// The class the mark's part is held to.
    pub class: Class,
    pub lifecycle: Lifecycle,
    pub outcome: Outcome,
}

/// Every observation that breaks its class's promise, said in a line each. A
/// class owed nothing (`Outcome::Free`) never disagrees.
pub fn disagreements(observed: &[Observed]) -> Vec<String> {
    let broken = observed.iter().filter(|o| {
        let owed = o.class.owes(o.lifecycle);
        owed != Outcome::Free && owed != o.outcome
    });
    broken
        .map(|o| {
            format!(
                "{}: {:?} owes {:?} {:?}, and it came out {:?}",
                o.mark,
                o.lifecycle,
                o.class,
                o.class.owes(o.lifecycle),
                o.outcome
            )
        })
        .collect()
}

/// The names one list has and the other lacks, each said in a line: `listed`
/// is what a struct or a lifecycle holds, `classified` what the inventory says
/// it should.
pub fn mismatched(what: &str, listed: &[&str], classified: &[&str]) -> Vec<String> {
    let missing = listed.iter().filter(|n| !classified.contains(n));
    let extra = classified.iter().filter(|n| !listed.contains(n));
    missing
        .map(|n| format!("{what}: `{n}` is not classified for it in the inventory"))
        .chain(extra.map(|n| format!("{what}: the inventory classifies `{n}`, which it lacks")))
        .collect()
}
