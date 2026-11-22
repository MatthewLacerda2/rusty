use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use super::transport;

pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    pub fn connect(explicit: Option<&str>) -> io::Result<Self> {
        let stream = dial(explicit)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            reader,
            writer: stream,
        })
    }

    pub fn request(&mut self, line: &str) -> io::Result<String> {
        self.exchange(line, &mut |_| {})
    }

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
        assert!(!address_label(None).is_empty());
    }

    #[test]
    fn connect_to_an_absent_address_is_an_io_error_not_a_panic() {
        // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is 104 bytes on macOS)
        let absent = std::env::temp_dir()
            .join(format!("rusty-client-absent-{}.sock", std::process::id()))
            .display()
            .to_string();
        assert!(Client::connect(Some(&absent)).is_err());
    }
}
