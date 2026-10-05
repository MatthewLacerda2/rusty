//! A generated asset is addressed by the hash of the brief that made it (#384).
//!
//! ```text
//! <project>/assets/generated/<kind>-<digest of the brief>.<ext>
//! <project>/assets/generated/<kind>-<digest of the brief>.<ext>.json   (the sidecar)
//! ```
//!
//! Asking for the same thing twice finds the file already there and spends nothing,
//! so a prompt rewritten ten times pays once per distinct wording, and putting a
//! word back is free. One convention for every endpoint: speech, sound effects and
//! music are three [`Brief`]s and one lifecycle.
//!
//! **The lifecycle** ([`GenerationState`]): `Sketch` (a brief with nothing generated
//! yet: the ordinary state of a line somebody is still writing, never an error) →
//! `Generated` (the file is there) → `Stale` (the brief changed since). A changed
//! brief marks its asset stale; it never regenerates on its own, because the agent
//! chooses when to spend.
//!
//! **The order every generating verb follows** ([`realise`]): the cache check
//! first, so a hit never resolves a key or touches the budget; then the provider
//! call (behind [`super::permit`]); then [`store`] the file and its [`Sidecar`];
//! then [`super::Permit::record`].

mod brief;
mod sidecar;

use std::path::{Path, PathBuf};

pub use brief::{canonical, digest, digest_of, fingerprint, Brief};
#[cfg(test)]
pub(crate) use sidecar::civil_date;
pub use sidecar::{today, Sidecar};

/// Where generated assets live, relative to the project root, which is the working
/// directory (#829). The one place this module names it. Whether a game's repo
/// versions these files is still open (#866).
pub const GENERATED_DIR: &str = "assets/generated";

/// Where a brief's output lives, relative to the working directory.
pub fn address<B: Brief + ?Sized>(brief: &B) -> serde_json::Result<PathBuf> {
    let name = format!("{}-{}.{}", brief.kind(), digest(brief)?, brief.extension());
    Ok(Path::new(GENERATED_DIR).join(name))
}

/// Where a generated asset stands. No `queued` and no `expired`: none of the
/// endpoints is long-running, so a state for work in flight would only be a state
/// every reader has to rule out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationState {
    /// Nothing generated for this brief, nor for an earlier one. Plays silence and
    /// spends nothing until someone generates it.
    Sketch,
    /// The file for this exact brief is there: a cache hit, never billed again.
    Generated,
    /// A file exists for an earlier version of the brief, but not for this one.
    /// It stays playable until someone chooses to pay for the new one.
    Stale,
}

/// Where `brief` stands under `root`, given `last`: the address the document
/// recorded when it last generated (project-relative, as [`address`] returns).
///
/// A brief edited back to an earlier wording finds that wording's file again, so a
/// revert is `Generated`, not `Stale`.
pub fn state<B: Brief + ?Sized>(
    root: &Path,
    brief: &B,
    last: Option<&Path>,
) -> serde_json::Result<GenerationState> {
    if root.join(address(brief)?).is_file() {
        return Ok(GenerationState::Generated);
    }
    let earlier = last.is_some_and(|path| root.join(path).is_file());
    Ok(if earlier {
        GenerationState::Stale
    } else {
        GenerationState::Sketch
    })
}

/// The address of `brief`'s output if it is already under `root`; `None` means
/// asking for it would cost money.
pub fn cached<B: Brief + ?Sized>(root: &Path, brief: &B) -> serde_json::Result<Option<PathBuf>> {
    let address = address(brief)?;
    Ok(root.join(&address).is_file().then_some(address))
}

/// Write `bytes` as `brief`'s output under `root`, with its sidecar, and return the
/// address. `estimate_cents` is what the call was estimated to cost.
pub fn store<B: Brief + ?Sized>(
    root: &Path,
    brief: &B,
    bytes: &[u8],
    estimate_cents: u64,
    generated_on: &str,
) -> std::io::Result<PathBuf> {
    let address = address(brief).map_err(std::io::Error::other)?;
    let file = root.join(&address);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, bytes)?;
    let sidecar = Sidecar {
        kind: brief.kind().to_owned(),
        brief: canonical(brief).map_err(std::io::Error::other)?,
        generated_on: generated_on.to_owned(),
        estimate_cents,
    };
    sidecar.save(&file)?;
    Ok(address)
}

/// What [`realise`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Realised {
    /// The file was already there; nothing was called and nothing spent.
    Cached(PathBuf),
    /// The provider was called and its output stored here.
    Generated(PathBuf),
}

/// One generation, start to finish: return the cached file if there is one;
/// otherwise call `generate` (which goes through [`super::permit`], calls the
/// provider and records the spend) for the bytes and their estimate in cents, and
/// [`store`] them.
pub fn realise<B, E>(
    root: &Path,
    brief: &B,
    generated_on: &str,
    generate: impl FnOnce(&B) -> Result<(Vec<u8>, u64), E>,
) -> Result<Realised, E>
where
    B: Brief + ?Sized,
    E: From<std::io::Error>,
{
    let io = |e: serde_json::Error| E::from(std::io::Error::other(e));
    if let Some(address) = cached(root, brief).map_err(io)? {
        return Ok(Realised::Cached(address));
    }
    let (bytes, estimate_cents) = generate(brief)?;
    let address = store(root, brief, &bytes, estimate_cents, generated_on)?;
    Ok(Realised::Generated(address))
}
