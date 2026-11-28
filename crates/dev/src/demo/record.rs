use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use super::calls::is_play_verb;
use super::files::Destination;
use super::format::{host_json, End, Entry, Exact, Header};
use super::index::{IndexLine, Indexer};
use super::{delta::InputDelta, digest};
use crate::harness::FIXED_DT;
use rusty_app::{GameWorld, PlayTransition, TapPoint, TickTap};
use rusty_core::input::InputFrame;
use rusty_script::scripting::HostCall;

struct Session {
    out: BufWriter<File>,
    path: PathBuf,
    t: u64,
    last: InputFrame,
    last_dt: f32,
    size: Option<(u32, u32)>,
    display_scale: Option<f32>,
    host: Vec<HostCall>,
    entry: Entry,
    index: Indexer,
}

pub struct Recorder {
    destination: Destination,
    session: Option<Session>,
    /// Whether the scene was saved at install, until a tick passes: acts made before then
    /// (a scenario's set-up) are replayed, so their edits count as no unsaved edit.
    clean_at_install: Option<bool>,
    written: Vec<PathBuf>,
}

fn is_saved(world: &GameWorld) -> Option<bool> {
    let path = world.script_manager().scene_path_cell().borrow().clone()?;
    let scene = world.scene();
    Some(!rusty_edit::has_unsaved_edits(&scene.borrow(), &path))
}

/// Held while the file is open, so `files::prune` never deletes it; a filesystem without
/// locks only loses that guard.
fn lock(file: File) -> BufWriter<File> {
    if let Err(err) = file.try_lock() {
        log::debug!("[Demo] cannot lock the demo being written: {err}");
    }
    BufWriter::new(file)
}

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

    pub fn written(&self) -> &[PathBuf] {
        &self.written
    }

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
            let index = session.index.end(world, session.t);
            if let Err(err) = session.write_index(&index) {
                log::warn!("[Demo] {}: {err}", session.path.display());
            }
            self.close(end);
        }
    }

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
        if world.scene().borrow().loaded.active_of_several().is_some() {
            let why =
                format!("Play started with several scene files loaded; {scene} replays alone");
            before_play.push(HostCall::Taint(why));
        }
        let header = Header::of(world, scene, &before_play);
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
                    display_scale: header.display_scale,
                    host: Vec::new(),
                    entry: Entry::default(),
                    index: Indexer::start(world),
                });
            }
            Err(err) => log::warn!("[Demo] {}: {err}", path.display()),
        }
    }
}

impl Session {
    fn read_input(&mut self, world: &GameWorld) {
        let frame = world.input().borrow().frame();
        let screen = world.resources.screen.borrow();
        let (size, scale) = (screen.game_view(), screen.reported_display_scale());
        drop(screen);
        let host = std::mem::take(&mut self.host);
        self.entry = Entry {
            t: self.t,
            dt: None,
            input: InputDelta::between(&self.last, &frame),
            host: host.iter().map(host_json).collect(),
            size: (size != self.size).then_some(size).flatten(),
            display_scale: (scale != self.display_scale).then_some(scale).flatten(),
            ..Entry::default()
        };
        self.last = frame;
        self.size = size;
        self.display_scale = scale;
    }

    fn read_dt(&mut self, dt: f32) {
        if dt.to_bits() != self.last_dt.to_bits() {
            self.entry.dt = Some(Exact(f64::from(dt)));
            self.last_dt = dt;
        }
    }

    fn write_index(&mut self, line: &IndexLine) -> std::io::Result<()> {
        writeln!(self.out, "{}", serde_json::to_string(line)?)
    }

    fn end_tick(&mut self, world: &GameWorld) -> std::io::Result<()> {
        if digest::due(self.t) {
            self.entry.digest = Some(digest::of(world));
        }
        let entry = std::mem::take(&mut self.entry);
        if !entry.is_empty() {
            writeln!(self.out, "{}", serde_json::to_string(&entry)?)?;
        }
        if let Some(index) = self.index.line(world, self.t) {
            self.write_index(&index)?;
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
            // This fires however Play is entered, so the header is read before `Awake` runs.
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
                    session.index.end(world, session.t);
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

    fn loads(&mut self, ready: Vec<u32>) -> Vec<u32> {
        if let Some(session) = &mut self.session {
            session.entry.loads.clone_from(&ready);
        }
        ready
    }

    fn load_progress(&mut self, moved: Option<u8>) -> Option<u8> {
        if let Some(session) = &mut self.session {
            session.entry.load_progress = moved;
        }
        moved
    }
}

impl Drop for Recorder {
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
