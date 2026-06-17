//! Integration coverage for the `Video` scripting namespace (issue #89) and the
//! persistence of video settings through `Storage`. The `Video.*` setters write a
//! shared `VideoSettings` cell that the platform layer reads each frame to
//! reconfigure the surface/window; these are render-only, one-way writes — they
//! never feed `FixedUpdate` — so the tests assert the cell + the storage blob, not
//! sim evolution. GPU surface/window reconfiguration itself can't run headless, so
//! the renderer-apply path is covered by the pure decision logic in
//! `core::video::VideoSettings::diff`, exercised here through the cell.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::core::storage::Storage;
use rusty::core::video::{VideoSettings, VIDEO_NAMESPACE};

fn setup() -> (Lua, Rc<RefCell<VideoSettings>>) {
    let lua = Lua::new();
    let video = Rc::new(RefCell::new(VideoSettings::default()));
    rusty::api::video::register(&lua, &video).unwrap();
    (lua, video)
}

#[test]
fn resolution_writes_cell_and_clamps_zero() {
    let (lua, video) = setup();
    lua.load("Video.SetResolution(1920, 1080)").exec().unwrap();
    assert_eq!(video.borrow().resolution(), (1920, 1080));

    let res: (u32, u32) = lua.load("return Video.GetResolution()").eval().unwrap();
    assert_eq!(res, (1920, 1080));

    // A zero axis is clamped to 1 so the platform layer never gets a 0-sized surface.
    lua.load("Video.SetResolution(0, 0)").exec().unwrap();
    assert_eq!(video.borrow().resolution(), (1, 1));
}

#[test]
fn vsync_and_fullscreen_round_trip_through_cell() {
    let (lua, video) = setup();
    assert!(video.borrow().vsync, "default vsync on");
    assert!(!video.borrow().fullscreen, "default windowed");

    lua.load("Video.SetVsync(false); Video.SetFullscreen(true)")
        .exec()
        .unwrap();
    assert!(!video.borrow().vsync);
    assert!(video.borrow().fullscreen);

    let v: bool = lua.load("return Video.GetVsync()").eval().unwrap();
    let f: bool = lua.load("return Video.GetFullscreen()").eval().unwrap();
    assert!(!v);
    assert!(f);
}

#[test]
fn settings_round_trip_through_storage_namespace() {
    // Mirrors the platform-layer persist/load boundary: a chosen VideoSettings is
    // written into the `video` Storage namespace and read back to the same value.
    let chosen = VideoSettings {
        width: 2560,
        height: 1440,
        vsync: false,
        fullscreen: true,
    };

    let mut storage = Storage::new();
    storage.set_namespace(VIDEO_NAMESPACE, chosen.to_json());

    let blob = storage.get_namespace(VIDEO_NAMESPACE).unwrap();
    assert_eq!(VideoSettings::from_json(&blob), chosen);
}

#[test]
fn diff_drives_minimal_reconfiguration() {
    // The platform apply site reconfigures only what changed. A pure resolution
    // change must not request a vsync/fullscreen reconfigure, and vice versa.
    let base = VideoSettings::default();

    let resized = VideoSettings {
        width: 800,
        height: 600,
        ..base
    };
    let c = resized.diff(&base);
    assert!(c.resolution && !c.vsync && !c.fullscreen);

    let same = base.diff(&base);
    assert!(!same.any(), "no-op change requests no reconfiguration");
}
