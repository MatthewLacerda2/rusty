//! MCP progress notifications (#1999): a request that carries
//! `params._meta.progressToken` (the MCP spec's progress utility) hears its bake's
//! progress lines as `notifications/progress`, sent while the request runs. A
//! client such as Claude Code aborts a tool call that has sent nothing for 30
//! minutes, and `Lighting.Generate()` on a Dust2-sized scene bakes for longer;
//! whether a notification restarts that clock is the client's choice (the MCP
//! TypeScript SDK does only when its caller opts in), so `Lighting.GenerateAsync()`
//! is the reliable path there. A request without a token sends none.
//!
//! The lines come from [`listen`](crate::bake::progress::listen): an embedded
//! session's bake tells them on this thread, and the attach backend tells the
//! lines the window streams back over its socket.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::bake::progress::listen::listening;

/// The progress token a raw request line asks for, if any.
pub fn token(line: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(line).ok()?;
    value.get("id")?;
    let token = value.pointer("/params/_meta/progressToken")?;
    // The spec's token is a string or an integer.
    (token.is_string() || token.is_i64() || token.is_u64()).then(|| token.clone())
}

/// One `notifications/progress` line: the `progress`-th line heard, its text the
/// message. No total: a bake's steps say their own percentages.
pub fn notification(token: &Value, progress: u64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": { "progressToken": token, "progress": progress, "message": message },
    })
    .to_string()
}

/// Run `body` (one request) with its bake's progress lines written to `output` as
/// notifications for `token`; with no token, run it as it is.
pub fn reporting<W: Write + 'static, R>(
    token: Option<Value>,
    output: &Rc<RefCell<W>>,
    body: impl FnOnce() -> R,
) -> R {
    let Some(token) = token else {
        return body();
    };
    let (output, mut heard) = (output.clone(), 0);
    let notify = move |text: &str| {
        heard += 1;
        let mut out = output.borrow_mut();
        // A failed write shows on the response's own write, which follows.
        let _ = writeln!(out, "{}", notification(&token, heard, text)).and_then(|()| out.flush());
    };
    listening(notify, body)
}

/// A writer whose bytes its caller keeps: [`run`](super::run) owns its output for
/// the whole session (a request's progress is written mid-request), so a caller
/// that reads what was written afterwards (a test) hands it a clone of one.
#[derive(Clone, Default)]
pub struct Captured(Rc<RefCell<Vec<u8>>>);

impl Captured {
    /// Everything written so far, as text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.borrow()).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
