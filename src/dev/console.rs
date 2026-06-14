//! src/dev/console.rs — Console: log output + live Lua REPL
//!
//! Two responsibilities behind one type:
//!   1. Log sink — buffers info/warn/error lines (exists today as ConsoleLogs).
//!   2. REPL     — evaluates a line of Lua against the LIVE runtime and returns the
//!                 result. The SAME evaluator backs both the in-editor terminal panel
//!                 (type while the game plays) and the headless harness (scenario
//!                 lines). There is exactly one evaluator, so windowed and headless
//!                 can never drift.
//!
//! "Call the API live" == feed a string to this evaluator.
//!
//! Allowed deps: scripting (mlua), api.
//! Status: SCAFFOLD — structure only; not yet implemented.
