//! src/dev/providers/ — the boundary every paid, networked call lives behind (#383).
//!
//! rusty's first relationship with the outside world beyond files on disk. The
//! client itself is scorsese's `scorsese-providers` crate (the vendors' HTTP wire,
//! dated price tables, the budget arithmetic), taken as a `dev`-only dependency the
//! way zimmer is taken (#413); this module is the adapter that holds rusty's half:
//! where a key comes from, where the project's budget lives, and when a call may go
//! out at all. Nothing else in the tree names a provider or opens a socket.
//!
//! **The rules, and why each one is a rule:**
//! - **Dev-only.** The whole module, and the dependency under it, compile only with
//!   the `dev` feature. A shipped game never calls a vendor: it plays files that
//!   were generated while authoring, the same lifecycle bakes follow.
//! - **Edit mode only.** Generation is authoring. A call during Play would make the
//!   sim depend on a network, which the determinism guard cannot see (it scans for
//!   clocks, not sockets), so [`permit`] refuses while playing.
//! - **A budget an unattended agent cannot cross.** Each project keeps a `budget`
//!   and a running `spent` ([`ledger`]); a call that would cross it is refused with
//!   both numbers and the overage. Raising the budget is one edit to that file.
//! - **Estimates, never bills.** No vendor reports what a call cost. Every figure is
//!   our arithmetic over scorsese's dated rate tables ([`prices`]), so the names say
//!   `estimate`.
//! - **No test ever makes a real call.** scorsese puts every provider behind a trait;
//!   tests here use mocks, and nothing in this module touches a network.
//!
//! Every verb that spends goes through [`permit`] first and, once the vendor has
//! answered, [`Permit::record`]s what it spent.

mod credentials;
pub mod ledger;
mod refusal;

#[cfg(test)]
mod tests;

use std::path::Path;

pub use credentials::{environment, resolve};
pub use ledger::Ledger;
pub use refusal::Refusal;
pub use scorsese_providers::credentials::{Environment, Provider, Secret};
pub use scorsese_providers::prices;

/// Leave to spend: the key to call with, and the ledger the spending is charged to.
///
/// Only [`permit`] makes one, so holding one proves the three checks passed.
#[derive(Debug)]
pub struct Permit {
    key: Secret,
    ledger: Ledger,
    estimate_cents: u64,
}

impl Permit {
    /// The key to hand the provider. Printing a [`Secret`] prints nothing.
    pub fn key(&self) -> &Secret {
        &self.key
    }

    /// Charge the estimate to the project's ledger and write it to `path`, once the
    /// vendor has answered. A call that failed before spending is not recorded.
    pub fn record(mut self, path: &Path) -> Result<Ledger, Refusal> {
        self.ledger.record(self.estimate_cents);
        self.ledger.save(path)?;
        Ok(self.ledger)
    }
}

/// Whether a call estimated at `estimate_cents` may go out now, and with what key.
///
/// Checked in the order a person would fix them: leave Play, bring a key, raise the
/// budget. `playing` is the sim's play state; `ledger_path` is the project's budget
/// file ([`ledger::path`] outside tests).
pub fn permit(
    playing: bool,
    provider: Provider,
    estimate_cents: u64,
    environment: &Environment,
    ledger_path: &Path,
) -> Result<Permit, Refusal> {
    if playing {
        return Err(Refusal::Playing);
    }
    let key = resolve(provider, environment)?;
    let ledger = Ledger::load(ledger_path)?;
    ledger.check(estimate_cents, ledger_path)?;
    Ok(Permit {
        key,
        ledger,
        estimate_cents,
    })
}
