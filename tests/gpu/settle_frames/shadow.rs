//! A shadow tile resize the atlas's hysteresis holds back is history: each
//! frame drawn counts towards it, so a settled shot after a jump that takes a
//! shadowed spot light out of its tile's band still draws every frame, and writes
//! what a full settle writes.

use super::{png, run};

/// A shadow-casting spot light; the camera settles inside its range (largest tile), then jumps
/// forty metres off (smallest), cuts, and shoots settled 16 frames: one past the 15 the atlas
/// holds a resize back, so the resize lands inside the settle.
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
  for _ = 1, 16 do Harness.Render() end
  Camera.SetPosition(0, 3, 40)
end
near()
shot("settled.png", { settle = 16 }, 17)
near()
full("full.png", 16)
"#;

#[test]
fn a_pending_shadow_resize_keeps_a_settle_drawing() {
    let Some(out) = run("shadow", JUMP, "settled.png") else {
        return;
    };
    assert_eq!(png(&out, "settled.png"), png(&out, "full.png"));
    let _ = std::fs::remove_dir_all(out);
}
