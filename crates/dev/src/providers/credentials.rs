//! Where a provider key comes from: the process environment first, then a `.env`
//! at the project root or above it that fills gaps and never overrides (#383).
//!
//! scorsese's resolver also reads a per-machine settings file; rusty does not, by
//! decision: the module is `dev`-only, so a shipped build has nobody for that file
//! to serve. So only scorsese's [`Environment`] (the `.env` reader) is reused here,
//! and the miss names exactly the places rusty looked.
//!
//! **A test never holds a key (#974).** Keys are read only by a process that opted
//! in at startup ([`read_keys`]): the binaries an author drives to spend, the editor,
//! `session` and `session-mcp` (the `read-keys` lint holds both the list and that
//! each still calls it). Every other process, so every test binary of every crate
//! under any runner, gets [`environment`] with every `*_API_KEY` variable dropped and
//! no `.env`, so a test that drives the live surface with a complete brief is
//! refused instead of spending, on a machine that has a real key as much as on CI.
//! A process started with [`NO_KEYS_VAR`] set is blanked even when it opted in: every
//! test spawns an engine binary through one helper that sets it (`tests/spawn.rs`,
//! held by the `engine-spawn` lint), so the runner is never what keeps a key out.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use scorsese_providers::credentials::{Environment, Provider, Secret};

use super::Refusal;

/// Set (to anything) in a process's environment, it holds no provider key even if
/// it opted in with [`read_keys`]. A blanked [`environment`] carries it too, which
/// is how [`resolve`] tells "this process reads no keys" from "no key was set".
pub const NO_KEYS_VAR: &str = "RUSTY_NO_PROVIDER_KEYS";

/// The process environment, plus the `.env` at the project root (the working
/// directory, #829) or the nearest one above it; in a process that reads no keys
/// ([`keys_blanked`]), the process environment without its keys.
pub fn environment() -> Environment {
    if keys_blanked() {
        return without_keys(std::env::vars());
    }
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Environment::discover(&root)
}

/// Set by [`read_keys`]; nothing else writes it.
static KEYS_READ: AtomicBool = AtomicBool::new(false);

/// Lets this process read provider keys. Called first thing by the binaries an
/// author drives to spend (the editor, `session`, `session-mcp`) and by nothing
/// else: a process that never calls it, every test binary included, holds no key.
pub fn read_keys() {
    KEYS_READ.store(true, Ordering::Relaxed);
}

/// Whether [`environment`] hands out no keys in this process (#974): it never
/// opted in with [`read_keys`], or it was started with [`NO_KEYS_VAR`] set.
pub fn keys_blanked() -> bool {
    blanked(
        KEYS_READ.load(Ordering::Relaxed),
        std::env::var_os(NO_KEYS_VAR).is_some(),
    )
}

/// [`keys_blanked`] as a pure function of its two inputs, so both are testable.
pub(super) fn blanked(opted_in: bool, told_not_to: bool) -> bool {
    !opted_in || told_not_to
}

/// `vars` minus every provider key (anything named `*_API_KEY`), no `.env`, and
/// [`NO_KEYS_VAR`] set, so a miss says why.
pub(super) fn without_keys(vars: impl IntoIterator<Item = (String, String)>) -> Environment {
    let kept = vars
        .into_iter()
        .filter(|(name, _)| !is_key(name) && name != NO_KEYS_VAR);
    Environment::of(kept.chain([(NO_KEYS_VAR.to_owned(), "1".to_owned())]))
}

/// Every provider's variable ends this way; a new one that did not would slip past
/// [`without_keys`], so a test pins each [`Provider`]'s name to it.
pub(super) fn is_key(name: &str) -> bool {
    name.ends_with("_API_KEY")
}

/// `provider`'s key out of `environment`, or a refusal naming where it looked (or,
/// in a process that reads no keys, that it does not).
///
/// A pure function of the value, so the order is testable without touching the
/// process environment (global state shared with every test thread).
pub fn resolve(provider: Provider, environment: &Environment) -> Result<Secret, Refusal> {
    let variable = provider.variable();
    if let Some(key) = environment.get(variable) {
        return Ok(Secret::new(key));
    }
    if environment.get(NO_KEYS_VAR).is_some() {
        return Err(Refusal::KeysNotRead {
            provider: provider.label(),
        });
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
