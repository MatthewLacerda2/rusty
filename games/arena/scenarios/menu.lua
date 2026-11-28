-- Escape opens the pause menu over the fight: the sim stops, the cursor is
-- freed and the widgets are wired; its sensitivity slider reaches the player;
-- Escape again closes it and the fight goes on. Its `menu` shot. No bot
-- fires while it is open, even one stood in front of the player, its gun cooled.

Harness.Step(3) -- enter Play: the menu builds, then wires the tick after
-- Found through its canvas: FindEntityByName skips an inactive entity, as Unity does.
local root = Scene.FindChild(Scene.FindEntityByName("Menu_Canvas"), "Menu_Root")
local player = Scene.FindEntityByName("Player")
Harness.Expect(root ~= nil and not Scene.IsActive(root), "the menu starts closed")

local function tap(key)
    Input.Press(key)
    Harness.Step(1)
    Input.Release(key)
    Harness.Step(1)
end

tap("ESCAPE")
Harness.Expect(Scene.IsActiveInHierarchy(root), "Escape opens the menu")
Harness.Expect(Time.GetTimeScale() == 0, "the sim stops under it: " .. Time.GetTimeScale())
Harness.Expect(not Input.IsCursorLocked() and Input.IsCursorVisible(), "the cursor is free")
local frozen = { Transform.GetPosition(Scene.FindEntityByName("Bot_1")) }
local shooter = Scene.GetScript(player, "shooter")
local health = shooter.health
-- Bot_2 faces the player from 3 m with a cooled gun: only the pause holds its fire.
local px, py, pz = Transform.GetPosition(player)
local bot = Scene.FindEntityByName("Bot_2")
NavMeshAgent.Warp(bot, px, py, pz + 3)
Scene.GetScript(bot, "snowman").gun.cooldown = 0

local slider = Scene.FindChild(Scene.FindEntityByName("Look sensitivity"), "Slider")
Scene.GetScript(slider, "slider").set_value(2)
Harness.Step(20)
Harness.Expect(Scene.GetScript(player, "shooter").look_scale == 2,
    "the slider reaches the player's look")
local now = { Transform.GetPosition(Scene.FindEntityByName("Bot_1")) }
Harness.Expect(now[1] == frozen[1] and now[3] == frozen[3], "nothing moves while paused")
Harness.Expect(shooter.health == health,
    "no bot hurts the player while paused: " .. health .. " -> " .. shooter.health)
local static = Scene.FindChild(Scene.FindEntityByName("Crosshair"), "Toggle Group/Option 2")
Scene.GetScript(static, "toggle").set_is_on(true)
Harness.Step(1)
Harness.Expect(Scene.GetScript(Scene.FindEntityByName("HUD"), "hud").static_cross == true,
    "the crosshair radio pair reaches the HUD")
Harness.Screenshot("cache/test/shots/menu.png", { settle = 4 })

tap("ESCAPE")
Harness.Expect(Time.GetTimeScale() == 1, "Escape closes it and the fight goes on")
Harness.Step(10)
Harness.Expect(not Scene.IsActive(root), "the menu fades out and hides")
