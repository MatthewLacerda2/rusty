use super::{check, RULE};
use crate::repo::scaffold::Repo;

const REGISTRY: &str = "macro_rules! first_class {\n\
    // sorted: rows\n\
    camera: rendering::camera::CameraComponent => Camera [camera_mut];\n\
    mask: ui::mask::MaskComponent => Mask [mask_mut];\n\
    rect_mask: ui::rect_mask::RectMaskComponent => RectMask [rect_mask_mut];\n\
}\n";

/// A repo with two UI components (`Mask`, `RectMask`), two widgets and one game.
fn repo(name: &str, recipe: &str) -> Repo {
    let repo = Repo::new(&format!("ui-coverage-{name}"), false);
    repo.file("crates/components/src/registry.rs", REGISTRY);
    repo.file("engine/scripts/ui/scroll_view.lua", "return {}\n");
    repo.file("engine/scripts/ui/slider.lua", "return {}\n");
    repo.file("games/arena/recipe.lua", recipe);
    repo
}

fn flagged(repo: &Repo) -> Vec<(String, usize)> {
    check(&repo.0)
        .into_iter()
        .inspect(|f| assert_eq!(f.rule, RULE))
        .map(|f| (f.path, f.line))
        .collect()
}

#[test]
fn passes_when_every_part_is_named_by_string_namespace_or_path() {
    let dir = repo(
        "pass",
        "Scene.AddComponent(id, \"Mask\")\nlocal v = UI.Create('Scroll View', c)\n",
    );
    dir.file(
        "games/shared/menu.lua",
        "RectMask.SetFeather(v, 8)\nScene.Instantiate(\"assets/prefabs/ui/Slider.prefab\")\n",
    );
    assert_eq!(flagged(&dir), []);
}

#[test]
fn flags_each_part_no_game_uses_at_its_row_or_script() {
    let dir = repo("flag", "Scene.AddComponent(id, \"Mask\")\n");
    assert_eq!(
        flagged(&dir),
        [
            ("crates/components/src/registry.rs".to_string(), 5),
            ("engine/scripts/ui/scroll_view.lua".to_string(), 1),
            ("engine/scripts/ui/slider.lua".to_string(), 1),
        ]
    );
}

#[test]
fn comments_scenarios_and_look_alike_names_are_no_use() {
    let dir = repo(
        "near",
        "-- Scene.AddComponent(id, \"RectMask\")\nMyMask.Set(1)\nx.Mask.Set(1)\n\
         Scene.AddComponent(id, \"Mask\")\nlocal s = \"slider bar\"\nUI.Create(\"Scroll View\")\n",
    );
    dir.file("games/arena/scenarios/a.lua", "RectMask.SetFeather(v, 8)\n");
    dir.file("games/arena/shots.lua", "UI.Create(\"Slider\")\n");
    assert_eq!(
        flagged(&dir),
        [
            ("crates/components/src/registry.rs".to_string(), 5),
            ("engine/scripts/ui/slider.lua".to_string(), 1),
        ]
    );
}

#[test]
fn an_inline_allow_with_a_reason_excuses_a_row_or_a_script() {
    let dir = repo(
        "allow",
        "Scene.AddComponent(id, \"Mask\")\nUI.Create(\"Slider\")\n",
    );
    let excused = REGISTRY.replace(
        "\nrect_mask:",
        "\n// lint: allow(ui-coverage) no game needs it yet\nrect_mask:",
    );
    dir.file("crates/components/src/registry.rs", &excused);
    dir.file(
        "engine/scripts/ui/scroll_view.lua",
        "-- lint: allow(ui-coverage) a helper module, used through slider\nreturn {}\n",
    );
    assert_eq!(flagged(&dir), []);
}

#[test]
fn a_repo_with_games_but_no_registry_or_widgets_is_flagged_not_passed() {
    let dir = Repo::new("ui-coverage-lost", false);
    dir.file("games/arena/recipe.lua", "");
    let paths: Vec<String> = check(&dir.0).into_iter().map(|f| f.path).collect();
    assert_eq!(
        paths,
        ["crates/components/src/registry.rs", "engine/scripts/ui"]
    );
    assert!(check(&Repo::new("ui-coverage-none", false).0).is_empty());
}
