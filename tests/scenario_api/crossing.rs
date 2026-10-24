//! Values crossing between the scenario VM and the gameplay VM keep their Lua
//! types (#1384): integer keys stay integers, a mixed table stays mixed, and
//! non-finite numbers survive, written into the instance and read back out.

use super::run;

const SCRIPT: &str = r#"
local player = Scene.FindEntityByName("Player")
Harness.Step(2)
local pc = Scene.GetScript(player, "player_controller")
pc.sparse = { [0] = true, [42] = true }
pc.mixed = { "rifle", ammo = 30 }
pc.huge = math.huge
pc.nan = 0 / 0
pc[7] = "seven"
local back = Scene.GetScript(player, "player_controller")
local sparse, mixed = back.sparse, back.mixed
Harness.Expect(sparse[0] == true and sparse[42] == true, "integer keys stay integers")
Harness.Expect(sparse["0"] == nil and sparse["42"] == nil, "and never become strings")
Harness.Expect(mixed[1] == "rifle" and #mixed == 1, "the array part crosses")
Harness.Expect(math.type(mixed.ammo) == "integer" and mixed.ammo == 30, "an integer stays one")
Harness.Expect(back.huge == math.huge, "math.huge crosses")
Harness.Expect(back.nan ~= back.nan, "NaN crosses")
Harness.Expect(back[7] == "seven", "an integer field key crosses")
"#;

#[test]
fn values_keep_their_lua_types_both_ways() {
    let (passed, json) = run("rusty_scenario_crossing", SCRIPT);
    assert!(passed, "scenario expectations pass: {json}");
}
