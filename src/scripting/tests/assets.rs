//! `Assets.GetLightmapUVSettings` / `SetLightmapUVSettings` (#831): the API side of
//! the model inspector's Generate Lightmap UVs checkbox; and
//! `Assets.Get/SetTextureImportSettings` (#1323), the image inspector's card.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::asset_instance::lightmap_test_support::{has_lightmap_uv, quad_model};
use crate::scene::{Camera, Scene};
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

#[test]
fn the_assets_api_sets_and_reads_the_setting() {
    let path = quad_model("rusty_lmuv_api");
    let mut raw = Scene::new();
    let id = instantiate_asset(&mut raw, &format!("{path}::Quad"), None, Vec3::ZERO);
    let scene = Rc::new(RefCell::new(raw));
    let m = manager(&scene);
    let lua = format!(
        "local p = {path:?}
         assert(Assets.GetLightmapUVSettings(p).generate == false)
         assert(Assets.SetLightmapUVSettings(p, {{ generate = true, packMargin = 100 }}) == 1)
         local s = Assets.GetLightmapUVSettings(p)
         assert(s.generate and s.hardAngle == 88 and s.packMargin == 64, 'clamped')"
    );
    m.exec(&lua).unwrap();
    assert!(has_lightmap_uv(&scene.borrow(), id.unwrap()));
}

#[test]
fn the_assets_api_sets_and_reads_a_textures_import_settings() {
    let png = crate::test_temp::dir().join("brick.png");
    let m = manager(&Rc::new(RefCell::new(Scene::new())));
    let lua = format!(
        "local p = {png:?}
         local s = Assets.GetTextureImportSettings(p)
         assert(s.wrapMode == 'Repeat' and s.filterMode == 'Trilinear' and s.generateMipmaps)
         Assets.SetTextureImportSettings(p, {{ filterMode = 'Point', generateMipmaps = false }})
         s = Assets.GetTextureImportSettings(p)
         assert(s.wrapMode == 'Repeat' and s.filterMode == 'Point' and not s.generateMipmaps)
         local ok, err = pcall(Assets.SetTextureImportSettings, p, {{ wrapMode = 'Tile' }})
         assert(not ok and tostring(err):find('Mirror'), tostring(err))"
    );
    m.exec(&lua).unwrap();
    let stored = crate::asset::sidecar::texture(&png);
    assert_eq!(stored.filter_mode, crate::asset::sidecar::FilterMode::Point);
}

fn manager(scene: &Rc<RefCell<Scene>>) -> ScriptManager {
    let mut m = ScriptManager::new(
        Rc::clone(scene),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    m
}
