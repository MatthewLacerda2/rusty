-- The player feeds the shared HUD: a full magazine at the start, shots
-- counted off it, its health after a hit, a damage number on a bot it hit.

Harness.Step(2) -- enter Play: the scripts start, the HUD builds
local match = Scene.GetScript(Scene.FindEntityByName("Match"), "match")
local player = Scene.FindEntityByName("Player")
local text = function(name) return Text.GetText(Scene.FindEntityByName(name)) end

Harness.Expect(text("Ammo") == "30 / 90", "a full magazine: " .. text("Ammo"))
Harness.Expect(text("Health") == "100", "full health: " .. text("Health"))

Input.Press("Mouse0")
Harness.Step(30)
Input.Release("Mouse0")
Harness.Step(1)
local mag = tonumber(text("Ammo"):match("^(%d+) /"))
Harness.Log("after half a second of fire: " .. text("Ammo"))
Harness.Expect(mag ~= nil and mag < 30 and mag > 0, "the shots came off the magazine")

match.over = true -- nobody moves or shoots from here
Damage.Apply(player, 25)
Harness.Step(1)
Harness.Expect(text("Health") == "75", "the bar follows a hit: " .. text("Health"))
local _, _, _, flash = Image.GetColor(Scene.FindEntityByName("Hit_Flash"))
Harness.Expect(flash > 0, "the hit flashes red")
Harness.Screenshot("cache/test/shots/hud.png", { settle = 4 })
