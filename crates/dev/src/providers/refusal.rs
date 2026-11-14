//! Why a provider call did not go out. Each wall has its own variant, because each
//! has its own fix and an agent that cannot tell them apart retries the wrong one.

use std::fmt;

/// A provider call refused before it spent anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The sim is in Play. Generation is authoring: stop, then generate.
    Playing,
    /// Inside a headless run (a scenario or a bench), which is a test, and a test
    /// never calls a vendor (#1384).
    Headless,
    /// This process reads no provider keys (`credentials::read_keys`): it is not
    /// one of the binaries an author spends from, or it was started with
    /// `RUSTY_NO_PROVIDER_KEYS` set. The fix is to run the verb from one that is.
    KeysNotRead { provider: &'static str },
    /// No key in the environment or the `.env`.
    MissingKey {
        provider: &'static str,
        variable: &'static str,
        looked_in: String,
    },
    /// The call would take the project past its budget. Every figure is an
    /// estimate in US cents.
    OverBudget {
        estimate_cents: u64,
        spent_cents: u64,
        budget_cents: u64,
        over_cents: u64,
        file: String,
    },
    /// The budget file could not be read or written. Never treated as "nothing
    /// spent": a ledger that resets on a parse error is no ceiling.
    Ledger(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Playing => write!(
                f,
                "provider calls are refused during Play: generation is an edit-mode \
                 verb (stop, then generate)"
            ),
            Self::Headless => write!(
                f,
                "provider calls are refused in a headless run (a scenario or a bench): \
                 generation is authoring, and a test never calls a vendor"
            ),
            Self::KeysNotRead { provider } => write!(
                f,
                "no key for {provider}: this process does not read provider keys. Only \
                 the editor (`rusty`, built with `dev`), `session` and `session-mcp` \
                 do, and not when RUSTY_NO_PROVIDER_KEYS is set: run the verb from one \
                 of them"
            ),
            Self::MissingKey {
                provider,
                variable,
                looked_in,
            } => write!(
                f,
                "no key for {provider}: looked in {looked_in}. Set {variable} \
                 (see .env.example)"
            ),
            Self::OverBudget {
                estimate_cents,
                spent_cents,
                budget_cents,
                over_cents,
                file,
            } => write!(
                f,
                "this call would spend an estimated {estimate_cents} cents on top of \
                 {spent_cents} already spent, which is over the project's \
                 {budget_cents}-cent budget by {over_cents}. Raise `budget_cents` in \
                 {file} to go on"
            ),
            Self::Ledger(why) => write!(f, "provider budget file: {why}"),
        }
    }
}

impl std::error::Error for Refusal {}
