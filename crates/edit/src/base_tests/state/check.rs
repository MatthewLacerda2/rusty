//! The inventory's gates as plain functions, so each has a case it must
//! flag and a case it must pass (`names`), apart from the scenes the
//! lifecycle tests build.

use rusty_scene::state::{promise, Class, Keyed, Lifecycle, Outcome};

#[derive(Clone, Copy, Debug)]
pub struct Observed {
    pub mark: &'static str,
    pub class: Class,
    pub keyed: Keyed,
    pub lifecycle: Lifecycle,
    pub outcome: Outcome,
}

/// Every observation that breaks its promise (`promise`), said in a line each. A
/// value owed nothing (`Outcome::Free`) never disagrees.
pub fn disagreements(observed: &[Observed]) -> Vec<String> {
    let broken = observed.iter().filter(|o| {
        let owed = promise(o.class, o.keyed, o.lifecycle);
        owed != Outcome::Free && owed != o.outcome
    });
    broken
        .map(|o| {
            format!(
                "{}: {:?} owes {:?} ({:?}) {:?}, and it came out {:?}",
                o.mark,
                o.lifecycle,
                o.class,
                o.keyed,
                promise(o.class, o.keyed, o.lifecycle),
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
