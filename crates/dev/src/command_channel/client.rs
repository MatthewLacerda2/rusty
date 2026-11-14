//! crates/dev/src/command_channel/client.rs — the **dial-in** half of the windowed command
//! channel (#307).
//!
//! The listener half (`super::transport`) lets a running window *accept* line-Lua
//! commands; this is the half an external driver uses to *send* them. The MCP attach
//! backend ([`crate::mcp::attach`]) uses it to forward `eval` calls to a live
//! windowed engine, but it is a plain line client: one Lua line out, one framed JSON
//! reply line back, byte-identical to [`super::super::session::run`].
//!
//! Address resolution is shared with the listener (same env vars, same defaults via
//! `super::transport`) so a client and the window it dials never disagree about
//! where the socket is.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use super::transport;

/// A connected line client: a buffered reader half and a writer half over one
/// connection to a running engine's command socket.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    /// Dial the command socket and return a ready client. `explicit` overrides the
    /// resolved address (the `--attach <addr>` form); `None` uses the same default the
    /// listener binds. An I/O error means nothing is listening there.
    pub fn connect(explicit: Option<&str>) -> io::Result<Self> {
        let stream = dial(explicit)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            reader,
            writer: stream,
        })
    }

    /// Send one command line and block for its single framed reply line, returned
    /// trimmed of its trailing newline. EOF before a reply (the engine closed the
    /// connection) is an `UnexpectedEof` error.
    pub fn request(&mut self, line: &str) -> io::Result<String> {
        self.exchange(line, &mut |_| {})
    }

    /// [`request`](Self::request), asking for the command's bake progress lines
    /// (#1999): each goes to `progress` as it arrives, before the reply. A window
    /// from before the protocol ignores the ask and only replies.
    pub fn request_progress(
        &mut self,
        line: &str,
        progress: &mut dyn FnMut(&str),
    ) -> io::Result<String> {
        self.exchange(&format!("{}{line}", super::ASK), progress)
    }

    fn exchange(&mut self, line: &str, progress: &mut dyn FnMut(&str)) -> io::Result<String> {
        writeln!(self.writer, "{line}")?;
        self.writer.flush()?;
        loop {
            let mut reply = String::new();
            if self.reader.read_line(&mut reply)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "engine closed the connection before replying",
                ));
            }
            let reply = reply.trim_end();
            match super::parse_progress(reply) {
                Some(text) => progress(&text),
                None => return Ok(reply.to_string()),
            }
        }
    }
}

/// Resolve the address label *without* connecting — for diagnostics naming where we
/// looked. Mirrors `dial`'s resolution exactly.
pub fn address_label(explicit: Option<&str>) -> String {
    match explicit {
        Some(addr) => addr.to_string(),
        None => default_address(),
    }
}

fn default_address() -> String {
    transport::socket_path().display().to_string()
}

fn dial(explicit: Option<&str>) -> io::Result<UnixStream> {
    let path = explicit
        .map(std::path::PathBuf::from)
        .unwrap_or_else(transport::socket_path);
    UnixStream::connect(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_label_uses_the_explicit_override_verbatim() {
        assert_eq!(address_label(Some("/run/custom.sock")), "/run/custom.sock");
    }

    #[test]
    fn address_label_falls_back_to_the_shared_default() {
        // The default is non-empty and matches the listener's resolution (env-driven,
        // so we only assert it is populated, not a specific path).
        assert!(!address_label(None).is_empty());
    }

    #[test]
    fn connect_to_an_absent_address_is_an_io_error_not_a_panic() {
        // Nothing is listening here, so the dial must fail cleanly.
        // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is ~104 bytes)
        let absent = std::env::temp_dir()
            .join(format!("rusty-client-absent-{}.sock", std::process::id()))
            .display()
            .to_string();
        assert!(Client::connect(Some(&absent)).is_err());
    }
}
