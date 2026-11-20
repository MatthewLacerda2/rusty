-- tests/fixtures/project/scenarios/bench/firefight.lua — the bench's firefight tick (#2448).
--
-- What about four automatic weapons firing ask of the engine every tick, through
-- the API a game uses: prefab instances spawned and destroyed (casings, one of
-- them under a soldier's hand bone: the gun-on-a-hand case), muzzle-flash meshes
-- toggled, a decal stamped (the oldest of its own dropped at its cap), a pickup
-- re-parented between two holders, and a raycast a shot. Every count is a
-- constant below; each instance lives a fixed number of ticks, so once LIFETIME
-- and DECALS ticks have passed the live entity and decal counts hold still: a
-- window measured after that measures churn, not growth. No randomness, no
-- clock: two runs do the same edits.

local SHOOTERS = 4  -- the first soldiers, who "fire" (fewer if the scene has fewer)
local SPAWNS = 4    -- prefab instances spawned a tick, and as many destroyed
local LIFETIME = 30 -- ticks an instance lives: SPAWNS * LIFETIME live at once
local MUZZLES = 8   -- muzzle-flash meshes, two under each shooter's hand
local TOGGLES = 2   -- of them toggled a tick
local DECALS = 64   -- this script's decals kept: one stamped a tick, the oldest dropped
local RAYS = 4      -- raycasts a tick, one a shooter
local EYES = 1.6

local Firefight = {
    fields = {
        prefab = {
            type = "string",
            default = "scenarios/bench/casing.prefab",
            tooltip = "The casing spawned, SPAWNS a tick",
        },
    },
}

local tick = 0
local shooters, hands, muzzles = {}, {}, {}
local holders, pickup = {}, nil
-- Queues, oldest first, read from `head` and written at `tail` (no holes for `#`).
local live = { head = 1, tail = 0 }   -- spawned instances: { id, dies }
local decals = { head = 1, tail = 0 } -- this script's decal ids

local function push(q, v)
    q.tail = q.tail + 1
    q[q.tail] = v
end

local function pop(q)
    local v = q[q.head]
    q[q.head] = nil
    q.head = q.head + 1
    return v
end

-- The descendant of `id` named `name`, depth first.
local function find(id, name)
    for _, child in ipairs(Scene.GetChildren(id)) do
        if Scene.GetName(child) == name then
            return child
        end
        local found = find(child, name)
        if found then
            return found
        end
    end
    return nil
end

local function box(name, parent, x, y, z, sx, sy, sz)
    local id = Scene.CreateEntity(name, "Box")
    Scene.RemoveComponent(id, "Collider") -- draw-only
    if parent then
        Scene.SetParent(id, parent)
        Transform.SetLocalPosition(id, x, y, z)
    else
        Transform.SetPosition(id, x, y, z)
    end
    Transform.SetLocalScale(id, sx, sy, sz)
    return id
end

function Firefight.Start(id)
    for i = 1, SHOOTERS do
        local soldier = Scene.FindEntityByName("Soldier_" .. (i - 1))
        if soldier == nil then
            break
        end
        shooters[i] = soldier
        hands[i] = find(soldier, "hand_r")
    end
    -- A smaller stress scene has fewer shooters; none is a scenario error.
    SHOOTERS = #shooters
    assert(SHOOTERS > 0, "firefight: no Soldier_0 in the scene (Harness.LoadStress first)")
    for i = 1, MUZZLES do
        local hand = hands[(i - 1) % SHOOTERS + 1]
        muzzles[i] = box("Muzzle_" .. i, hand, 0, -0.1, 0.3, 0.08, 0.08, 0.15)
    end
    holders[1] = Scene.CreateEntity("Holder_A")
    Transform.SetPosition(holders[1], -3, 1, -10)
    holders[2] = Scene.CreateEntity("Holder_B")
    Transform.SetPosition(holders[2], 3, 1, -10)
    pickup = box("Pickup", holders[1], 0, 0, 0, 0.3, 0.2, 0.5)
end

-- Spawn this tick's instances: the first under a shooter's hand bone (a kinematic
-- body, carried by the walk), the rest ejected from the shooters as casings.
local function spawn()
    for k = 1, SPAWNS do
        local s = (tick + k) % SHOOTERS + 1
        local id
        if k == 1 and hands[s] then
            id = Scene.Instantiate(Firefight.prefab, hands[s])
            Physics.SetKinematic(id, true)
            Transform.SetLocalPosition(id, 0, -0.05, 0.1)
        else
            id = Scene.Instantiate(Firefight.prefab)
            local x, y, z = Transform.GetPosition(shooters[s])
            Transform.SetPosition(id, x + 0.3, y + 1.4, z)
            Physics.SetVelocity(id, 1.5, 2, (k - 2) * 0.5)
        end
        push(live, { id = id, dies = tick + LIFETIME })
    end
    while live[live.head] and live[live.head].dies <= tick do
        Scene.DestroyEntity(pop(live).id)
    end
end

-- This tick's shots: a ray from each shooter's eyes to the Player, and one decal
-- where the first lands (on the deck below it when it hits nothing).
local function shoot()
    local player = Scene.FindEntityByName("Player")
    local px, py, pz = 0, 1.5, -15
    if player then
        px, py, pz = Transform.GetPosition(player)
    end
    local decal_at
    for r = 1, RAYS do
        local shooter = shooters[(r - 1) % SHOOTERS + 1]
        local x, y, z = Transform.GetPosition(shooter)
        local hit, _, _, hx, hy, hz, nx, ny, nz =
            Physics.Raycast(x, y + EYES, z, px - x, py - y - EYES, pz - z, shooter)
        if r == 1 then
            decal_at = hit and { hx, hy, hz, nx, ny, nz } or { x, 0, z + 1, 0, 1, 0 }
        end
    end
    push(decals, Decals.Spawn(decal_at[1], decal_at[2], decal_at[3],
        decal_at[4], decal_at[5], decal_at[6], { size = 0.2 }))
    if decals.tail - decals.head + 1 > DECALS then
        Decals.Remove(pop(decals))
    end
end

function Firefight.Update(id, dt)
    tick = tick + 1
    spawn()
    for k = 1, TOGGLES do
        local m = muzzles[(tick * TOGGLES + k) % MUZZLES + 1]
        Scene.SetActive(m, not Scene.IsActive(m))
    end
    shoot()
    Scene.SetParent(pickup, holders[tick % 2 + 1])
end

return Firefight
