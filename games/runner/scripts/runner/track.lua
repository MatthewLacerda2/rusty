-- runner/track.lua — the track past the recipe's start pad: chunks of floor and
-- rails built ahead of the runner and destroyed behind it, each holding rows of obstacles drawn from the
-- seeded `Random`, and the crates and barrels dropped onto it as rigidbodies when
-- the runner comes near. Every solid the runner must not touch is a *hazard*:
-- `track.hazards[id]` is its kind (`box`, `bar`, `wall` or `crate`).

local Track = {}

Track.CHUNK = 20          -- metres of track per chunk
Track.LANE = 2.5          -- metres between lane centres
Track.START = 40          -- where the generated track begins: the recipe's start pad is before it
local AHEAD = 120         -- metres of track kept built ahead of the runner
local BEHIND = 15         -- metres of track kept behind it before it is destroyed
local ROWS = { 10 }       -- where a chunk's obstacle rows sit, from its start: 0.8 s apart at top speed
local DROP_LEAD = 2.6     -- seconds ahead of the runner a crate row is dropped
local DROP_HEIGHT = 6.0   -- metres above the track a crate starts its fall

-- Lane -1, 0 or 1 (left to right as the camera sees it) to its world x. The
-- camera looks down +Z, so screen right is -X.
function Track.lane_x(lane) return -lane * Track.LANE end

