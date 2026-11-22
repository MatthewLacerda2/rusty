use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::bake::progress::listen::listening;

pub fn token(line: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(line).ok()?;
    value.get("id")?;
    let token = value.pointer("/params/_meta/progressToken")?;
    (token.is_string() || token.is_i64() || token.is_u64()).then(|| token.clone())
}

pub fn notification(token: &Value, progress: u64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": { "progressToken": token, "progress": progress, "message": message },
    })
    .to_string()
}

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
        let _ = writeln!(out, "{}", notification(&token, heard, text)).and_then(|()| out.flush());
    };
    listening(notify, body)
}

#[derive(Clone, Default)]
pub struct Captured(Rc<RefCell<Vec<u8>>>);

impl Captured {
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
