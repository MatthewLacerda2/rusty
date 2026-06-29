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
//! and per-connection handler threads (in [`transport`]) therefore **never touch the
//! world**: they only move command strings to the main loop and read framed responses
//! back. All evaluation happens on the main loop in [`CommandChannel::drain`], so sim
//! ordering and determinism are unchanged.
//!
//! ## Transport (cross-platform — see [`transport`])
//! - **unix:** a `UnixListener` at `$RUSTY_CMD_SOCK`, else `$XDG_RUNTIME_DIR/rusty.sock`,
//!   else `/tmp/rusty.sock`. A stale socket file is removed before binding and (best
//!   effort) on drop.
//! - **windows:** a `TcpListener` on `$RUSTY_CMD_ADDR`, else `127.0.0.1:8787`. Std only —
//!   no named-pipe crate, no new dependency.
//!
//! The chosen address/path is logged to the engine console on boot so an agent can
//! find it. A bind failure is logged and the engine keeps running **without** the
//! channel — it is never fatal.
//!
//! ## Protocol
//! One Lua command per line in; one framed JSON response per command out, in lockstep
//! and **byte-identical** to the headless session ([`crate::dev::session::response_line`]):
//! `{"ok":true,"result":"…"}` / `{"ok":false,"error":"…"}`. Blank lines are skipped.
//! A malformed line comes back as `{"ok":false,…}` and tears down neither the
//! connection nor the engine; EOF / read error ends only that connection's handler.

pub mod client;
mod transport;

use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender};

use super::{console, session};
use crate::scripting::{ConsoleLogs, ScriptManager};

/// One queued command: the input line plus the reply channel its handler thread
/// blocks on. The main loop evaluates the line and sends the framed JSON back.
pub(super) type Command = (String, Sender<String>);

/// The main-loop side of the windowed command channel. Holds the receiver the
/// listener thread feeds; [`drain`](CommandChannel::drain) is polled once per frame
/// to evaluate queued commands on the main thread and reply.
pub struct CommandChannel {
    rx: Receiver<Command>,
    /// On unix, the bound socket path — removed (best effort) on drop so a restart
    /// can rebind cleanly. Unused on windows (TCP needs no filesystem cleanup).
    #[cfg(unix)]
    socket_path: std::path::PathBuf,
}

