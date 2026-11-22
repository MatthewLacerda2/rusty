//! Each widget is a tree of first-class primitives whose behaviour is an engine-shipped Lua script
//! component, so games restyle or fork it freely.
//! [`seed`] rewrites `assets/scripts/ui/` and `assets/prefabs/ui/` on every boot; a game that forks
//! a widget copies it to another path.

mod basic;
mod dropdown;
mod input;
pub mod parts;
mod range;
mod scroll;

use std::path::Path;

use crate::prefab::extract_prefab;
use crate::PREFAB_EXTENSION;
use rusty_scene::Scene;

const PREFAB_DIR: &str = "assets/prefabs/ui";

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

    pub fn parse(name: &str) -> Option<Self> {
        let squash = |s: &str| s.replace(' ', "").to_ascii_lowercase();
        let want = squash(name);
        Self::ALL.into_iter().find(|w| squash(w.label()) == want)
    }

    pub fn prefab_path(self) -> Option<String> {
        (self != Self::Canvas).then(|| format!("{PREFAB_DIR}/{}.{PREFAB_EXTENSION}", self.label()))
    }
}

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

/// Unity's GameObject ▸ UI menu: under `parent` when inside a canvas, else the first root canvas,
/// else a new `Canvas`.
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

fn under_canvas(scene: &Scene, id: u32) -> bool {
    let world = &scene.world;
    world.self_and_ancestors(id).any(|c| world.has_canvas(c))
}

fn first_root_canvas(scene: &Scene) -> Option<u32> {
    let world = &scene.world;
    world
        .ids_with_canvas()
        .into_iter()
        .find(|&c| world.parent_id(c).is_none_or(|p| !under_canvas(scene, p)))
}

pub fn seed() {
    let scripts = std::path::PathBuf::from(parts::SCRIPT_DIR);
    std::fs::create_dir_all(&scripts).ok();
    for (name, code) in crate::io::bundled::UI_SCRIPTS {
        write_if_changed(&scripts.join(name), code.as_bytes());
    }
    std::fs::create_dir_all(std::path::PathBuf::from(PREFAB_DIR)).ok();
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

/// Through a temp file and a rename, so a concurrent reader never sees a half-written script.
fn write_if_changed(path: &Path, bytes: &[u8]) {
    // lint: allow(stored-format) seeds scripts and `.prefab`s; the prefab format is `edit::prefab`'s
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return;
    }
    // lint: allow(stored-format) seeds scripts and `.prefab`s; the prefab format is `edit::prefab`'s
    rusty_core::fs::written::write(path, bytes).ok();
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
