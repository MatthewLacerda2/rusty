//! src/dev/command_channel/ — Windowed dev command channel (#282).
//!
//! Gives the **windowed**, rendered engine the same line-oriented Lua command
//! channel the headless `session` binary exposes, so an external process (Claude
//! Code, a bot-player, a test driver) can attach to a *running playtest* and drive
//! it through the very same evaluator the in-editor console uses
//! ([`crate::dev::console::evaluate_line`]). Windowed and headless can never drift.
//!
//! ## Single-thread constraint
//! The sim/world lives behind `Rc<RefCell<…>>` and is **not** `Send`. The listener
//! and per-connection handler threads (in `transport`) therefore **never touch the
//! world**: they only move command strings to the main loop and read framed responses
//! back. All evaluation happens on the main loop in [`CommandChannel::drain`], so sim
//! ordering and determinism are unchanged. The editor may be asleep outside Play
//! (`shell::pace`, #1721), so a handler rings [`crate::core::wake`] with each command.
//!
//! ## Transport (see `transport`)
//! A `UnixListener` at `$RUSTY_CMD_SOCK`, else `$XDG_RUNTIME_DIR/rusty.sock`, else
//! `/tmp/rusty.sock`. A stale socket file is removed before binding and (best effort)
//! on drop.
//!
//! The chosen address/path is logged to the engine console on boot so an agent can
//! find it. A bind failure is logged and the engine keeps running **without** the
//! channel — it is never fatal.
//!
//! ## Protocol
//! One Lua command per line in; one framed JSON response per command out, in lockstep
//! and **byte-identical** to the headless session (`crate::dev::session::response_line`):
//! `{"ok":true,"result":"…"}` / `{"ok":false,"error":"…"}`. Blank lines are skipped.
//! A command that asks (its line starts with `progress::ASK`, #1999) also gets its
//! bake's progress lines (`dev::bake::progress`) first, one `{"progress":"…"}`
//! line each: a line with no `ok` key is progress, and the reply is the line that
//! has one. A command that doesn't ask gets only its reply, as before.
//! A malformed line comes back as `{"ok":false,…}` and tears down neither the
//! connection nor the engine; EOF / read error ends only that connection's handler.

pub mod client;
mod progress;
mod transport;

use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender};

use super::bake::progress::listen::listening;
use super::{console, session};
use crate::script::scripting::{ConsoleLogs, ScriptManager};
pub(super) use progress::Reply;
pub use progress::{asks, parse_progress, progress_line, ASK};

/// One queued command: the input line plus the reply channel its handler thread
/// blocks on. The main loop evaluates the line and sends the framed JSON back,
/// after any progress lines a bake told while it ran.
pub(super) type Command = (String, Sender<Reply>);

/// The main-loop side of the windowed command channel. Holds the receiver the
/// listener thread feeds; [`drain`](CommandChannel::drain) is polled once per frame
/// to evaluate queued commands on the main thread and reply.
pub struct CommandChannel {
    rx: Receiver<Command>,
    /// The bound socket path — removed (best effort) on drop so a restart can
    /// rebind cleanly.
    socket_path: std::path::PathBuf,
}

impl CommandChannel {
    /// Bind the socket, spawn the listener thread, log the address, and return the
    /// channel. On bind failure the error is logged to `console` and `None` is
    /// returned — the windowed app keeps running without a command channel.
    pub fn start(console: &RefCell<ConsoleLogs>) -> Option<Self> {
        let bind = |tx| transport::bind_and_listen(tx, transport::socket_path());
        Self::listen(console, bind)
    }

    /// [`start`](Self::start) over a given transport bind.
    fn listen(
        console: &RefCell<ConsoleLogs>,
        bind: impl FnOnce(Sender<Command>) -> std::io::Result<transport::Bound>,
    ) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Command>();
        match bind(tx) {
            Ok(bound) => {
                console.borrow_mut().info(format!(
                    "[cmd] command channel listening on {}",
                    bound.label
                ));
                Some(Self {
                    rx,
                    socket_path: bound.path,
                })
            }
            Err(err) => {
                console
                    .borrow_mut()
                    .error(format!("[cmd] command channel disabled: {}", err));
                None
            }
        }
    }

    /// Evaluate every currently-queued command on the main thread and reply.
    ///
    /// `try_recv`s all pending `(line, reply_tx)` pairs (never blocks — keeps this a
    /// once-per-frame poll), runs each through [`console::evaluate_line`] against the
    /// live world, frames the outcome with `session::response_line` (identical to
    /// the headless session), and sends it back. Send errors are ignored — the client
    /// may have disconnected between sending and our reply. Returns how many ran.
    pub fn drain(&self, scripts: &ScriptManager, console: &RefCell<ConsoleLogs>) -> usize {
        let mut ran = 0;
        while let Ok((line, reply_tx)) = self.rx.try_recv() {
            // A bake's progress lines go to a client that asked while the line
            // runs (#1999); one that didn't gets only the reply.
            let (asked, line) = asks(&line);
            let evaluate = || console::evaluate_line(scripts, console, line);
            let outcome = match asked {
                true => {
                    let progress = reply_tx.clone();
                    let tell = move |text: &str| {
                        let _ = progress.send(Reply::Progress(text.to_string()));
                    };
                    listening(tell, evaluate)
                }
                false => evaluate(),
            };
            let warnings = crate::logging::forward(console);
            let _ = reply_tx.send(Reply::Done(session::response_line(&outcome, &warnings)));
            ran += 1;
        }
        ran
    }
}

impl Drop for CommandChannel {
    fn drop(&mut self) {
        // Best-effort cleanup so a restart can rebind to the same path.
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests;
