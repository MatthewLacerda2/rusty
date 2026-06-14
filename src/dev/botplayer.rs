//! src/dev/botplayer.rs — Bot-player pattern (notes, not a subsystem)
//!
//! A bot-player is NOT special engine code. It is a normal scene script, tagged
//! dev-only ("won't ship"), attached to the Player, that drives WRITABLE Input from
//! its Update() — i.e. it presses the same keys a human would. Run it headless via
//! the harness at max speed and read the summary (won/lost, time, health, errors).
//!
//! This file exists only to document the pattern and host any shared bot helpers;
//! the actual bots live in project/scripts as .lua, like any other script.
//!
//! Allowed deps: api (writable Input).
//! Status: SCAFFOLD — structure only; not yet implemented.
