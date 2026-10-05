//! src/scene/authoring/ui_widgets/ — the standard UI widget kit (#422).
//!
//! Unity's **GameObject ▸ UI** menu: Canvas, Panel, Image, Text and the widgets —
//! Button, Toggle (+ Toggle Group), Slider, Scrollbar, Scroll View, Dropdown and
//! Input Field. Each widget is a small tree of first-class primitives
//! (`RectTransform`, `Image`, `Text`, `Selectable`, `RectMask`, `LayoutGroup`, …)
//! whose behaviour is an engine-shipped Lua **script component**
//! (`engine/scripts/ui/*.lua`) — not a first-class component, so games restyle or
//! fork it freely. The trees are built here, once: the editor's Create ▸ UI menu
//! and `UI.Create` call [`create_ui`], and [`seed`] writes the same trees as
//! `.prefab`s for `Scene.Instantiate`.
//!
//! **Engine-owned directories.** [`seed`] rewrites the project's
//! `assets/scripts/ui/` and `assets/prefabs/ui/` from the engine on every boot, so they never go stale;
//! a game that forks a widget copies its script (or prefab) to another path.

mod basic;
mod dropdown;
mod input;
pub mod parts;
mod range;
mod scroll;

use std::path::Path;

use crate::scene::prefab::extract_prefab;
use crate::scene::Scene;

/// Where [`seed`] writes the widget prefabs, relative to the project root.
pub const PREFAB_DIR: &str = "assets/prefabs/ui";

/// Every entry of the Create ▸ UI menu, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiWidget {
    Canvas,
    Panel,
    Image,
    Text,
    Button,
    Toggle,
    ToggleGroup,
    Slider,
    Scrollbar,
    ScrollView,
    Dropdown,
    InputField,
}

impl UiWidget {
    pub const ALL: [UiWidget; 12] = [
        Self::Canvas,
        Self::Panel,
        Self::Image,
        Self::Text,
        Self::Button,
        Self::Toggle,
        Self::ToggleGroup,
        Self::Slider,
        Self::Scrollbar,
        Self::ScrollView,
        Self::Dropdown,
        Self::InputField,
    ];

    /// The menu label, which is also the new root's name and the prefab's file stem.
    pub fn label(self) -> &'static str {
        match self {
            Self::Canvas => "Canvas",
            Self::Panel => "Panel",
            Self::Image => "Image",
            Self::Text => "Text",
            Self::Button => "Button",
            Self::Toggle => "Toggle",
            Self::ToggleGroup => "Toggle Group",
            Self::Slider => "Slider",
            Self::Scrollbar => "Scrollbar",
            Self::ScrollView => "Scroll View",
            Self::Dropdown => "Dropdown",
            Self::InputField => "Input Field",
        }
    }

    /// Parse a label, case- and space-insensitively (`"scrollview"`, `"Input Field"`).
    pub fn parse(name: &str) -> Option<Self> {
        let squash = |s: &str| s.replace(' ', "").to_ascii_lowercase();
        let want = squash(name);
        Self::ALL.into_iter().find(|w| squash(w.label()) == want)
    }

    /// The prefab [`seed`] writes for it (`None` for `Canvas`, a scene root).
    pub fn prefab_path(self) -> Option<String> {
        (self != Self::Canvas).then(|| format!("{PREFAB_DIR}/{}.prefab", self.label()))
    }
}

/// Build `kind` under `parent` as is — no canvas lookup. Returns its root.
fn build(scene: &mut Scene, kind: UiWidget, parent: Option<u32>) -> u32 {
    match kind {
        UiWidget::Canvas => basic::canvas(scene, parent),
        UiWidget::Panel => basic::panel(scene, parent),
        UiWidget::Image => basic::image_node(scene, parent),
        UiWidget::Text => basic::text_node(scene, parent),
        UiWidget::Button => basic::button(scene, parent),
        UiWidget::Toggle => basic::toggle(scene, parent),
        UiWidget::ToggleGroup => basic::toggle_group(scene, parent),
        UiWidget::Slider => range::slider(scene, parent),
        UiWidget::Scrollbar => range::scrollbar(scene, parent),
        UiWidget::ScrollView => scroll::scroll_view(scene, parent),
        UiWidget::Dropdown => dropdown::dropdown(scene, parent),
        UiWidget::InputField => input::input_field(scene, parent),
    }
}

/// Create `kind` the way Unity's GameObject ▸ UI menu does: under `parent` when it
/// is inside a canvas, else under the scene's first root canvas, else under a new
/// `Canvas`. A `Canvas` itself is created at the root. Returns the new root.
pub fn create_ui(scene: &mut Scene, kind: UiWidget, parent: Option<u32>) -> u32 {
    if kind == UiWidget::Canvas {
        return build(scene, kind, None);
    }
    let host = parent
        .filter(|&p| scene.world.contains(p) && under_canvas(scene, p))
        .or_else(|| first_root_canvas(scene))
        .unwrap_or_else(|| build(scene, UiWidget::Canvas, None));
    build(scene, kind, Some(host))
}

/// Whether `id` or an ancestor carries a `Canvas` (bounded against parent cycles).
fn under_canvas(scene: &Scene, id: u32) -> bool {
    let world = &scene.world;
    let mut at = Some(id);
    for _ in 0..=world.len() {
        let Some(c) = at else { return false };
        if world.has_canvas(c) {
            return true;
        }
        at = world.parent_id(c);
    }
    false
}

/// The first canvas (in scene order) with no canvas above it.
fn first_root_canvas(scene: &Scene) -> Option<u32> {
    let world = &scene.world;
    world
        .ids_with_canvas()
        .into_iter()
        .find(|&c| world.parent_id(c).is_none_or(|p| !under_canvas(scene, p)))
}

/// Rewrite the engine-owned widget directories: the scripts embedded from
/// `engine/scripts/ui/` (`scene::io::bundled`), and one prefab per widget built fresh. Called on boot
/// (and by tests); failures are skipped, like the other seeders.
pub fn seed() {
    let scripts = parts::SCRIPT_DIR;
    std::fs::create_dir_all(scripts).ok();
    for (name, code) in crate::scene::io::bundled::UI_SCRIPTS {
        write_if_changed(&Path::new(scripts).join(name), code.as_bytes());
    }
    std::fs::create_dir_all(PREFAB_DIR).ok();
    for kind in UiWidget::ALL {
        let Some(path) = kind.prefab_path() else {
            continue;
        };
        let mut scene = Scene::new();
        let root = build(&mut scene, kind, None);
        let json = extract_prefab(&scene, root).and_then(|p| serde_json::to_vec_pretty(&p).ok());
        if let Some(json) = json {
            write_if_changed(Path::new(&path), &json);
        }
    }
}

/// Write `bytes` to `path` unless it already holds them — through a temp file and a
/// rename, so a concurrent reader (another test process seeding the same
/// workspace) never sees a half-written script.
fn write_if_changed(path: &Path, bytes: &[u8]) {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
        std::fs::remove_file(&tmp).ok();
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
