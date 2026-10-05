//! `Assets.GetLightmapUVSettings` / `SetLightmapUVSettings` (#831): the API side of
//! the model inspector's Generate Lightmap UVs checkbox.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use super::{ConsoleLogs, ScriptManager};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::asset_instance::lightmap_test_support::{has_lightmap_uv, quad_model};
use crate::scene::{Camera, Scene};
use crate::time::Time;

#[test]
fn the_assets_api_sets_and_reads_the_setting() {
    let path = quad_model("rusty_lmuv_api");
    let mut raw = Scene::new();
    let id = instantiate_asset(&mut raw, &format!("{path}::Quad"), None, Vec3::ZERO);
    let scene = Rc::new(RefCell::new(raw));
    let mut m = ScriptManager::new(
        Rc::clone(&scene),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
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
