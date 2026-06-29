//! src/dev/command_channel/client.rs — the **dial-in** half of the windowed command
//! channel (#307).
//!
//! The listener half ([`super::transport`]) lets a running window *accept* line-Lua
//! commands; this is the half an external driver uses to *send* them. The MCP attach
//! backend ([`crate::dev::mcp::attach`]) uses it to forward `eval` calls to a live
//! windowed engine, but it is a plain line client: one Lua line out, one framed JSON
//! reply line back, byte-identical to [`super::super::session::run`].
//!
//! Address resolution is shared with the listener (same env vars, same defaults via
//! [`super::transport`]) so a client and the window it dials never disagree about
//! where the socket is.

use std::io::{self, BufRead, BufReader, Write};

use super::transport;

/// The platform connection type: a unix domain socket on unix, a loopback TCP stream
/// on windows — mirroring the listener's two transports.
#[cfg(unix)]
type RawStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type RawStream = std::net::TcpStream;

/// A connected line client: a buffered reader half and a writer half over one
/// connection to a running engine's command socket.
pub struct Client {
    reader: BufReader<RawStream>,
    writer: RawStream,
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
        writeln!(self.writer, "{line}")?;
        self.writer.flush()?;
        let mut reply = String::new();
        if self.reader.read_line(&mut reply)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "engine closed the connection before replying",
            ));
        }
        Ok(reply.trim_end().to_string())
    }
}

/// Resolve the address label *without* connecting — for diagnostics naming where we
/// looked. Mirrors [`dial`]'s resolution exactly.
pub fn address_label(explicit: Option<&str>) -> String {
    match explicit {
        Some(addr) => addr.to_string(),
        None => default_address(),
    }
}

#[cfg(unix)]
fn default_address() -> String {
    transport::socket_path().display().to_string()
}
#[cfg(windows)]
fn default_address() -> String {
    transport::tcp_addr()
}

#[cfg(unix)]
fn dial(explicit: Option<&str>) -> io::Result<RawStream> {
    let path = explicit
        .map(std::path::PathBuf::from)
        .unwrap_or_else(transport::socket_path);
    RawStream::connect(&path)
}
#[cfg(windows)]
fn dial(explicit: Option<&str>) -> io::Result<RawStream> {
    let addr = explicit
        .map(str::to_string)
        .unwrap_or_else(transport::tcp_addr);
    RawStream::connect(&addr)
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
        #[cfg(unix)]
        let absent = std::env::temp_dir()
            .join(format!("rusty-client-absent-{}.sock", std::process::id()))
            .display()
            .to_string();
        #[cfg(windows)]
        let absent = "127.0.0.1:1".to_string();
        assert!(Client::connect(Some(&absent)).is_err());
    }
}
