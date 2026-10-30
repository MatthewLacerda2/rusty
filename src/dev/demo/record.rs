//! The recorder: a [`TickTap`] that writes every Play session to a `.demo` (#1581).
//!
//! It opens a demo right before Play is entered, however it is entered (a tick, a
//! paused frame's poll, `Editor.EnterPlay`): the header is read before `Awake` and
//! `Start` run, and the host acts made since the last tick (a scenario's set-up)
//! go in it as `before_play`. Then it writes one line per tick that changed
//! anything (the input delta, the `dt`, the host acts before it, a new Game view
//! size, a digest every [`digest::EVERY`] ticks), and right before Play is left
//! (or the run ends, [`Recorder::stop`]) the end line: the acts after the last
//! tick and a final digest. Host acts come from the world's journal
//! (`scripting::HostJournal`); acts in edit mode are dropped frame by frame,
//! ticked or paused, and `Editor.EnterPlay` / `Editor.ExitPlay` are never kept:
//! the transition they ask for is the demo's start or end.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use super::delta::InputDelta;
use super::digest;
use super::files::Destination;
use super::format::{host_json, End, Entry, Exact, Header};
use super::sources::Sources;
use crate::app::{GameWorld, PlayTransition, TapPoint, TickTap};
use crate::core::input::InputFrame;
use crate::dev::harness::FIXED_DT;
use crate::scripting::HostCall;

/// One open demo.
struct Session {
    out: BufWriter<File>,
    path: PathBuf,
    /// The tick in flight, from the first after Play was entered.
    t: u64,
    last: InputFrame,
    last_dt: f32,
    size: Option<(u32, u32)>,
    host: Vec<HostCall>,
    entry: Entry,
}

/// Records a world's Play sessions; install with [`record`](super::record).
pub struct Recorder {
    destination: Destination,
    session: Option<Session>,
    /// Whether the scene had no unsaved edits when the recorder was installed,
    /// while no tick has passed since: acts made then (a scenario's set-up) are
    /// replayed, so they are no unsaved edit of their own.
    clean_at_install: Option<bool>,
    /// The demos this recorder wrote, oldest first.
    written: Vec<PathBuf>,
}

/// Whether `world`'s scene is its file's, as saved; `None` without a file.
fn is_saved(world: &GameWorld) -> Option<bool> {
    let path = world.script_manager().scene_path_cell().borrow().clone()?;
    Some(!world.scene().borrow().has_unsaved_edits(&path))
}

/// Whether `act` only enters or leaves Play: the transition it asks for is the
/// demo's own start or end, so it is not an act to replay.
fn is_play_verb(act: &HostCall) -> bool {
    let named = match act {
        HostCall::Eval(line) => line.trim_end_matches(';').trim_end(),
        HostCall::Call { function, .. } => function,
        HostCall::Taint(_) => return false,
    };
    let named = named.strip_suffix("()").unwrap_or(named);
    matches!(named, "Editor.EnterPlay" | "Editor.ExitPlay")
}

/// `file` locked for as long as it is open, so no pruning deletes it meanwhile
/// (`files::prune`); a filesystem without locks only loses that guard.
fn lock(file: File) -> BufWriter<File> {
    if let Err(err) = file.try_lock() {
        log::debug!("[Demo] cannot lock the demo being written: {err}");
    }
    BufWriter::new(file)
}

/// The host acts since the last drain, without the Play and Stop verbs.
fn drain(world: &GameWorld) -> Vec<HostCall> {
    let mut acts = world.script_manager().journal().borrow_mut().take();
    acts.retain(|act| !is_play_verb(act));
    acts
}

impl Recorder {
    pub(super) fn new(destination: Destination, world: &GameWorld) -> Self {
        Self {
            destination,
            session: None,
            clean_at_install: is_saved(world),
            written: Vec::new(),
        }
    }

    /// The demos written (or being written) so far, oldest first.
    pub fn written(&self) -> &[PathBuf] {
        &self.written
    }

    /// End the open demo, if any, on `world` as it stands: the host acts since
    /// its last tick and the state digest go in its end line. Right before Stop,
    /// and when a run ends without one.
    pub fn stop(&mut self, world: &GameWorld) {
        let acts = drain(world);
        if let Some(session) = &mut self.session {
            session.host.extend(acts);
            let end = End {
                end: session.t,
                host: session.host.iter().map(host_json).collect(),
                digest: Some(digest::of(world)),
            };
            session.host.clear();
            self.close(end);
        }
    }

