//! Where a provider key comes from: the process environment first, then a `.env`
//! at the project root or above it that fills gaps and never overrides (#383).
//!
//! scorsese's resolver also reads a per-machine settings file; rusty does not, by
//! decision: the module is `dev`-only, so a shipped build has nobody for that file
//! to serve. So only scorsese's [`Environment`] (the `.env` reader) is reused here,
//! and the miss names exactly the places rusty looked.

use std::path::PathBuf;

use scorsese_providers::credentials::{Environment, Provider, Secret};

use super::Refusal;

/// The process environment, plus the `.env` at the project root (the working
/// directory, #829) or the nearest one above it.
pub fn environment() -> Environment {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Environment::discover(&root)
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
