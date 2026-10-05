//! What made a generated file: the brief, the date, the estimate (#384).
//!
//! One small JSON file beside each generated file (`<file>.json`), so a copy of the
//! project months later can still say where every generated asset came from, and
//! the brief can be read back to regenerate or adjust it. It records the brief, the
//! date and the estimated cost, never the key or the account: it travels with a copy
//! of the project, and the money does not.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The record kept beside one generated file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sidecar {
    /// The brief's kind, as in the file name.
    pub kind: String,
    /// The brief in canonical form ([`super::canonical`]); hashing it with `kind`
    /// gives back the digest in the file name.
    pub brief: Value,
    /// The UTC day the file was generated, `YYYY-MM-DD`.
    pub generated_on: String,
    /// What the call was estimated to cost, in US cents. An estimate, not a bill.
    pub estimate_cents: u64,
}

impl Sidecar {
    /// The sidecar beside the generated file at `file`.
    pub fn path_for(file: &Path) -> std::path::PathBuf {
        let mut name = file.as_os_str().to_owned();
        name.push(".json");
        name.into()
    }

    /// Read the sidecar of the generated file at `file`.
    pub fn load(file: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(Self::path_for(file))?;
        serde_json::from_str(&text).map_err(std::io::Error::other)
    }

    /// Write it beside the generated file at `file`.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(Self::path_for(file), json + "\n")
    }
}

/// Today's UTC date as `YYYY-MM-DD`. A day, not a time: when a file was made is
/// provenance, and nothing finer is worth a reader's attention.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(secs / 86_400)
}

/// `YYYY-MM-DD` for a count of days since 1970-01-01 (Howard Hinnant's
/// days-to-civil, proleptic Gregorian), so no date crate is needed for one line.
pub(crate) fn civil_date(days: u64) -> String {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
