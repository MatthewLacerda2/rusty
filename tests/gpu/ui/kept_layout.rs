//! A game's HUD keeps its layout: a script-built HUD under a `CanvasGroup`, a
//! stacked viewmodel camera, a render-texture minimap, a pool of inactive damage
//! numbers anchored to a moving foe, the UI systems ticking every step, and a sized
//! FPS text rewritten every thirty frames. The renderer lays out on the first frame
//! and on the text's frames alone, each named by its reason counter.

use rusty::dev::harness::Harness;

const HUD: &str = r#"
local c = Scene.CreateEntity("HUD")
Scene.AddComponent(c, "Canvas")
Canvas.SetSortOrder(c, 10)
Canvas.SetReferenceResolution(c, 1920, 1080)
Scene.AddComponent(c, "CanvasGroup")
for i = 1, 40 do
  local e = Scene.CreateEntity("E" .. i)
  Scene.AddComponent(e, "RectTransform")
  Scene.SetParent(e, c)
  if i % 2 == 0 then
    Scene.AddComponent(e, "Shape")
  else
    Scene.AddComponent(e, "Text")
    Text.SetText(e, "x" .. i)
  end
  RectTransform.SetAnchoredPosition(e, 20 * i, 30)
  RectTransform.SetSizeDelta(e, 100, 20)
end
VM = Scene.CreateEntity("Viewmodel")
Scene.AddComponent(VM, "Camera")
Camera.SetRenderOrder(VM, 1)
Camera.SetClearFlags(VM, "DepthOnly")
local map = Scene.CreateEntity("MinimapCam")
Scene.AddComponent(map, "Camera")
Camera.SetProjection(map, "Orthographic", 40)
Camera.SetTargetTexture(map, "minimap", 256, 256)
local img = Scene.CreateEntity("Minimap")
Scene.AddComponent(img, "RectTransform")
Scene.AddComponent(img, "Image")
Image.SetTexture(img, "rt:minimap")
Scene.SetParent(img, c)
FOE = Scene.CreateEntity("Foe", "Cube")
for i = 1, 8 do
  local d = Scene.CreateEntity("Damage" .. i)
  Scene.AddComponent(d, "RectTransform")
  Scene.AddComponent(d, "Text")
  Scene.SetParent(d, c)
  RectTransform.SetWorldAnchor(d, FOE, 0, 2, 0)
  Scene.SetActive(d, false)
end
FPS = Scene.CreateEntity("FPS")
Scene.AddComponent(FPS, "RectTransform")
Scene.AddComponent(FPS, "Text")
Scene.AddComponent(FPS, "LayoutElement")
LayoutElement.SetFit(FPS, "PreferredSize", "PreferredSize")
Scene.SetParent(FPS, c)
"#;

#[test]
fn gpu_a_game_hud_lays_out_only_when_a_sized_text_changes() {
    let out = crate::temp::dir().join("rusty_2412_kept_layout");
    let mut h = Harness::new(&out, "");
    h.step(2);
    crate::play::eval(&h, HUD);
    for frame in 0..61 {
        h.step(1);
        let foe = format!("Transform.SetPosition(FOE, {}, 0, -5)", frame as f32 * 0.1);
        crate::play::eval(&h, &foe);
        let rewritten = frame % 30 == 0;
        if rewritten {
            crate::play::eval(&h, &format!("Text.SetText(FPS, 'FPS {frame}')"));
        }
        let Some(drawn) = h.render_frame() else {
            assert!(
                !rusty::render::gpu_required(),
                "RUSTY_REQUIRE_GPU=1 but no frame"
            );
            return;
        };
        let c = drawn.counters;
        let reason = match frame {
            0 => "rebuild",
            _ if rewritten => "content",
            _ => {
                assert_eq!(c.ui_laid_out, 0, "frame {frame}: kept ({:?})", c.ui_relaid);
                continue;
            }
        };
        assert_eq!(c.ui_laid_out, 1, "frame {frame}: laid out");
        let pairs = c.pairs();
        let named = |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|p| p.1);
        let key = format!("ui_relaid_{reason}");
        assert_eq!(
            named(&key),
            Some(1),
            "frame {frame}: {key} ({:?})",
            c.ui_relaid
        );
    }
}
