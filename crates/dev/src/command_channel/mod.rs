pub mod client;
mod progress;
mod transport;

use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender};

use super::bake::progress::listen::listening;
use super::{console, session};
pub(super) use progress::Reply;
pub use progress::{asks, parse_progress, progress_line, ASK};
use rusty_script::scripting::{ConsoleLogs, ScriptManager};

pub(super) type Command = (String, Sender<Reply>);

pub struct CommandChannel {
    rx: Receiver<Command>,
    socket_path: std::path::PathBuf,
}

impl CommandChannel {
    pub fn start(console: &RefCell<ConsoleLogs>) -> Option<Self> {
        let bind = |tx| transport::bind_and_listen(tx, transport::socket_path());
        Self::listen(console, bind)
    }

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

    pub fn drain(&self, scripts: &ScriptManager, console: &RefCell<ConsoleLogs>) -> usize {
        let mut ran = 0;
        while let Ok((line, reply_tx)) = self.rx.try_recv() {
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
            let warnings = rusty_logging::forward(console);
            let _ = reply_tx.send(Reply::Done(session::response_line(&outcome, &warnings)));
            ran += 1;
        }
        ran
    }
}

impl Drop for CommandChannel {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests;
