-- crates/script/src/api/particle/live.lua — the Lua half of the live-particle
-- calls (#2171). `live.rs` hands an emitter's particles over as one packed
-- string (`read`) and takes them back as one (`write`), so the per-particle
-- tables are built here, by the VM, not one Rust call per field.
--
-- A particle is 18 little-endian f32s: position xyz, velocity xyz, age,
-- lifetime, size, rotation (degrees), colour rgba, custom 1-4. A record
-- written back is a u32 source index (the live particle it updates; past the
-- end adds one) and the 18 numbers, where NaN means "keep the value".
local read, write = ...
local pack, unpack, concat = string.pack, string.unpack, table.concat
local PARTICLE = "<" .. string.rep("f", 18)
local RECORD = "<I4" .. string.rep("f", 18)
local NAN = 0 / 0
local NONE = {}

-- The record updating live particle `src` (0-based) from the table `p`. A field
-- left out is sent as NaN; one that is not a number fails in `pack`. Inlined
-- (`or`, not a helper per field): a Lua call per field was most of the cost.
local function record(src, p)
    local q, v, c, k = p.position or NONE, p.velocity or NONE, p.color or NONE, p.custom or NONE
    return pack(RECORD, src,
        q.x or NAN, q.y or NAN, q.z or NAN, v.x or NAN, v.y or NAN, v.z or NAN,
        p.age or NAN, p.lifetime or NAN, p.size or NAN, p.rotation or NAN,
        c.r or NAN, c.g or NAN, c.b or NAN, c.a or NAN,
        k[1] or NAN, k[2] or NAN, k[3] or NAN, k[4] or NAN)
end

local function live_id(id, verb)
    if id == nil then
        error("Particles." .. verb .. ": no entity (nil id)", 3)
    end
end

local function get(id)
    local s, count = read(id)
    local list, at = {}, 1
    for i = 1, count do
        local px, py, pz, vx, vy, vz, age, life, size, rot, r, g, b, a, c1, c2, c3, c4, nxt =
            unpack(PARTICLE, s, at)
        at = nxt
        list[i] = {
            position = { x = px, y = py, z = pz },
            velocity = { x = vx, y = vy, z = vz },
            age = age, lifetime = life, size = size, rotation = rot,
            color = { r = r, g = g, b = b, a = a },
            custom = { c1, c2, c3, c4 },
        }
    end
    return list
end

local function set(id, list)
    live_id(id, "SetParticles")
    if type(list) ~= "table" then
        error("Particles.SetParticles: the list is not a table", 2)
    end
    local out = {}
    for i = 1, #list do
        local p = list[i]
        if type(p) ~= "table" then
            error(("Particles.SetParticles: entry %d is not a particle table"):format(i), 2)
        end
        out[i] = record(i - 1, p)
    end
    return write(id, "SetParticles", concat(out), #list)
end

local function for_each(id, fn)
    live_id(id, "ForEach")
    local s, count = read(id)
    local P, V, C, K, p = {}, {}, {}, {}, {}
    local out, m, at = {}, 0, 1
    for i = 1, count do
        local nxt
        P.x, P.y, P.z, V.x, V.y, V.z, p.age, p.lifetime, p.size, p.rotation,
            C.r, C.g, C.b, C.a, K[1], K[2], K[3], K[4], nxt = unpack(PARTICLE, s, at)
        at = nxt
        p.position, p.velocity, p.color, p.custom = P, V, C, K
        if fn(p) ~= false then
            m = m + 1
            out[m] = record(i - 1, p)
        end
    end
    return write(id, "ForEach", concat(out), m)
end

return get, set, for_each
