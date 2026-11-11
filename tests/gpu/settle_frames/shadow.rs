//! A shadow tile resize the atlas's hysteresis holds back (#1711) is history: each
//! frame drawn counts towards it, so a settled shot after a jump that takes a
//! shadowed spot light out of its tile's band still draws every frame, and writes
//! what the full settle wrote before #2092.

use super::{png, run};

/// A shadow-casting spot light over the origin; the camera stands inside its range
/// (it wants the largest tile) for long enough to settle there, then jumps forty
/// metres off (it wants the smallest), cut, and takes a shot settled 32 frames
/// both ways.
const JUMP: &str = r#"
Graphics.SetTaaActive(false)
local lamp = Scene.CreateEntity("Lamp")
Scene.AddComponent(lamp, "Light")
Light.SetType(lamp, "Spot")
Light.SetRange(lamp, 4)
Light.SetIntensity(lamp, 5)
Light.SetCastShadows(lamp, true)
Transform.SetPosition(lamp, 0, 3, 0)
local function near()
  Camera.SetPosition(0, 3, 1)
  Camera.SetYaw(-90)
  for _ = 1, 20 do Harness.Render() end
  Camera.SetPosition(0, 3, 40)
end
near()
shot("settled.png", { settle = 32 }, 33)
near()
full("full.png", 32)
"#;

#[test]
fn a_pending_shadow_resize_keeps_a_settle_drawing() {
    let Some(out) = run("shadow", JUMP, "settled.png") else {
        return;
    };
    assert_eq!(png(&out, "settled.png"), png(&out, "full.png"));
    let _ = std::fs::remove_dir_all(out);
}
