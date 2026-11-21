//! A Standard MIDI File read as a song: what maps to what, what is refused,
//! and what is said about the rest.
//!
//! Every fixture is written byte by byte in `file`, rather than committed as a
//! `.mid` or produced by the parser's own writer: a fixture that the code
//! under test also encoded could agree with it about a mistake.

mod export;
mod file;
mod parts;
mod refusals;
mod timing;
