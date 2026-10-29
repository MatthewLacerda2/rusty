//! The recorder: a [`TickTap`] that writes every Play session to a `.demo` (#1581).
//!
//! It opens a demo on the tick that enters Play, writes one line per tick that
//! changed anything (the input delta, the host acts before it, a new Game view
//! size, a digest every [`digest::EVERY`] ticks) and closes it on Stop or when the
//! world goes. Host acts come from the world's journal (`scripting::HostJournal`),
//! which it keeps recording from install on: acts made before the tick that
//! enters Play (a scenario's set-up) belong to tick 0, and acts in edit mode are
//! dropped tick by tick.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use super::delta::InputDelta;
use super::digest;
use super::files::Destination;
use super::format::{end_line, host_json, Entry, Header};
use crate::app::{GameWorld, PlayTransition, TapPoint, TickTap};
use crate::core::input::InputFrame;
use crate::scripting::HostCall;

/// One open demo.
struct Session {
    out: BufWriter<File>,
    path: PathBuf,
    /// The tick in flight, from the one that entered Play.
    t: u64,
    last: InputFrame,
    size: Option<(u32, u32)>,
    host: Vec<HostCall>,
    entry: Entry,
}

/// Records a world's Play sessions; install with [`record`](super::record).
pub struct Recorder {
    destination: Destination,
    session: Option<Session>,
    /// The demos this recorder wrote, oldest first.
    written: Vec<PathBuf>,
}

impl Recorder {
    pub(super) fn new(destination: Destination) -> Self {
        Self {
            destination,
            session: None,
            written: Vec::new(),
        }
    }

    /// The demos written (or being written) so far, oldest first.
    pub fn written(&self) -> &[PathBuf] {
        &self.written
    }

    /// Close the open demo, if any: its end line, flushed.
    pub fn finish(&mut self) {
        if let Some(mut session) = self.session.take() {
            let end = end_line(session.t);
            let wrote = writeln!(session.out, "{end}").and_then(|()| session.out.flush());
            if let Err(err) = wrote {
                log::warn!("[Demo] {}: {err}", session.path.display());
            }
        }
    }

    fn start(&mut self, world: &GameWorld) {
        let keep = world.resources.application.borrow().settings().demos.keep;
        let scene_path = world.script_manager().scene_path_cell().borrow().clone();
        let scene = scene_path.clone().unwrap_or_default();
        let Some(path) = self.destination.next(&scene, keep) else {
            return;
        };
        let mut host = Vec::new();
        match &scene_path {
            None => host.push(HostCall::Taint(
                "Play started on a scene with no file".into(),
            )),
            Some(p) if world.scene().borrow().has_unsaved_edits(p) => {
                host.push(HostCall::Taint(format!(
                    "Play started on unsaved edits to {p}"
                )));
            }
            Some(_) => {}
        }
        let header = Header {
            scene,
            engine_commit: crate::core::project::engine_commit().to_string(),
            seed: world.script_manager().play_seed(),
            settings: world.resources.application.borrow().settings(),
            storage: world.resources.storage.borrow().data(),
            size: world.resources.screen.borrow().game_view(),
        };
        // lint: allow(asset-write) a demo, read by `play --demo`, never by a path-keyed cache
        let opened = File::create(&path).map(BufWriter::new).and_then(|mut out| {
            let header = serde_json::to_string(&header)?;
            writeln!(out, "{header}").map(|()| out)
        });
        match opened {
            Ok(out) => {
                self.written.push(path.clone());
                self.session = Some(Session {
                    out,
                    path,
                    t: 0,
                    last: InputFrame::default(),
                    size: header.size,
                    host,
                    entry: Entry::default(),
                });
            }
            Err(err) => log::warn!("[Demo] {}: {err}", path.display()),
        }
    }
}

impl Session {
    /// The tick's input and size, read right before it is published.
    fn read_input(&mut self, world: &GameWorld) {
        let frame = world.input().borrow().frame();
        let size = world.resources.screen.borrow().game_view();
        let host = std::mem::take(&mut self.host);
        self.entry = Entry {
            t: self.t,
            input: InputDelta::between(&self.last, &frame),
            host: host.iter().map(host_json).collect(),
            size: (size != self.size).then_some(size).flatten(),
            digest: None,
        };
        self.last = frame;
        self.size = size;
    }

    /// The tick is done: its line written, if it has anything to say.
    fn end_tick(&mut self, world: &GameWorld) -> std::io::Result<()> {
        if digest::due(self.t) {
            self.entry.digest = Some(digest::of(world));
        }
        let entry = std::mem::take(&mut self.entry);
        if !entry.is_empty() {
            writeln!(self.out, "{}", serde_json::to_string(&entry)?)?;
        }
        if entry.digest.is_some() {
            self.out.flush()?;
        }
        self.t += 1;
        Ok(())
    }
}

impl TickTap for Recorder {
    fn at(&mut self, point: TapPoint, world: &mut GameWorld) {
        let journal = world.script_manager().journal();
        let entering = world.play_requested() && !world.scene().borrow().is_playing();
        match point {
            TapPoint::BeforeTick | TapPoint::Transition(PlayTransition::Entered) => {
                if self.session.is_none() && (entering || point != TapPoint::BeforeTick) {
                    self.start(world);
                }
                let acts = journal.borrow_mut().take();
                if let Some(session) = &mut self.session {
                    session.host.extend(acts);
                }
            }
            TapPoint::Transition(_) => self.finish(),
            TapPoint::BeforeInput => {
                if let Some(session) = &mut self.session {
                    session.read_input(world);
                }
            }
            TapPoint::AfterTick => {
                let Some(session) = &mut self.session else {
                    return;
                };
                if let Err(err) = session.end_tick(world) {
                    log::warn!("[Demo] {}: {err}", session.path.display());
                    self.session = None;
                }
            }
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.finish();
    }
}
