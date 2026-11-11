-- scenarios/nav.lua — every spawn reaches every other room and both floors.
--
-- In each block: a `complete` path from every spawn to every other spawn (one
-- per room, one on the upper floor) and to the crate top; from the first spawn
-- to every other point of the map; the stairs alone reach the upper floor (the
-- ladder switched off); and the navmesh holds the ladder, a drop off the upper
-- floor and a jump onto the crate.

Harness.Step(1) -- enter Play: the navmesh bakes

local function points(group)
    local out = {}
    for _, id in ipairs(Scene.GetChildren(Scene.FindEntityByName(group))) do
        local x, y, z = Transform.GetPosition(id)
        out[#out + 1] = { name = Scene.GetName(id), x = x, y = y, z = z }
    end
    return out
end
local spawns = points("Spawns")
local all = points("Spawns")
for _, w in ipairs(points("Waypoints")) do all[#all + 1] = w end
Harness.Expect(#spawns >= 5, "the arena has a spawn per room and on the upper floor")

local function block(name) return name:match("_(%d+)_") end
local failed = {}
local function check(a, b)
    local path = Navigation.CalculatePath(a.x, a.y, a.z, b.x, b.y, b.z)
    if path.status ~= "complete" then
        failed[#failed + 1] = a.name .. " -> " .. b.name .. " (" .. path.status .. ")"
    end
end
local paths = 0
for _, a in ipairs(spawns) do
    for _, b in ipairs(all) do
        if a ~= b and (block(a.name) == block(b.name) or a == spawns[1]) then
            check(a, b)
            paths = paths + 1
        end
    end
end
Harness.Log(string.format("%d paths, %d not complete", paths, #failed))
for i = 1, math.min(#failed, 10) do Harness.Log("not complete: " .. failed[i]) end
Harness.Expect(#failed == 0, "a complete path from every spawn to every room and both floors")

-- The navmesh's links: the ladder connected, a drop off the upper floor (3 m),
-- a jump up onto the crate (1 m).
local drop, jump = false, false
for _, link in ipairs(Navigation.GetOffMeshLinks()) do
    local rise = link.endPos.y - link.startPos.y
    if link.type == "drop" and rise < -2.5 then drop = true end
    if link.type == "jump" and rise > 0.8 and rise < 1.2 then jump = true end
end
local ladder = Scene.FindEntityByName("LadderLink_1")
Harness.Expect(OffMeshLink.IsConnected(ladder), "the ladder joins the ground to the upper floor")
Harness.Expect(drop, "a drop leads off the upper floor")
Harness.Expect(jump, "a jump leads onto the crate")

-- The stairs on their own: with the ladder off, the upper floor is still a
-- complete path away, up 12 steps under MaxStep.
OffMeshLink.SetActive(ladder, false)
Harness.Step(1)
local foot, top = spawns[2], spawns[5]
local up = Navigation.CalculatePath(foot.x, foot.y, foot.z, top.x, top.y, top.z)
Harness.Log(string.format("%s -> %s by the stairs: %s, %.1f m", foot.name, top.name, up.status, up.length or 0))
Harness.Expect(up.status == "complete", "the stairs alone reach the upper floor")
