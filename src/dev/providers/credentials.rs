//! Where a provider key comes from: the process environment first, then a `.env`
//! at the project root or above it that fills gaps and never overrides (#383).
//!
//! scorsese's resolver also reads a per-machine settings file; rusty does not, by
//! decision: the module is `dev`-only, so a shipped build has nobody for that file
//! to serve. So only scorsese's [`Environment`] (the `.env` reader) is reused here,
//! and the miss names exactly the places rusty looked.
//!
//! **A test never holds a key (#974).** Inside a test process [`environment`] drops
//! every `*_API_KEY` variable and skips the `.env`, so a test that drives the live
//! surface with a complete brief is refused with "no key" instead of spending, on a
//! machine that has a real key as much as on CI. A test process is a unit-test build
//! (`cfg(test)`) or any process nextest starts (it sets `NEXTEST`, which reaches the
//! integration binary, where `cfg(test)` is off for the library).

use std::path::PathBuf;

use scorsese_providers::credentials::{Environment, Provider, Secret};

use super::Refusal;

/// The process environment, plus the `.env` at the project root (the working
/// directory, #829) or the nearest one above it; inside a test, the process
/// environment without its keys ([`keys_blanked`]).
pub fn environment() -> Environment {
    if keys_blanked() {
        return without_keys(std::env::vars());
    }
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Environment::discover(&root)
}

/// Whether this process is a test, so [`environment`] hands out no keys (#974).
pub fn keys_blanked() -> bool {
    cfg!(test) || std::env::var_os("NEXTEST").is_some()
}

/// `vars` minus every provider key (anything named `*_API_KEY`), and no `.env`.
pub(super) fn without_keys(vars: impl IntoIterator<Item = (String, String)>) -> Environment {
    Environment::of(vars.into_iter().filter(|(name, _)| !is_key(name)))
}

/// Every provider's variable ends this way; a new one that did not would slip past
/// [`without_keys`], so a test pins each [`Provider`]'s name to it.
pub(super) fn is_key(name: &str) -> bool {
    name.ends_with("_API_KEY")
}

/// `provider`'s key out of `environment`, or a refusal naming where it looked.
///
/// A pure function of the value, so the order is testable without touching the
/// process environment (global state shared with every test thread).
pub fn resolve(provider: Provider, environment: &Environment) -> Result<Secret, Refusal> {
    let variable = provider.variable();
    if let Some(key) = environment.get(variable) {
        return Ok(Secret::new(key));
    }
    let mut looked_in = format!("the {variable} variable");
    match environment.dotenv() {
        Some(dotenv) => looked_in.push_str(&format!(", {}", dotenv.display())),
        None => looked_in.push_str(", a .env at the project root or above (there is none)"),
    }
    Err(Refusal::MissingKey {
        provider: provider.label(),
        variable,
        looked_in,
    })
}
