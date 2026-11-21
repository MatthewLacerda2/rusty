//! Every integration test of `zimmer`, in one binary (#1128).
//!
//! Each module is what used to be a test binary of its own; one binary
//! links once instead of once per file.

mod common;
mod defaults;
mod fingerprint;
mod level;
mod midi;
mod notes;
mod patch;
mod promises;
mod rendering;
mod song;
