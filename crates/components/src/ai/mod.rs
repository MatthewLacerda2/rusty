//! crates/components/src/ai/ — the AI components (#1900): what an agent senses.
//!
//! The senses that fill them run in `ai::perception`; the components hold the
//! authored settings and the agent's memory of what it sensed.

pub mod memory;
pub mod perception;
