//! The project's budget and what it has spent, persisted beside the project (#383).
//!
//! One small JSON file, `provider_budget.json` at the project root: what the project
//! *wanted* to spend (`budget_cents`) and what it *already* spent (`spent_cents`).
//! Per project and cumulative, never per call or per session: a per-call limit is
//! the mistake scorsese made and had to fix (scorsese#237), where one overnight run
//! crossed a per-shot ceiling dozens of times over. The ceiling arithmetic itself is
//! scorsese's [`Budget`]; only the file is rusty's.

use std::path::{Path, PathBuf};

use scorsese_providers::credentials::Budget;
use serde::{Deserialize, Serialize};

use super::Refusal;

/// The budget file, relative to the project root (the working directory, #829). It
/// sits at the root, not under `cache/`: the budget is authored, not regenerable.
const FILE_NAME: &str = "provider_budget.json";

/// The budget a project starts with before anyone writes one: US$5. Low enough that
/// a runaway loop is an annoyance, high enough for a real session of barks.
pub const DEFAULT_BUDGET_CENTS: u64 = 500;

/// Where the current project keeps its ledger.
pub fn path() -> PathBuf {
    PathBuf::from(FILE_NAME)
}

/// What a project may spend, and what it has, in estimated US cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    pub budget_cents: u64,
    pub spent_cents: u64,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            budget_cents: DEFAULT_BUDGET_CENTS,
            spent_cents: 0,
        }
    }
}

impl Ledger {
    /// The ledger at `path`; the default budget with nothing spent when there is no
    /// file yet. A file that exists and does not parse is a refusal, never a reset.
    pub fn load(path: &Path) -> Result<Self, Refusal> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(Refusal::Ledger(format!("{}: {e}", path.display()))),
        };
        serde_json::from_str(&text)
            .map_err(|e| Refusal::Ledger(format!("{} does not parse: {e}", path.display())))
    }

    /// Write the ledger to `path`, creating its folder if needed.
    pub fn save(&self, path: &Path) -> Result<(), Refusal> {
        let fail = |e: &dyn std::fmt::Display| Refusal::Ledger(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| fail(&e))?;
        std::fs::write(path, json + "\n").map_err(|e| fail(&e))
    }

    /// Whether `estimate_cents` more fits under the budget. `path` only names the
    /// file in the refusal, so the fix is one copy-paste away.
    pub fn check(&self, estimate_cents: u64, path: &Path) -> Result<(), Refusal> {
        Budget::new(self.budget_cents, self.spent_cents)
            .check(estimate_cents)
            .map_err(|over| Refusal::OverBudget {
                estimate_cents: over.estimate,
                spent_cents: over.spent,
                budget_cents: over.ceiling,
                over_cents: over.over,
                file: path.display().to_string(),
            })
    }

    /// Count `cents` more as spent.
    pub fn record(&mut self, cents: u64) {
        self.spent_cents = self.spent_cents.saturating_add(cents);
    }
}