function Track.new()
    return { chunks = {}, next_index = Track.START // Track.CHUNK, hazards = {}, crates = {}, drops = {}, dropped = 0 }
end

local function solid(name, parent, primitive, x, y, z, sx, sy, sz, material)
    local id = Scene.CreateEntity(name, primitive)
    Scene.SetParent(id, parent)
    Transform.SetPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Scene.AddComponent(id, "Collider") -- #2051: a primitive gets none of its own yet
    Material.Set(id, material)
    return id
end

-- Pick `n` distinct lanes of the three.
local function pick_lanes(n)
    local lanes = { -1, 0, 1 }
    for i = 3, 2, -1 do
        local j = Random.Range(1, i + 1)
        lanes[i], lanes[j] = lanes[j], lanes[i]
    end
    return { table.unpack(lanes, 1, n) }
end

-- One row of obstacles at `z`: a jumpable box in one or two lanes, a bar across
-- every lane to slide under, walls in two lanes, or crates to be dropped.
local function row(track, chunk, z)
    local kind = Random.Range(1, 5)
    local root = chunk.root
    if kind == 1 then
        for _, lane in ipairs(pick_lanes(Random.Range(1, 3))) do
            local id = solid("Box", root, "Box", Track.lane_x(lane), 0.35, z, 2.0, 0.7, 1.0, "box")
            track.hazards[id] = "box"
        end
    elseif kind == 2 then
        local id = solid("Bar", root, "Box", 0, 1.45, z, 8.2, 0.5, 0.3, "bar")
        track.hazards[id] = "bar"
    elseif kind == 3 then
        for _, lane in ipairs(pick_lanes(2)) do
            local id = solid("Wall", root, "Box", Track.lane_x(lane), 1.25, z, 2.3, 2.5, 0.6, "wall")
            track.hazards[id] = "wall"
        end
    else
        track.drops[#track.drops + 1] = { z = z, lanes = pick_lanes(Random.Range(1, 3)), chunk = chunk }
    end
end

local function build(track, index)
    local z0 = index * Track.CHUNK
    local root = Scene.CreateEntity("Chunk")
    local chunk = { index = index, root = root, crates = {} }
    local mid = z0 + Track.CHUNK / 2
    solid("Floor", root, "Box", 0, -0.5, mid, 9.0, 1, Track.CHUNK, "track")
    solid("RailLeft", root, "Box", 4.3, 0.3, mid, 0.4, 0.6, Track.CHUNK, "rail")
    solid("RailRight", root, "Box", -4.3, 0.3, mid, 0.4, 0.6, Track.CHUNK, "rail")
    for _, dz in ipairs(ROWS) do row(track, chunk, z0 + dz) end
    track.chunks[#track.chunks + 1] = chunk
end

local function destroy(track, chunk)
    for _, id in ipairs(chunk.crates) do
        track.hazards[id], track.crates[id] = nil, nil
        Scene.DestroyEntity(id)
    end
    for i = #track.drops, 1, -1 do
        if track.drops[i].chunk == chunk then table.remove(track.drops, i) end
    end
    -- The obstacles are the root's children and go with it.
    for id in pairs(track.hazards) do
        if Scene.GetParent(id) == chunk.root then track.hazards[id] = nil end
    end
    Scene.DestroyEntity(chunk.root)
end

-- Drop a crate (or a barrel) on each of `lanes` at `z`, tumbling as it falls. A
-- single lane may get a second one above the first, to stack or knock it over;
-- two lanes get one each, so the pile rarely spills into the third. Returns the
-- new bodies' ids.
function Track.drop(track, chunk, z, lanes)
    local ids = {}
    local most = #lanes == 1 and 2 or 1
    for _, lane in ipairs(lanes) do
        for n = 1, Random.Range(1, most + 1) do
            local barrel = Random.Value() < 0.35
            local id = Scene.CreateEntity(barrel and "Barrel" or "Crate", barrel and "Cylinder" or "Box")
            local x = Track.lane_x(lane) + (Random.Value() - 0.5) * 0.4
            Transform.SetPosition(id, x, DROP_HEIGHT + 1.4 * (n - 1), z + (Random.Value() - 0.5) * 0.6)
            Transform.SetRotation(id, Random.Range(0, 30), Random.Range(0, 90), Random.Range(0, 30))
            if barrel then
                Transform.SetLocalScale(id, 0.8, 0.5, 0.8)
                Scene.AddComponent(id, "Collider")
                -- A box, not a cylinder: a RigidBody has no drag to stop a
                -- barrel rolling on its side for good (#2062).
                Physics.SetColliderShape(id, { kind = "Box", x = 1, y = 2, z = 1 })
            else
                Transform.SetLocalScale(id, 0.9, 0.9, 0.9)
                Scene.AddComponent(id, "Collider")
            end
            Material.Set(id, barrel and "barrel" or "crate")
            Scene.AddComponent(id, "RigidBody")
            Physics.SetVelocity(id, 0, -4, 0)
            Physics.SetAngularVelocity(id, Random.Value() * 4 - 2, Random.Value() * 4 - 2, Random.Value() * 4 - 2)
            track.hazards[id] = "crate"
            track.dropped = track.dropped + 1
            track.crates[id] = { falling = true, n = track.dropped }
            chunk.crates[#chunk.crates + 1] = id
            ids[#ids + 1] = id
        end
    end
    return ids
end

-- Keep the track built `AHEAD` of the runner at `z` and destroyed `BEHIND` it,
-- and drop the crate rows the runner, at `speed`, reaches in `DROP_LEAD` seconds.
function Track.advance(track, z, speed)
    while track.next_index * Track.CHUNK < z + AHEAD do
        build(track, track.next_index)
        track.next_index = track.next_index + 1
    end
    while #track.chunks > 0 and (track.chunks[1].index + 1) * Track.CHUNK < z - BEHIND do
        destroy(track, table.remove(track.chunks, 1))
    end
    for i = #track.drops, 1, -1 do
        local d = track.drops[i]
        if d.z - z < speed * DROP_LEAD then
            table.remove(track.drops, i)
            Track.drop(track, d.chunk, d.z, d.lanes)
        end
    end
end

-- The crates that came to rest on something this tick, as `{x, y, z}` points.
function Track.landings(track)
    local points = {}
    for id, crate in pairs(track.crates) do
        local _, vy = Physics.GetVelocity(id)
        if crate.falling and vy > -0.5 and vy < 0.5 then
            local x, y, z = Transform.GetPosition(id)
            if y < DROP_HEIGHT - 1 then
                crate.falling = false
                points[#points + 1] = { x, 0.05, z }
            end
        end
    end
    table.sort(points, function(a, b) return a[3] < b[3] end) -- pairs order is not fixed
    return points
end

-- Every hazard's world bounds, nearest first: `{id, kind, min = {x,y,z}, max = {x,y,z}}`.
function Track.bounds(track)
    local list = {}
    for id, kind in pairs(track.hazards) do
        local found, x0, y0, z0, x1, y1, z1 = Physics.GetBounds(id)
        if found then
            list[#list + 1] = { id = id, kind = kind, min = { x0, y0, z0 }, max = { x1, y1, z1 } }
        end
    end
    table.sort(list, function(a, b)
        if a.min[3] ~= b.min[3] then return a.min[3] < b.min[3] end
        return a.id < b.id
    end)
    return list
end

-- Destroy the whole track (a restart).
function Track.clear(track)
    while #track.chunks > 0 do destroy(track, table.remove(track.chunks)) end
    track.next_index, track.drops, track.dropped = Track.START // Track.CHUNK, {}, 0
end

return Track