impl CommandChannel {
    /// Bind the socket, spawn the listener thread, log the address, and return the
    /// channel. On bind failure the error is logged to `console` and `None` is
    /// returned — the windowed app keeps running without a command channel.
    pub fn start(console: &RefCell<ConsoleLogs>) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Command>();
        match transport::bind_and_listen(tx) {
            Ok(bound) => {
                console.borrow_mut().info(format!(
                    "[cmd] command channel listening on {}",
                    bound.label
                ));
                Some(Self {
                    rx,
                    #[cfg(unix)]
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
    /// live world, frames the outcome with [`session::response_line`] (identical to
    /// the headless session), and sends it back. Send errors are ignored — the client
    /// may have disconnected between sending and our reply.
    pub fn drain(&self, scripts: &ScriptManager, console: &RefCell<ConsoleLogs>) {
        while let Ok((line, reply_tx)) = self.rx.try_recv() {
            let outcome = console::evaluate_line(scripts, console, &line);
            let _ = reply_tx.send(session::response_line(&outcome));
        }
    }
}

#[cfg(unix)]
impl Drop for CommandChannel {
    fn drop(&mut self) {
        // Best-effort cleanup so a restart can rebind to the same path.
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::session::Session;

    fn session() -> Session {
        Session::new("").expect("session boots in edit mode")
    }

    /// Build a channel whose listener side is unused: we drive the receiver directly
    /// by feeding `(line, reply_tx)` pairs, exercising the core `drain` logic against
    /// a live world without a real socket. Returns the sender + the channel.
    fn local_channel() -> (Sender<Command>, CommandChannel) {
        let (tx, rx) = mpsc::channel::<Command>();
        let channel = CommandChannel {
            rx,
            #[cfg(unix)]
            socket_path: std::path::PathBuf::from("/tmp/rusty-test-unused.sock"),
        };
        (tx, channel)
    }

    /// Send one line through the channel, drain it against the live world, and return
    /// the single framed reply the handler would have written back.
    fn round_trip(s: &Session, tx: &Sender<Command>, ch: &CommandChannel, line: &str) -> String {
        let (reply_tx, reply_rx) = mpsc::channel::<String>();
        tx.send((line.to_string(), reply_tx))
            .expect("queue command");
        ch.drain(s.world().script_manager(), s.world().console());
        reply_rx.recv().expect("drain replies once")
    }

    #[test]
    fn drain_frames_a_successful_command() {
        let s = session();
        let (tx, ch) = local_channel();
        let reply = round_trip(&s, &tx, &ch, "1 + 1");
        assert!(reply.contains("\"ok\":true") && reply.contains("\"result\":\"2\""));
    }

    #[test]
    fn drain_round_trips_a_setter_and_getter_against_the_live_world() {
        let s = session();
        s.world()
            .scene()
            .borrow_mut()
            .add_entity("Probe".to_string());
        let (tx, ch) = local_channel();

        let id_reply = round_trip(&s, &tx, &ch, "Scene.FindEntityByName(\"Probe\")");
        // Pull the rendered id out of the framed reply for the follow-up commands.
        let id: serde_json::Value = serde_json::from_str(&id_reply).unwrap();
        let id = id["result"].as_str().unwrap().to_string();
        assert_ne!(id, "0", "the live scene must resolve the authored entity");

        round_trip(
            &s,
            &tx,
            &ch,
            &format!("Transform.SetPosition({}, 1, 2, 3)", id),
        );
        let pos = round_trip(&s, &tx, &ch, &format!("Transform.GetPosition({})", id));
        assert!(
            pos.contains("\"ok\":true") && pos.contains("\"result\":\"1, 2, 3\""),
            "the setter must persist on the live world: {pos}"
        );
    }

    #[test]
    fn malformed_line_frames_an_error_and_channel_survives() {
        let s = session();
        let (tx, ch) = local_channel();

        let bad = round_trip(&s, &tx, &ch, "this is not lua %%%");
        assert!(
            bad.contains("\"ok\":false") && bad.contains("\"error\""),
            "a malformed line must frame an error: {bad}"
        );

        // A subsequent good command still works: bad input never breaks the channel.
        let good = round_trip(&s, &tx, &ch, "1 + 1");
        assert!(good.contains("\"ok\":true") && good.contains("\"result\":\"2\""));
    }

    #[test]
    fn drain_replies_once_per_queued_command_in_order() {
        let s = session();
        let (tx, ch) = local_channel();
        let (a_tx, a_rx) = mpsc::channel::<String>();
        let (b_tx, b_rx) = mpsc::channel::<String>();
        tx.send(("1 + 1".to_string(), a_tx)).unwrap();
        tx.send(("2 + 2".to_string(), b_tx)).unwrap();

        // One drain call clears the whole queue — both replies are ready after it.
        ch.drain(s.world().script_manager(), s.world().console());
        assert!(a_rx.recv().unwrap().contains("\"result\":\"2\""));
        assert!(b_rx.recv().unwrap().contains("\"result\":\"4\""));
    }

    #[cfg(unix)]
    #[test]
    fn start_binds_a_unix_socket_then_drops_cleanly() {
        // Point the channel at an isolated path so the test never collides with a
        // running engine or another test.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("rusty-cmd-test-{}.sock", std::process::id()));
        // SAFETY: single-threaded test setup; no other thread reads the env here.
        unsafe { std::env::set_var("RUSTY_CMD_SOCK", &path) };

        let console = RefCell::new(ConsoleLogs::new());
        let channel = CommandChannel::start(&console).expect("bind the unix socket");
        assert!(path.exists(), "start() must create the socket file");

        drop(channel);
        assert!(!path.exists(), "drop must remove the socket file");
        unsafe { std::env::remove_var("RUSTY_CMD_SOCK") };
    }
}
