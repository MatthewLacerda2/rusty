-- Four automatic weapons' per-tick load through the game API: prefab instances spawned and
-- destroyed (one under a soldier's hand bone), muzzle-flash meshes toggled, a decal stamped (oldest
-- dropped at its cap), a pickup re-parented, a raycast a shot. Each instance lives a fixed number
-- of ticks, so after LIFETIME and DECALS ticks live counts hold still: the window measures churn,
-- not growth. Deterministic.

local SHOOTERS = 4
local SPAWNS = 4
local LIFETIME = 30 -- ticks an instance lives
local MUZZLES = 8
local TOGGLES = 2
local DECALS = 64
local RAYS = 4
local RANGE = 60    -- metres
local CHEST = 1.0

local Firefight = {
    fields = {
        prefab = {
            type = "string",
            default = "scenarios/bench/casing.prefab",
            tooltip = "The casing spawned, SPAWNS a tick",
        },
        trace = {
            type = "boolean",
            default = false,
            tooltip = "Log every shot (for the firefight's tests, never the bench)",
        },
    },
}

local tick = 0
local shooters, hands, muzzles = {}, {}, {}
local holders, pickup = {}, nil
local live = { head = 1, tail = 0 }
local decals = { head = 1, tail = 0 }

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

local function box(name, parent, x, y, z, sx, sy, sz)
    local id = Scene.CreateEntity(name, "Box")
    Scene.RemoveComponent(id, "Collider")
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
        hands[i] = Animator.GetBone(soldier, "hand_r")
    end
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

-- First hit not on the shooter's own rig is taken (`root`: its hitboxes surround its muzzle).
local hits = {} -- kept across ticks: RaycastAllNonAlloc refills it
local function shoot()
    local player = Scene.FindEntityByName("Player")
    local px, py, pz = 0, 0.5, -15
    if player then
        px, py, pz = Transform.GetPosition(player)
    end
    py = py + CHEST
    local decal_at
    for r = 1, RAYS do
        local s = (r - 1) % SHOOTERS + 1
        local x, y, z = Transform.GetPosition(muzzles[s])
        local dx, dy, dz = px - x, py - y, pz - z
        local n = Physics.RaycastAllNonAlloc(hits, x, y, z, dx, dy, dz, RANGE)
        local hit
        for i = 1, n do
            if hits[i].root ~= shooters[s] then
                hit = hits[i]
                break
            end
        end
        if Firefight.trace then
            Debug.Log(string.format("firefight shot %d from %d: %s at %.3f, eye %.3f %.3f %.3f",
                r, shooters[s], hit and tostring(hit.root) or "nothing",
                hit and hit.distance or -1, Transform.GetPosition(shooters[s])))
        end
        if r == 1 then
            decal_at = hit and { hit.point.x, hit.point.y, hit.point.z,
                hit.normal.x, hit.normal.y, hit.normal.z } or { x, 0, z, 0, 1, 0 }
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