    /// Close the open demo with `end`, flushed.
    fn close(&mut self, end: End) {
        if let Some(mut session) = self.session.take() {
            let wrote = serde_json::to_string(&end)
                .map_err(std::io::Error::from)
                .and_then(|end| writeln!(session.out, "{end}"))
                .and_then(|()| session.out.flush());
            if let Err(err) = wrote {
                log::warn!("[Demo] {}: {err}", session.path.display());
            }
        }
    }

    /// An edit-mode frame ended without entering Play, ticked or paused: what a
    /// host did in it is no part of a demo.
    fn edit_frame_done(&mut self, world: &GameWorld) {
        world.script_manager().journal().borrow_mut().take();
        self.clean_at_install = None;
    }

    fn start(&mut self, world: &GameWorld) {
        let mut before_play = drain(world);
        let keep = world.resources.application.borrow().settings().demos.keep;
        let scene_path = world.script_manager().scene_path_cell().borrow().clone();
        let scene = scene_path.clone().unwrap_or_default();
        let Some(path) = self.destination.next(&scene, keep) else {
            return;
        };
        let saved = match self.clean_at_install {
            Some(clean) if !before_play.is_empty() => Some(clean),
            _ => is_saved(world),
        };
        let taint = match saved {
            None => Some("Play started on a scene with no file".to_string()),
            Some(false) => Some(format!("Play started on unsaved edits to {scene}")),
            Some(true) => None,
        };
        before_play.extend(taint.map(HostCall::Taint));
        let time = world.resources.time.borrow().clone();
        let header = Header {
            scene,
            engine_commit: crate::core::project::engine_commit().to_string(),
            seed: world.script_manager().play_seed(),
            sources: Sources::of(world),
            settings: world.resources.application.borrow().settings(),
            storage: world.resources.storage.borrow().data(),
            size: world.resources.screen.borrow().game_view(),
            time_scale: Exact(f64::from(time.time_scale)),
            paused: time.paused,
            pending_steps: time.pending_steps,
            before_play: before_play.iter().map(host_json).collect(),
        };
        // lint: allow(asset-write) a demo, read by `play --demo`, never by a path-keyed cache
        let opened = File::create(&path).map(lock).and_then(|mut out| {
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
                    last_dt: FIXED_DT,
                    size: header.size,
                    host: Vec::new(),
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
            dt: None,
            input: InputDelta::between(&self.last, &frame),
            host: host.iter().map(host_json).collect(),
            size: (size != self.size).then_some(size).flatten(),
            digest: None,
        };
        self.last = frame;
        self.size = size;
    }

    /// The tick's `dt`, written when it is not the last tick's (bit for bit).
    fn read_dt(&mut self, dt: f32) {
        if dt.to_bits() != self.last_dt.to_bits() {
            self.entry.dt = Some(Exact(f64::from(dt)));
            self.last_dt = dt;
        }
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
        match point {
            TapPoint::Before(PlayTransition::Entered) if self.session.is_none() => {
                self.start(world);
            }
            TapPoint::Before(PlayTransition::Exited) => self.stop(world),
            TapPoint::BeforeTick => {
                if let Some(session) = &mut self.session {
                    session.host.extend(drain(world));
                }
            }
            TapPoint::BeforeInput => {
                if let Some(session) = &mut self.session {
                    session.read_input(world);
                }
            }
            TapPoint::AfterPoll if self.session.is_none() => self.edit_frame_done(world),
            TapPoint::AfterTick => {
                let Some(session) = &mut self.session else {
                    self.edit_frame_done(world);
                    return;
                };
                if let Err(err) = session.end_tick(world) {
                    log::warn!("[Demo] {}: {err}", session.path.display());
                    self.session = None;
                }
            }
            TapPoint::Before(_) | TapPoint::Transition(_) | TapPoint::AfterPoll => {}
        }
    }

    fn dt(&mut self, dt: f32) -> f32 {
        if let Some(session) = &mut self.session {
            session.read_dt(dt);
        }
        dt
    }
}

impl Drop for Recorder {
    /// A world dropped mid-Play: the end line, without a final digest.
    fn drop(&mut self) {
        let end = self.session.as_ref().map(|s| End {
            end: s.t,
            ..End::default()
        });
        if let Some(end) = end {
            self.close(end);
        }
    }
}
